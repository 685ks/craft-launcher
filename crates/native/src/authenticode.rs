//! Windows Authenticode: checking an MSI's signature before Windows Installer runs it, the
//! Windows side of the macOS `codesign` check.
//!
//! The check asks PowerShell's `Get-AuthenticodeSignature` (part of every Windows since 7) and
//! reads its answer as JSON, so no Windows API is called directly and nothing here is `unsafe`.
//! The path reaches PowerShell through an environment variable, never inside the script text,
//! so no file name can change what the script does.
//!
//! What passes ([`judge`]):
//!
//! - a valid signature, when [`WINDOWS_SIGNERS`](artcraft_launcher_core::catalog::WINDOWS_SIGNERS)
//!   names the publisher and the signer's subject is one of them;
//! - no signature at all, only while that list is empty (the Craft Apps' MSIs aren't signed
//!   yet): the download's SHA-256 has already been checked against the release, so the file is
//!   the one published;
//! - nothing else: a broken, tampered, untrusted or foreign signature always stops the install.

use serde::Deserialize;

/// What `Get-AuthenticodeSignature` says about a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Valid,
    NotSigned,
    HashMismatch,
    NotTrusted,
    UnknownError,
    NotSupportedFileFormat,
    Incompatible,
}

impl Status {
    /// The `SignatureStatus` enum, by name or by number (Windows PowerShell 5.1 writes enums
    /// as numbers in JSON).
    fn from_json(v: &serde_json::Value) -> Option<Self> {
        if let Some(n) = v.as_u64() {
            return Some(match n {
                0 => Self::Valid,
                1 => Self::UnknownError,
                2 => Self::NotSigned,
                3 => Self::HashMismatch,
                4 => Self::NotTrusted,
                5 => Self::NotSupportedFileFormat,
                6 => Self::Incompatible,
                _ => return None,
            });
        }
        Some(match v.as_str()? {
            "Valid" => Self::Valid,
            "NotSigned" => Self::NotSigned,
            "HashMismatch" => Self::HashMismatch,
            "NotTrusted" => Self::NotTrusted,
            "UnknownError" => Self::UnknownError,
            "NotSupportedFileFormat" => Self::NotSupportedFileFormat,
            "Incompatible" => Self::Incompatible,
            _ => return None,
        })
    }
}

/// A file's signature, as far as the launcher cares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub status: Status,
    /// The signing certificate's subject (`CN=Storyteller Inc, O=…`), when there is one.
    pub subject: Option<String>,
}

#[derive(Deserialize)]
struct Raw {
    #[serde(rename = "Status")]
    status: serde_json::Value,
    #[serde(rename = "Subject", default)]
    subject: Option<String>,
}

/// Reads the script's JSON answer.
pub fn parse(json: &str) -> Result<Signature, String> {
    let json = json.trim().trim_start_matches('\u{feff}');
    if json.len() > 64 * 1024 {
        return Err("the signature check answered with too much text".into());
    }
    let raw: Raw = serde_json::from_str(json).map_err(|e| format!("the signature check's answer is unreadable ({e})"))?;
    let status = Status::from_json(&raw.status).ok_or("the signature check gave an unknown status")?;
    Ok(Signature { status, subject: raw.subject.filter(|s| !s.trim().is_empty()) })
}

/// The `CN=` value of a certificate subject, if it has one.
pub fn common_name(subject: &str) -> Option<&str> {
    // Split on the commas outside quotes: `CN="Storyteller, Inc", O=…`.
    let mut parts = Vec::new();
    let (mut start, mut quoted) = (0, false);
    for (i, c) in subject.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                parts.push(subject.get(start..i).unwrap_or_default());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(subject.get(start..).unwrap_or_default());
    parts.into_iter().map(str::trim).find_map(|part| part.strip_prefix("CN=")).map(|cn| cn.trim_matches('"').trim())
}

/// Whether `signature` lets the installer run, given the publishers allowed to sign (empty: the
/// releases aren't signed yet). `Err` says why not.
pub fn judge(signature: &Signature, signers: &[&str]) -> Result<(), String> {
    match signature.status {
        Status::Valid => {
            if signers.is_empty() {
                // Signed although the releases aren't yet: a valid, trusted signature is welcome.
                return Ok(());
            }
            let cn = signature.subject.as_deref().and_then(common_name);
            match cn {
                Some(cn) if signers.iter().any(|s| s.eq_ignore_ascii_case(cn)) => Ok(()),
                Some(cn) => Err(format!("the installer is signed by {cn}, not by the app's publisher, so it wasn't run")),
                None => Err("the installer's signature names no publisher, so it wasn't run".into()),
            }
        }
        Status::NotSigned if signers.is_empty() => Ok(()),
        Status::NotSigned => Err("the installer isn't signed by the app's publisher, so it wasn't run".into()),
        Status::HashMismatch => Err("the installer's signature doesn't match its contents (it was changed after signing), so it wasn't run".into()),
        Status::NotTrusted => Err("Windows doesn't trust the installer's signature, so it wasn't run".into()),
        Status::UnknownError | Status::Incompatible | Status::NotSupportedFileFormat => {
            Err("Windows couldn't check the installer's signature, so it wasn't run".into())
        }
    }
}

