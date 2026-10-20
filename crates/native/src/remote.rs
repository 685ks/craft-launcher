//! Fetching the signed catalog (`core::remote_catalog`), with the last good copy kept.
//!
//! A catalog is used only once its minisign signature checks out against the built-in key and
//! its trusted comment says `catalog`. A fetched catalog older than the one already kept is
//! ignored, so an old (validly signed) file can't be replayed. Without a key in this build
//! nothing is fetched at all and the built-in catalog stands.

use artcraft_launcher_core::remote_catalog::{self, RemoteCatalog};

use crate::http::Http;
use crate::paths::Paths;

/// Checks a catalog's signature and reads it.
pub fn verify(key: &str, json: &str, signature: &str) -> Result<RemoteCatalog, String> {
    let comment = crate::signing::verify(key, json.as_bytes(), signature)?;
    if !comment.split_whitespace().any(|w| w.eq_ignore_ascii_case("catalog")) {
        return Err(format!("the signature was made for something other than the catalog ({comment})"));
    }
    RemoteCatalog::parse(json)
}

/// The catalog kept from an earlier run, checked again.
pub fn load_cached(paths: &Paths) -> Option<RemoteCatalog> {
    let key = artcraft_launcher_core::catalog::RELEASE_SIGNING_KEY?;
    load_cached_with(paths, key)
}

fn load_cached_with(paths: &Paths, key: &str) -> Option<RemoteCatalog> {
    let json = std::fs::read_to_string(paths.catalog_cache()).ok()?;
    let sig = std::fs::read_to_string(paths.catalog_signature_cache()).ok()?;
    match verify(key, &json, &sig) {
        Ok(c) => Some(c),
        Err(e) => {
            log::warn!("the saved catalog doesn't verify ({e}); using the built-in one");
            None
        }
    }
}

/// Fetches the catalog; returns it when it verifies and is at least as new as `current`, and
/// keeps it for the next run.
pub fn refresh(http: &Http, paths: &Paths, current: Option<&RemoteCatalog>) -> Option<RemoteCatalog> {
    let key = artcraft_launcher_core::catalog::RELEASE_SIGNING_KEY?;
    refresh_with(paths, key, current, |url| http.get_text(url))
}

pub(crate) fn refresh_with(
    paths: &Paths,
    key: &str,
    current: Option<&RemoteCatalog>,
    fetch: impl Fn(&str) -> Result<Option<String>, String>,
) -> Option<RemoteCatalog> {
    let fetched = (|| -> Result<Option<(String, String)>, String> {
        let Some(json) = fetch(remote_catalog::CATALOG_URL)? else { return Ok(None) };
        if json.len() > remote_catalog::MAX_BYTES {
            return Err("the catalog is too large".into());
        }
        let sig = fetch(remote_catalog::CATALOG_SIGNATURE_URL)?.ok_or("the catalog has no signature")?;
        Ok(Some((json, sig)))
    })();
    let (json, sig) = match fetched {
        Ok(Some(pair)) => pair,
        Ok(None) => return None,
        Err(e) => {
            log::info!("catalog unavailable: {e}");
            return None;
        }
    };
    let catalog = match verify(key, &json, &sig) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("ignoring the fetched catalog: {e}");
            return None;
        }
    };
    if current.is_some_and(|c| catalog.issued_at < c.issued_at) {
        log::warn!("ignoring a fetched catalog older than the one kept");
        return None;
    }
    for (path, text) in [(paths.catalog_cache(), &json), (paths.catalog_signature_cache(), &sig)] {
        if let Err(e) = crate::paths::write_atomic(&path, text.as_bytes()) {
            log::warn!("couldn't keep the catalog: {e}");
        }
    }
    Some(catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUB: &str = include_str!("../tests/fixtures/minisign/test.pub");
    const OTHER_PUB: &str = include_str!("../tests/fixtures/minisign/other.pub");
    const CATALOG: &str = include_str!("../tests/fixtures/minisign/catalog.json");
    const CATALOG_SIG: &str = include_str!("../tests/fixtures/minisign/catalog.json.minisig");
    const SUMS: &str = include_str!("../tests/fixtures/minisign/SHA256SUMS.txt");
    const SUMS_SIG: &str = include_str!("../tests/fixtures/minisign/SHA256SUMS.txt.minisig");

    fn serve(json: &'static str, sig: &'static str) -> impl Fn(&str) -> Result<Option<String>, String> {
        move |url: &str| Ok(Some(if url.ends_with(".minisig") { sig } else { json }.to_string()))
    }

    #[test]
    fn a_signed_catalog_is_used_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let c = refresh_with(&paths, PUB, None, serve(CATALOG, CATALOG_SIG)).unwrap();
        assert_eq!(c.issued_at, 1_792_500_000);
        assert_eq!(c.retired(), vec!["gridcraft".to_string()]);
        // Unknown apps are dropped.
        assert!(!c.apps.contains_key("unknowncraft"));
        assert_eq!(load_cached_with(&paths, PUB), Some(c));
    }

    #[test]
    fn a_wrong_key_another_files_signature_or_an_older_catalog_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        assert_eq!(refresh_with(&paths, OTHER_PUB, None, serve(CATALOG, CATALOG_SIG)), None);
        // A validly signed checksum list isn't a catalog.
        assert!(verify(PUB, SUMS, SUMS_SIG).is_err());
        let newer = RemoteCatalog { issued_at: 1_800_000_000, ..RemoteCatalog::default() };
        assert_eq!(refresh_with(&paths, PUB, Some(&newer), serve(CATALOG, CATALOG_SIG)), None);
        assert!(!paths.catalog_cache().exists());
        let tampered = CATALOG.replace("retired\": true", "retired\": false");
        assert!(verify(PUB, &tampered, CATALOG_SIG).is_err());
    }

    #[test]
    fn without_an_answer_the_built_in_catalog_stands() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        assert_eq!(refresh_with(&paths, PUB, None, |_| Ok(None)), None);
        assert_eq!(refresh_with(&paths, PUB, None, |_| Err("offline".to_string())), None);
        assert_eq!(load_cached_with(&paths, PUB), None);
        // A saved copy that no longer verifies is not used.
        std::fs::create_dir_all(&paths.cache).unwrap();
        std::fs::write(paths.catalog_cache(), CATALOG.replace("1792500000", "1792500001")).unwrap();
        std::fs::write(paths.catalog_signature_cache(), CATALOG_SIG).unwrap();
        assert_eq!(load_cached_with(&paths, PUB), None);
    }
}
