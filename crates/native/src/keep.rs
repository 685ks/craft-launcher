//! The packages kept for "Roll back": after a successful install, the file it came from is moved
//! to `<data>/packages/<app>/current/`, and the one that was there before goes to `previous/`.
//! Rolling back installs `previous/` again (after checking its SHA-256 still matches) and then
//! swaps the two, so the newer version is the one kept.
//!
//! Only the launcher's own files are touched, everything stays under its data folder, and a
//! failure to keep a package never fails the install that produced it.

use std::path::{Path, PathBuf};

use artcraft_launcher_core::PackageKind;
use serde::{Deserialize, Serialize};

use crate::paths::Paths;

/// One kept package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    pub version: String,
    pub kind: PackageKind,
    /// The file's name inside its slot (the release asset's name).
    pub file: String,
    /// Lowercase hex SHA-256 of the file when it was kept.
    pub sha256: String,
    pub size: u64,
}

/// The formats the launcher can put an older version of back by itself: it owns the files (the
/// portable ZIP). Windows Installer refuses or needs extra steps to downgrade, so MSI downloads
/// aren't kept.
pub fn can_roll_back(kind: PackageKind) -> bool {
    matches!(kind, PackageKind::PortableZip)
}

/// Bytes kept for rolling back, every app together.
pub fn total_size(paths: &Paths) -> u64 {
    crate::disk::size_of(&paths.packages())
}

const CURRENT: &str = "current";
const PREVIOUS: &str = "previous";
const INFO: &str = "kept.json";

fn app_dir(paths: &Paths, app: &str) -> PathBuf {
    paths.packages().join(app)
}

/// What a slot holds, if it holds something usable.
fn read_slot(dir: &Path) -> Option<(Kept, PathBuf)> {
    let kept: Kept = serde_json::from_slice(&std::fs::read(dir.join(INFO)).ok()?).ok()?;
    if !artcraft_launcher_core::release::valid_file_name(&kept.file) {
        return None;
    }
    let file = dir.join(&kept.file);
    file.is_file().then_some((kept, file))
}

/// The package kept from before the last update, if any.
pub fn previous(paths: &Paths, app: &str) -> Option<(Kept, PathBuf)> {
    read_slot(&app_dir(paths, app).join(PREVIOUS))
}

/// The package of the version installed now, if it was kept.
pub fn current(paths: &Paths, app: &str) -> Option<(Kept, PathBuf)> {
    read_slot(&app_dir(paths, app).join(CURRENT))
}

/// Moves `file` (just installed, `version` in `kind`) into the current slot, the old current one
/// into the previous slot. Reinstalling the same version replaces the current slot only.
pub fn keep(paths: &Paths, app: &str, version: &str, kind: PackageKind, file: &Path) -> Result<(), String> {
    let name = file.file_name().and_then(|n| n.to_str()).ok_or("the package has no usable name")?.to_string();
    if !artcraft_launcher_core::release::valid_file_name(&name) {
        return Err("the package has an unexpected name".into());
    }
    let sha256 = crate::http::sha256_file(file)?;
    let size = std::fs::metadata(file).map_err(|e| e.to_string())?.len();
    let root = app_dir(paths, app);
    let (current_dir, previous_dir) = (root.join(CURRENT), root.join(PREVIOUS));
    let same_version = current(paths, app).is_some_and(|(k, _)| k.version == version);
    if !same_version && current_dir.exists() {
        remove_dir(&previous_dir)?;
        std::fs::rename(&current_dir, &previous_dir).map_err(|e| format!("couldn't keep the earlier package ({e})"))?;
    }
    remove_dir(&current_dir)?;
    std::fs::create_dir_all(&current_dir).map_err(|e| format!("couldn't create {} ({e})", current_dir.display()))?;
    move_file(file, &current_dir.join(&name))?;
    let info = serde_json::to_vec_pretty(&Kept { version: version.to_string(), kind, file: name, sha256, size }).map_err(|e| e.to_string())?;
    crate::paths::write_atomic(&current_dir.join(INFO), &info)
}