/// The PowerShell script: the path comes from `$env:ARTHAVAN_PROJECTS_VERIFY`.
pub const SCRIPT: &str = "$ErrorActionPreference = 'Stop'; \
    $s = Get-AuthenticodeSignature -LiteralPath $env:ARTHAVAN_PROJECTS_VERIFY; \
    [pscustomobject]@{ Status = $s.Status.ToString(); Subject = if ($s.SignerCertificate) { $s.SignerCertificate.Subject } else { $null } } | ConvertTo-Json -Compress";

/// Checks `file`'s Authenticode signature with PowerShell.
#[cfg(windows)]
pub fn verify(file: &std::path::Path) -> Result<(), String> {
    let powershell = std::env::var_os("SystemRoot")
        .map_or_else(|| std::path::PathBuf::from(r"C:\Windows"), std::path::PathBuf::from)
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let mut cmd = crate::process::command(&powershell);
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", SCRIPT]).env("ARTHAVAN_PROJECTS_VERIFY", file);
    let out = crate::process::run_command(cmd, "powershell.exe", std::time::Duration::from_secs(60))
        .map_err(|e| format!("couldn't check the installer's signature ({e})"))?;
    let signature = parse(&out)?;
    log::info!("{}: Authenticode {:?}, signer {:?}", file.display(), signature.status, signature.subject);
    judge(&signature, artcraft_launcher_core::catalog::WINDOWS_SIGNERS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_ways_powershell_writes_the_status() {
        let s = parse(r#"{"Status":"Valid","Subject":"CN=Storyteller Inc, O=Storyteller Inc, C=US"}"#).unwrap();
        assert_eq!(s.status, Status::Valid);
        assert_eq!(s.subject.as_deref().and_then(common_name), Some("Storyteller Inc"));
        assert_eq!(parse("\u{feff}{\"Status\":2,\"Subject\":null}\r\n").unwrap(), Signature { status: Status::NotSigned, subject: None });
        assert_eq!(parse(r#"{"Status":3}"#).unwrap().status, Status::HashMismatch);
        assert!(parse(r#"{"Status":"Sparkly"}"#).is_err());
        assert!(parse(r#"{"Status":99}"#).is_err());
        assert!(parse("not json").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn common_names_come_out_of_quoted_and_plain_subjects() {
        assert_eq!(common_name(r#"CN="Storyteller, Inc", O=x"#), Some("Storyteller, Inc"));
        assert_eq!(common_name("O=x, CN=Craft"), Some("Craft"));
        assert_eq!(common_name("O=x"), None);
    }

    #[test]
    fn unsigned_installers_run_only_while_no_publisher_is_named() {
        let unsigned = Signature { status: Status::NotSigned, subject: None };
        assert_eq!(judge(&unsigned, &[]), Ok(()));
        assert!(judge(&unsigned, &["Storyteller Inc"]).is_err());
    }

    #[test]
    fn a_valid_signature_must_be_the_publishers() {
        let ours = Signature { status: Status::Valid, subject: Some("CN=Storyteller Inc, O=Storyteller Inc".into()) };
        let theirs = Signature { status: Status::Valid, subject: Some("CN=Mallory Ltd".into()) };
        let nameless = Signature { status: Status::Valid, subject: None };
        assert_eq!(judge(&ours, &["storyteller inc"]), Ok(()));
        assert!(judge(&theirs, &["Storyteller Inc"]).unwrap_err().contains("Mallory Ltd"));
        assert!(judge(&nameless, &["Storyteller Inc"]).is_err());
        assert_eq!(judge(&theirs, &[]), Ok(()));
    }

    #[test]
    fn a_broken_signature_always_stops_the_install() {
        for status in [Status::HashMismatch, Status::NotTrusted, Status::UnknownError, Status::Incompatible, Status::NotSupportedFileFormat] {
            let s = Signature { status, subject: Some("CN=Storyteller Inc".into()) };
            assert!(judge(&s, &[]).is_err(), "{status:?}");
            assert!(judge(&s, &["Storyteller Inc"]).is_err(), "{status:?}");
        }
    }

    #[test]
    fn the_script_takes_the_path_from_the_environment_only() {
        assert!(SCRIPT.contains("-LiteralPath $env:ARTHAVAN_PROJECTS_VERIFY"));
        assert!(SCRIPT.ends_with("ConvertTo-Json -Compress"));
    }
}
