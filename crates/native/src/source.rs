//! Where release information comes from.
//!
//! [`GitHubSource`] asks the GitHub REST API (one request per app, revalidated with ETags) and,
//! when the API is out of quota, falls back to github.com's release pages: the
//! `releases/latest` redirect names the tag and `SHA256SUMS.txt` lists every file with its
//! checksum, which costs no API quota at all. A later [`ReleaseSource`] will ask the
//! catalog API for every app in one request.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use artcraft_launcher_core::channel::Channel;
use artcraft_launcher_core::protocol::{LatestRelease, Source};
use artcraft_launcher_core::release::{self, Release};
use serde::{Deserialize, Serialize};

use crate::http::{Fetched, Http};

/// One answer from a source.
#[derive(Clone, Debug)]
pub struct Answer {
    pub latest: LatestRelease,
    pub source: Source,
}

/// Looks up latest releases. Called from worker threads.
pub trait ReleaseSource: Send + Sync {
    fn latest(&self, repo: &str, channel: Channel) -> Answer;
    /// The last answer saved for `repo` on `channel`, for an instant first paint.
    fn cached(&self, repo: &str, channel: Channel) -> Option<LatestRelease>;
    /// API refused until this time (Unix seconds).
    fn rate_limited_until(&self) -> Option<i64>;
    /// Persist anything worth keeping.
    fn save(&self);
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Entry {
    etag: Option<String>,
    /// `None`: the repo has no releases.
    release: Option<Release>,
    /// Saved before channels: `true` reads as Beta.
    prereleases: bool,
    #[serde(default)]
    channel: Option<Channel>,
    fetched_at: i64,
}

impl Entry {
    fn channel(&self) -> Channel {
        self.channel.unwrap_or(Channel::default_for(self.prereleases))
    }
}

#[derive(Default, Serialize, Deserialize)]
struct CacheFile {
    entries: BTreeMap<String, Entry>,
}

pub struct GitHubSource {
    http: Http,
    cache: Mutex<CacheFile>,
    cache_path: std::path::PathBuf,
    limited_until: Mutex<Option<i64>>,
    /// `https://api.github.com`; a local fake in tests.
    api: String,
}

impl GitHubSource {
    pub fn new(http: Http, cache_path: std::path::PathBuf) -> Self {
        let cache = std::fs::read(&cache_path).ok().and_then(|b| serde_json::from_slice::<CacheFile>(&b).ok()).unwrap_or_default();
        Self { http, cache: Mutex::new(cache), cache_path, limited_until: Mutex::new(None), api: "https://api.github.com".into() }
    }

    fn entry(&self, repo: &str, channel: Channel) -> Option<Entry> {
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        cache.entries.get(repo).filter(|e| e.channel() == channel).cloned()
    }

    fn store(&self, repo: &str, entry: Entry) {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner).entries.insert(repo.to_string(), entry);
    }

    fn limited(&self) -> bool {
        let until = *self.limited_until.lock().unwrap_or_else(PoisonError::into_inner);
        until.is_some_and(|u| u > crate::now())
    }

    fn ask_api(&self, repo: &str, channel: Channel) -> Result<Option<Answer>, String> {
        let prereleases = channel.wants_prereleases();
        let url = if prereleases {
            format!("{}/repos/{repo}/releases?per_page=30", self.api)
        } else {
            format!("{}/repos/{repo}/releases/latest", self.api)
        };
        let saved = self.entry(repo, channel);
        let found =
            |release: Option<Release>| Answer { latest: release.map_or(LatestRelease::None, LatestRelease::Found), source: Source::GitHubApi };
        match self.http.get_api(&url, saved.as_ref().and_then(|e| e.etag.as_deref()))? {
            Fetched::Body { text, etag } => {
                let release = if prereleases {
                    release::parse_github_release_list_for(&text, repo, channel)?
                } else {
                    Some(release::parse_github_release(&text, repo)?)
                };
                self.store(repo, Entry { etag, release: release.clone(), prereleases, channel: Some(channel), fetched_at: crate::now() });
                Ok(Some(found(release)))
            }
            Fetched::NotModified => match saved {
                Some(e) => {
                    self.store(repo, Entry { fetched_at: crate::now(), ..e.clone() });
                    Ok(Some(found(e.release)))
                }
                None => Err("GitHub said nothing changed, but there's no saved answer".into()),
            },
            Fetched::NotFound => {
                self.store(repo, Entry { etag: None, release: None, prereleases, channel: Some(channel), fetched_at: crate::now() });
                Ok(Some(found(None)))
            }
            Fetched::RateLimited { reset } => {
                *self.limited_until.lock().unwrap_or_else(PoisonError::into_inner) = Some(reset.unwrap_or_else(|| crate::now() + 15 * 60));
                Ok(None)
            }
        }
    }

    /// The quota-free route through github.com (stable releases only).
    fn ask_pages(&self, repo: &str) -> Result<Answer, String> {
        let location = self.http.redirect_target(&format!("https://github.com/{repo}/releases/latest"))?;
        let Some(tag) = location.as_deref().and_then(|l| release::tag_from_redirect(l, repo)) else {
            // No published release: github.com sends us to the release list instead.
            return Ok(Answer { latest: LatestRelease::None, source: Source::GitHubPages });
        };
        if let Some(e) = self.entry(repo, Channel::Stable)
            && let Some(r) = e.release.as_ref().filter(|r| r.tag == tag)
        {
            return Ok(Answer { latest: LatestRelease::Found(r.clone()), source: Source::GitHubPages });
        }
        let sums_url = format!("{}{tag}/SHA256SUMS.txt", release::download_prefix(repo));
        let sums = self.http.get_text(&sums_url)?.map(|t| release::parse_sha256sums(&t)).unwrap_or_default();
        let r = release::release_from_sums(repo, &tag, &sums);
        self.store(
            repo,
            Entry { etag: None, release: Some(r.clone()), prereleases: false, channel: Some(Channel::Stable), fetched_at: crate::now() },
        );
        Ok(Answer { latest: LatestRelease::Found(r), source: Source::GitHubPages })
    }
}