/// After a roll back: the previous slot becomes the current one and the other way round.
pub fn swap(paths: &Paths, app: &str) -> Result<(), String> {
    let root = app_dir(paths, app);
    let (current_dir, previous_dir, swapping) = (root.join(CURRENT), root.join(PREVIOUS), root.join("swapping"));
    remove_dir(&swapping)?;
    let rename = |a: &Path, b: &Path| std::fs::rename(a, b).map_err(|e| format!("couldn't swap the kept packages ({e})"));
    if current_dir.exists() {
        rename(&current_dir, &swapping)?;
    }
    if previous_dir.exists() {
        rename(&previous_dir, &current_dir)?;
    }
    if swapping.exists() {
        rename(&swapping, &previous_dir)?;
    }
    Ok(())
}

/// Forgets every package kept for `app` (it was uninstalled).
pub fn forget(paths: &Paths, app: &str) {
    if let Err(e) = remove_dir(&app_dir(paths, app)) {
        log::warn!("{app}: {e}");
    }
}

fn remove_dir(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("couldn't remove {} ({e})", dir.display())),
    }
}

/// A rename, or a copy and a delete across file systems (the cache and data folders may differ).
fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to).map_err(|e| format!("couldn't keep the package ({e})"))?;
    let _ = std::fs::remove_file(from);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn each_update_keeps_the_package_before_it() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let downloads = paths.downloads();
        assert_eq!(previous(&paths, "photocraft"), None);

        keep(&paths, "photocraft", "0.5.0", PackageKind::PortableZip, &package(&downloads, "photocraft-0.5.0.zip", b"five")).unwrap();
        assert_eq!(previous(&paths, "photocraft"), None);
        assert_eq!(current(&paths, "photocraft").unwrap().0.version, "0.5.0");

        let six = package(&downloads, "photocraft-0.6.0.zip", b"six");
        keep(&paths, "photocraft", "0.6.0", PackageKind::PortableZip, &six).unwrap();
        assert!(!six.exists(), "the download moved into the kept folder");
        let (kept, file) = previous(&paths, "photocraft").unwrap();
        assert_eq!((kept.version.as_str(), kept.size), ("0.5.0", 4));
        assert_eq!(kept.sha256, crate::http::sha256_file(&file).unwrap());
        assert_eq!(std::fs::read(file).unwrap(), b"five");

        // Reinstalling 0.6.0 keeps 0.5.0 as the one to roll back to.
        keep(&paths, "photocraft", "0.6.0", PackageKind::PortableZip, &package(&downloads, "photocraft-0.6.0.zip", b"six")).unwrap();
        assert_eq!(previous(&paths, "photocraft").unwrap().0.version, "0.5.0");
    }

    #[test]
    fn rolling_back_swaps_the_slots_and_uninstalling_forgets_them() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let downloads = paths.downloads();
        keep(&paths, "filmcraft", "0.5.0", PackageKind::Msi, &package(&downloads, "filmcraft-0.5.0.msi", b"5")).unwrap();
        keep(&paths, "filmcraft", "0.6.0", PackageKind::Msi, &package(&downloads, "filmcraft-0.6.0.msi", b"6")).unwrap();
        swap(&paths, "filmcraft").unwrap();
        assert_eq!(current(&paths, "filmcraft").unwrap().0.version, "0.5.0");
        assert_eq!(previous(&paths, "filmcraft").unwrap().0.version, "0.6.0");
        forget(&paths, "filmcraft");
        assert_eq!(previous(&paths, "filmcraft"), None);
        assert!(!paths.packages().join("filmcraft").exists());
    }

    #[test]
    fn a_damaged_slot_offers_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let slot = paths.packages().join("photocraft").join(PREVIOUS);
        std::fs::create_dir_all(&slot).unwrap();
        std::fs::write(slot.join(INFO), b"{not json").unwrap();
        assert_eq!(previous(&paths, "photocraft"), None);
        let info = Kept { version: "0.5.0".into(), kind: PackageKind::Msi, file: "../escape.msi".into(), sha256: String::new(), size: 0 };
        std::fs::write(slot.join(INFO), serde_json::to_vec(&info).unwrap()).unwrap();
        assert_eq!(previous(&paths, "photocraft"), None);
        let info = Kept { file: "gone.msi".into(), ..info };
        std::fs::write(slot.join(INFO), serde_json::to_vec(&info).unwrap()).unwrap();
        assert_eq!(previous(&paths, "photocraft"), None);
    }
}
