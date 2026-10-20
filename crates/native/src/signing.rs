//! Signed release manifests and the signed catalog: minisign (Ed25519) signatures checked
//! against the public key built into the launcher
//! ([`RELEASE_SIGNING_KEY`](artcraft_launcher_core::catalog::RELEASE_SIGNING_KEY)).
//!
//! A release may carry `SHA256SUMS.txt.minisig`, a signature of its `SHA256SUMS.txt` made in
//! the release workflow (see `docs/release-signing.md`). When it does, the checksums the
//! launcher installs against come from the signed list, and a signature that doesn't check out
//! stops the install. A release without one (v0.5.1 and earlier, or any release made without the
//! signing secret) is still installed, against its published checksums, and shows as unsigned.
//!
//! The signature's trusted comment must name the release's tag, so a signed list can't be
//! replayed onto another release.

use artcraft_launcher_core::protocol::SignatureState;
use artcraft_launcher_core::release::{self, Release};

use crate::http::Http;

/// The name of a release's signed checksum list.
pub const SUMS_SIGNATURE: &str = "SHA256SUMS.txt.minisig";
/// The longest signature file read.
const MAX_SIGNATURE: usize = 4 * 1024;

/// Checks `signature` (a `.minisig` file's text) of `message` against `public_key` (the
/// base64 line of a `minisign.pub`, or the whole file). Returns the trusted comment.
pub fn verify(public_key: &str, message: &[u8], signature: &str) -> Result<String, String> {
    if signature.len() > MAX_SIGNATURE {
        return Err("the signature file is too large".into());
    }
    let key = public_key.trim();
    let key = if key.lines().count() > 1 { minisign_verify::PublicKey::decode(key) } else { minisign_verify::PublicKey::from_base64(key) }
        .map_err(|e| format!("the built-in signing key is unreadable ({e})"))?;
    let sig = minisign_verify::Signature::decode(signature).map_err(|e| format!("the signature is unreadable ({e})"))?;
    key.verify(message, &sig, false).map_err(|e| format!("the signature doesn't match ({e})"))?;
    Ok(sig.trusted_comment().to_string())
}

/// Whether a trusted comment names `tag` as a whole word (`Arthavan Projects v0.6.0`).
pub fn names_tag(comment: &str, tag: &str) -> bool {
    !tag.is_empty() && comment.split(|c: char| c.is_whitespace() || c == ',' || c == ';').any(|w| w == tag)
}

/// The release's checksum list, checked against its signature when it has one and the launcher
/// has a key to check it with: `Ok(Some(list))` when signed and verified, `Ok(None)` when unsigned,
/// `Err` when the signature is there but wrong, or can't be fetched.
pub fn signed_sums(release: &Release, http: &Http) -> Result<Option<Vec<(String, String)>>, String> {
    signed_sums_with(release, artcraft_launcher_core::catalog::RELEASE_SIGNING_KEY, |url| http.get_text(url).map_err(|e| e.to_string()))
}

/// [`signed_sums`] with the key and the fetching passed in, for tests.
pub fn signed_sums_with(
    release: &Release,
    key: Option<&str>,
    fetch: impl Fn(&str) -> Result<Option<String>, String>,
) -> Result<Option<Vec<(String, String)>>, String> {
    let (Some(key), Some(sig_asset), Some(sums_asset)) = (key, release.assets.iter().find(|a| a.name == SUMS_SIGNATURE), release.sums_asset()) else {
        return Ok(None);
    };
    let sig = fetch(&sig_asset.url)?.ok_or("the release's signature couldn't be fetched")?;
    let sums = fetch(&sums_asset.url)?.ok_or("the release's checksums couldn't be fetched")?;
    let comment = verify(key, sums.as_bytes(), &sig).map_err(|e| format!("this release's signed checksums don't verify: {e}"))?;
    if !names_tag(&comment, &release.tag) {
        return Err(format!("this release's signature was made for another release ({comment})"));
    }
    Ok(Some(release::parse_sha256sums(&sums)))
}

/// What the UI is told about a release's signature.
pub fn state(release: &Release, http: &Http) -> SignatureState {
    match signed_sums(release, http) {
        Ok(Some(_)) => SignatureState::Signed,
        Ok(None) => SignatureState::Unsigned,
        Err(e) => SignatureState::Invalid(e),
    }
}