impl ReleaseSource for GitHubSource {
    fn latest(&self, repo: &str, channel: Channel) -> Answer {
        let api_error = if self.limited() {
            None
        } else {
            match self.ask_api(repo, channel) {
                Ok(Some(answer)) => return answer,
                Ok(None) => None,
                Err(e) => Some(e),
            }
        };
        if channel == Channel::Stable && api_error.is_none() {
            match self.ask_pages(repo) {
                Ok(answer) => return answer,
                Err(e) => log::warn!("{repo}: release pages: {e}"),
            }
        }
        let why = api_error.unwrap_or_else(|| "GitHub's request limit was reached".into());
        match self.cached(repo, channel) {
            Some(latest) => Answer { latest, source: Source::Cache },
            None => Answer { latest: LatestRelease::Failed(why), source: Source::Cache },
        }
    }

    fn cached(&self, repo: &str, channel: Channel) -> Option<LatestRelease> {
        self.entry(repo, channel).map(|e| e.release.map_or(LatestRelease::None, LatestRelease::Found))
    }

    fn rate_limited_until(&self) -> Option<i64> {
        let until = *self.limited_until.lock().unwrap_or_else(PoisonError::into_inner);
        until.filter(|u| *u > crate::now())
    }

    fn save(&self) {
        let json = {
            let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
            serde_json::to_vec(&*cache)
        };
        match json {
            Ok(bytes) => {
                if let Err(e) = crate::paths::write_atomic(&self.cache_path, &bytes) {
                    log::warn!("release cache: {e}");
                }
            }
            Err(e) => log::warn!("release cache: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::sync::Arc;

    use super::*;

    const REPO: &str = "storytold/photocraft";

    /// A fake GitHub API: answers each request with the next of `replies` (status line and
    /// headers, body), recording each request's `If-None-Match`.
    fn fake_api(replies: Vec<(&'static str, String)>) -> (String, Arc<Mutex<Vec<Option<String>>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (head, body) in replies {
                let Ok((stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(stream);
                let mut etag = None;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':')
                        && name.eq_ignore_ascii_case("if-none-match")
                    {
                        etag = Some(value.trim().to_string());
                    }
                }
                log.lock().unwrap().push(etag);
                let mut stream = reader.into_inner();
                let _ = write!(stream, "{head}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        (base, seen)
    }

    fn source(api: String, dir: &std::path::Path) -> GitHubSource {
        let mut s = GitHubSource::new(Http::new().unwrap(), dir.join("releases.json"));
        s.api = api;
        s
    }

    fn list() -> String {
        format!("[{}]", include_str!("../../core/tests/fixtures/photocraft-release.json"))
    }

    #[test]
    fn an_unchanged_release_is_revalidated_with_its_etag_and_read_from_the_cache() {
        let (api, seen) = fake_api(vec![("HTTP/1.1 200 OK\r\nETag: \"abc\"", list()), ("HTTP/1.1 304 Not Modified", String::new())]);
        let dir = tempfile::tempdir().unwrap();
        let s = source(api, dir.path());
        let first = s.latest(REPO, Channel::Beta);
        let LatestRelease::Found(r) = &first.latest else { panic!("{first:?}") };
        assert_eq!(r.tag, "v0.9.0");
        assert_eq!(first.source, Source::GitHubApi);
        let again = s.latest(REPO, Channel::Beta);
        assert_eq!(again.latest, first.latest, "304: the saved answer");
        assert_eq!(*seen.lock().unwrap(), [None, Some("\"abc\"".to_string())]);
        // Saved, so the next run starts from it (and revalidates with the same tag).
        s.save();
        let reloaded = GitHubSource::new(Http::new().unwrap(), dir.path().join("releases.json"));
        assert_eq!(reloaded.cached(REPO, Channel::Beta), Some(first.latest));
        assert_eq!(reloaded.entry(REPO, Channel::Beta).and_then(|e| e.etag).as_deref(), Some("\"abc\""));
    }

    #[test]
    fn a_429_backs_off_and_answers_from_the_cache_without_asking_again() {
        let (api, seen) =
            fake_api(vec![("HTTP/1.1 200 OK\r\nETag: \"abc\"", list()), ("HTTP/1.1 429 Too Many Requests\r\nRetry-After: 600", String::new())]);
        let dir = tempfile::tempdir().unwrap();
        let s = source(api, dir.path());
        let first = s.latest(REPO, Channel::Beta).latest;
        let limited = s.latest(REPO, Channel::Beta);
        assert_eq!((limited.latest, limited.source), (first.clone(), Source::Cache));
        let until = s.rate_limited_until().unwrap();
        assert!(until > crate::now() + 500, "held off for Retry-After");
        // Still held off: GitHub isn't asked at all.
        let held = s.latest(REPO, Channel::Beta);
        assert_eq!((held.latest, held.source), (first, Source::Cache));
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[test]
    fn each_channel_keeps_its_own_answer() {
        let (api, _) = fake_api(vec![("HTTP/1.1 200 OK", list())]);
        let dir = tempfile::tempdir().unwrap();
        let s = source(api, dir.path());
        assert!(matches!(s.latest(REPO, Channel::Beta).latest, LatestRelease::Found(_)));
        assert_eq!(s.cached(REPO, Channel::Stable), None, "a Beta answer isn't the Stable one");
    }
}
