//! The signed catalog: a small JSON file published in this repository (`catalog/catalog.json`,
//! signed as `catalog.json.minisig`) that can adjust the built-in [`SUITE`](crate::SUITE)
//! without a new launcher release.
//!
//! It can only adjust apps the launcher already knows: point one at a renamed repository (still
//! under [`GITHUB_OWNER`](crate::catalog::GITHUB_OWNER), so the download rules hold) or mark one
//! retired (hidden unless installed). It can never add an app, a download source or an install
//! path; ids it doesn't know are ignored. Without a verified copy the launcher uses the built-in
//! catalog unchanged.
//!
//! ```json
//! { "schema": 1, "issued_at": 1792500000,
//!   "apps": [ { "id": "photocraft", "repo": "685ks/photocraft" }, { "id": "gridcraft", "retired": true } ] }
//! ```

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::catalog::{CraftApp, GITHUB_OWNER};

/// Where the catalog is published: the raw file on this repository's main branch.
pub const CATALOG_URL: &str = concat!("https://raw.githubusercontent.com/", "685ks", "/craft-launcher/main/catalog/catalog.json");
/// Its signature, beside it.
pub const CATALOG_SIGNATURE_URL: &str = concat!("https://raw.githubusercontent.com/", "685ks", "/craft-launcher/main/catalog/catalog.json.minisig");
/// The only schema this launcher reads.
pub const SCHEMA: u32 = 1;
/// The largest catalog read, in bytes.
pub const MAX_BYTES: usize = 256 * 1024;

/// What the catalog says about one app.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppOverride {
    /// The repository to ask about releases, instead of the built-in one.
    pub repo: Option<String>,
    /// No longer offered: hidden from the lists unless installed.
    pub retired: bool,
}

/// A verified catalog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemoteCatalog {
    /// When it was made (Unix seconds): an older catalog never replaces a newer one.
    pub issued_at: i64,
    pub apps: BTreeMap<String, AppOverride>,
}

#[derive(Deserialize)]
struct RawCatalog {
    schema: u32,
    issued_at: i64,
    #[serde(default)]
    apps: Vec<RawApp>,
}

#[derive(Deserialize)]
struct RawApp {
    id: String,
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    retired: bool,
}

/// Whether `repo` is `<GITHUB_OWNER>/<name>` with an ordinary repository name.
pub fn allowed_repo(repo: &str) -> bool {
    let Some((owner, name)) = repo.split_once('/') else { return false };
    owner == GITHUB_OWNER
        && !name.is_empty()
        && name.len() <= 100
        && name != "."
        && name != ".."
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

impl RemoteCatalog {
    /// Reads a catalog. Entries for unknown apps are dropped; a repository outside the owner's
    /// account is an error for the whole file (it was signed, so something is badly wrong).
    pub fn parse(json: &str) -> Result<Self, String> {
        if json.len() > MAX_BYTES {
            return Err("the catalog is too large".into());
        }
        let raw: RawCatalog = serde_json::from_str(json).map_err(|e| format!("the catalog is unreadable ({e})"))?;
        if raw.schema != SCHEMA {
            return Err(format!("the catalog uses schema {}, which this version doesn't read", raw.schema));
        }
        let mut apps = BTreeMap::new();
        for app in raw.apps {
            if CraftApp::by_id(&app.id).is_none() {
                continue;
            }
            if let Some(repo) = &app.repo
                && !allowed_repo(repo)
            {
                return Err(format!("the catalog points {} at {repo}, outside {GITHUB_OWNER}", app.id));
            }
            apps.insert(app.id, AppOverride { repo: app.repo, retired: app.retired });
        }
        Ok(Self { issued_at: raw.issued_at, apps })
    }

    /// The repository to ask about `app`'s releases.
    pub fn repo_for(&self, app: &CraftApp) -> String {
        self.apps.get(app.id).and_then(|o| o.repo.clone()).unwrap_or_else(|| app.repo.to_string())
    }

    /// The ids of retired apps.
    pub fn retired(&self) -> Vec<String> {
        self.apps.iter().filter(|(_, o)| o.retired).map(|(id, _)| id.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_overrides_for_known_apps_only() {
        let c = RemoteCatalog::parse(
            r#"{"schema":1,"issued_at":5,"apps":[{"id":"photocraft","repo":"685ks/photocraft-next"},{"id":"gridcraft","retired":true},{"id":"evilcraft","repo":"685ks/evil"}],"extra":1}"#,
        )
        .unwrap();
        assert_eq!(c.issued_at, 5);
        assert_eq!(c.apps.len(), 2);
        let photo = CraftApp::by_id("photocraft").unwrap();
        let film = CraftApp::by_id("filmcraft").unwrap();
        assert_eq!(c.repo_for(photo), "685ks/photocraft-next");
        assert_eq!(c.repo_for(film), film.repo);
        assert_eq!(c.retired(), vec!["gridcraft".to_string()]);
    }

    #[test]
    fn repositories_stay_under_the_owner() {
        assert!(allowed_repo("685ks/photocraft"));
        for bad in ["someone/photocraft", "685ks/", "685ks/..", "685ks/a/b", "685ks/a?b", "photocraft", "685ks/a b"] {
            assert!(!allowed_repo(bad), "{bad}");
        }
        let err = RemoteCatalog::parse(r#"{"schema":1,"issued_at":1,"apps":[{"id":"photocraft","repo":"mallory/photocraft"}]}"#).unwrap_err();
        assert!(err.contains("outside"), "{err}");
    }

    #[test]
    fn other_schemas_and_garbage_are_refused() {
        assert!(RemoteCatalog::parse(r#"{"schema":2,"issued_at":1,"apps":[]}"#).is_err());
        assert!(RemoteCatalog::parse("[]").is_err());
        assert!(RemoteCatalog::parse(&" ".repeat(MAX_BYTES + 1)).is_err());
        assert_eq!(RemoteCatalog::parse(r#"{"schema":1,"issued_at":0}"#).unwrap(), RemoteCatalog::default());
    }

    #[test]
    fn the_catalog_lives_in_the_launchers_own_repository() {
        assert!(CATALOG_URL.starts_with(&format!("https://raw.githubusercontent.com/{}/", crate::catalog::LAUNCHER_REPO)));
        assert_eq!(CATALOG_SIGNATURE_URL, format!("{CATALOG_URL}.minisig"));
    }
}