/// The checksum to install `asset` against, from a verified signed list: the list must name
/// the file, and agree with any checksum GitHub or the feed gave for it.
pub fn checksum_from_signed(signed: &[(String, String)], name: &str, published: Option<&str>) -> Result<String, String> {
    let hash = signed
        .iter()
        .find(|(_, n)| n == name)
        .map(|(h, _)| h.to_ascii_lowercase())
        .ok_or_else(|| format!("the signed checksums don't list {name}"))?;
    if published.is_some_and(|p| !p.eq_ignore_ascii_case(&hash)) {
        return Err(format!("{name}'s published checksum differs from the signed one, so it wasn't installed"));
    }
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use artcraft_launcher_core::release::Asset;

    use super::*;

    const PUB: &str = include_str!("../tests/fixtures/minisign/test.pub");
    const OTHER_PUB: &str = include_str!("../tests/fixtures/minisign/other.pub");
    const SUMS: &str = include_str!("../tests/fixtures/minisign/SHA256SUMS.txt");
    const SIG: &str = include_str!("../tests/fixtures/minisign/SHA256SUMS.txt.minisig");
    const OTHER_SIG: &str = include_str!("../tests/fixtures/minisign/SHA256SUMS.txt.other.minisig");

    fn key_line(file: &str) -> &str {
        file.lines().nth(1).unwrap()
    }

    #[test]
    fn a_minisign_signature_verifies_with_its_key_only() {
        assert_eq!(verify(PUB, SUMS.as_bytes(), SIG).unwrap(), "Arthavan Projects v0.6.0");
        assert_eq!(verify(key_line(PUB), SUMS.as_bytes(), SIG).unwrap(), "Arthavan Projects v0.6.0");
        assert!(verify(OTHER_PUB, SUMS.as_bytes(), SIG).is_err());
        assert!(verify(PUB, SUMS.as_bytes(), OTHER_SIG).is_err());
        let tampered = SUMS.replace("1111", "1112");
        assert!(verify(PUB, tampered.as_bytes(), SIG).unwrap_err().contains("doesn't match"));
        assert!(verify(PUB, SUMS.as_bytes(), "garbage").is_err());
        assert!(verify("not a key", SUMS.as_bytes(), SIG).is_err());
    }

    #[test]
    fn trusted_comments_must_name_the_tag_exactly() {
        assert!(names_tag("Arthavan Projects v0.6.0", "v0.6.0"));
        assert!(!names_tag("Arthavan Projects v0.6.0", "v0.6"));
        assert!(!names_tag("Arthavan Projects v0.6.0-rc.1", "v0.6.0"));
        assert!(!names_tag("anything", ""));
    }

    fn release(tag: &str, signed: bool) -> Release {
        let base = "https://github.com/685ks/craft-launcher/releases/download";
        let mut assets = vec![Asset { name: "SHA256SUMS.txt".into(), size: 0, url: format!("{base}/{tag}/SHA256SUMS.txt"), sha256: None }];
        if signed {
            assets.push(Asset { name: SUMS_SIGNATURE.into(), size: 0, url: format!("{base}/{tag}/{SUMS_SIGNATURE}"), sha256: None });
        }
        Release { tag: tag.into(), assets, ..Release::default() }
    }

    fn fetch(sig: &'static str) -> impl Fn(&str) -> Result<Option<String>, String> {
        move |url: &str| Ok(Some(if url.ends_with(".minisig") { sig.to_string() } else { SUMS.to_string() }))
    }

    #[test]
    fn a_signed_release_yields_its_signed_checksums() {
        let sums = signed_sums_with(&release("v0.6.0", true), Some(key_line(PUB)), fetch(SIG)).unwrap().unwrap();
        assert_eq!(sums.len(), 3);
        assert_eq!(
            checksum_from_signed(&sums, "arthavan-projects-0.6.0-windows-x64.msi", None).unwrap(),
            "3333333333333333333333333333333333333333333333333333333333333333"
        );
    }

    #[test]
    fn unsigned_releases_and_keyless_builds_stay_installable() {
        let unexpected = |_: &str| -> Result<Option<String>, String> { panic!("nothing should be fetched") };
        assert_eq!(signed_sums_with(&release("v0.5.1", false), Some(key_line(PUB)), unexpected), Ok(None));
        assert_eq!(signed_sums_with(&release("v0.6.0", true), None, unexpected), Ok(None));
    }

    #[test]
    fn a_bad_or_replayed_signature_stops_the_install() {
        assert!(signed_sums_with(&release("v0.6.0", true), Some(key_line(PUB)), fetch(OTHER_SIG)).is_err());
        let replayed = signed_sums_with(&release("v0.7.0", true), Some(key_line(PUB)), fetch(SIG)).unwrap_err();
        assert!(replayed.contains("another release"), "{replayed}");
        let missing = |url: &str| Ok(if url.ends_with(".minisig") { None } else { Some(SUMS.to_string()) });
        assert!(signed_sums_with(&release("v0.6.0", true), Some(key_line(PUB)), missing).is_err());
    }

    #[test]
    fn the_signed_checksum_must_list_the_file_and_agree_with_github() {
        let sums = vec![("ab".repeat(32), "a.msi".to_string())];
        assert_eq!(checksum_from_signed(&sums, "a.msi", Some(&"AB".repeat(32))).unwrap(), "ab".repeat(32));
        assert!(checksum_from_signed(&sums, "a.msi", Some(&"cd".repeat(32))).is_err());
        assert!(checksum_from_signed(&sums, "b.msi", None).is_err());
    }
}
