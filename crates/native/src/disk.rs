//! How much room the Craft Apps and the launcher's own files take, and clearing the cache.
//!
//! Measuring never follows a symbolic link and stops after [`MAX_ENTRIES`] files, so a link
//! loop or a huge folder can't hang the worker thread. Clearing only ever removes what the
//! launcher put in its own cache folder: downloads and the details pictures.

use std::path::{Path, PathBuf};

use artcraft_launcher_core::protocol::DiskUsage;
use artcraft_launcher_core::{CraftApp, InstallSource, Installed};

use crate::manifest::Record;
use crate::paths::Paths;

/// The most files one measurement looks at.
const MAX_ENTRIES: usize = 500_000;

/// Bytes under `path`: the file itself, or everything inside the folder, without following links.
pub fn size_of(path: &Path) -> u64 {
    let mut budget = MAX_ENTRIES;
    size_within(path, &mut budget)
}

fn size_within(path: &Path, budget: &mut usize) -> u64 {
    let Some(left) = budget.checked_sub(1) else { return 0 };
    *budget = left;
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(path) else { return 0 };
    let mut total: u64 = 0;
    for entry in entries.flatten() {
        if *budget == 0 {
            break;
        }
        total = total.saturating_add(size_within(&entry.path(), budget));
    }
    total
}

/// What to measure for an installed app, when the launcher can tell: the folder or file it
/// lives in. Apps owned by a package manager (deb, rpm, pacman, Flatpak) or found on `PATH` are
/// spread over system folders the launcher doesn't own, so they have no size here.
pub fn installed_paths(app: &CraftApp, installed: &Installed, record: Option<&Record>, paths: &Paths) -> Option<Vec<PathBuf>> {
    if let Some(record) = record.filter(|r| !r.files.is_empty()) {
        return Some(record.files.clone());
    }
    let location = PathBuf::from(&installed.location);
    match installed.source {
        // A tarball unpacked into the launcher's own folder: the whole folder (its home included).
        InstallSource::Launcher if location.starts_with(paths.apps()) => Some(vec![paths.apps().join(app.id)]),
        // The exe of an MSI or portable install: the folder it sits in.
        InstallSource::Msi => location.parent().map(|dir| vec![dir.to_path_buf()]),
        InstallSource::Launcher if location.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) => {
            location.parent().map(|dir| vec![dir.to_path_buf()])
        }
        InstallSource::Launcher | InstallSource::MacApp | InstallSource::AppImage => Some(vec![location]),
        InstallSource::Deb | InstallSource::Rpm | InstallSource::Pacman | InstallSource::Flatpak | InstallSource::Binary => None,
    }
}

/// Measures every installed app and the launcher's cache.
pub fn measure(installed: &[(&'static CraftApp, Installed, Option<Record>)], paths: &Paths) -> DiskUsage {
    let mut usage = DiskUsage::default();
    for (app, inst, record) in installed {
        if let Some(list) = installed_paths(app, inst, record.as_ref(), paths) {
            let bytes = list.iter().fold(0u64, |sum, p| sum.saturating_add(size_of(p)));
            usage.apps.insert(app.id.to_string(), bytes);
        }
    }
    usage.downloads = size_of(&paths.downloads());
    usage.pictures = size_of(&paths.screenshots());
    usage.cache = size_of(&paths.cache);
    usage.kept = crate::keep::total_size(paths);
    usage
}

/// Empties the download folder and the saved pictures. Returns the bytes freed. Anything that
/// can't be removed (a file another program holds open) is left and skipped over.
pub fn clear_cache(paths: &Paths) -> Result<u64, String> {
    let mut freed: u64 = 0;
    let mut failed = Vec::new();
    for dir in [paths.downloads(), paths.screenshots()] {
        // Only what sits inside the launcher's cache folder: never a path from elsewhere.
        if !dir.starts_with(&paths.cache) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let bytes = size_of(&path);
            let removed = match std::fs::symlink_metadata(&path) {
                Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&path),
                Ok(_) => std::fs::remove_file(&path),
                Err(e) => Err(e),
            };
            match removed {
                Ok(()) => freed = freed.saturating_add(bytes),
                Err(e) => {
                    log::warn!("couldn't remove {} ({e})", path.display());
                    failed.push(path.display().to_string());
                }
            }
        }
    }
    if failed.is_empty() || freed > 0 { Ok(freed) } else { Err(format!("couldn't remove {}", failed.join(", "))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_add_up_and_links_are_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/one"), vec![0u8; 100]).unwrap();
        std::fs::write(dir.path().join("a/b/two"), vec![0u8; 50]).unwrap();
        assert_eq!(size_of(&dir.path().join("a")), 150);
        assert_eq!(size_of(&dir.path().join("a/one")), 100);
        assert_eq!(size_of(&dir.path().join("missing")), 0);
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::fs::write(outside.path().join("big"), vec![0u8; 4096]).unwrap();
            std::os::unix::fs::symlink(outside.path(), dir.path().join("a/link")).unwrap();
            std::os::unix::fs::symlink(dir.path().join("a"), dir.path().join("a/b/loop")).unwrap();
            assert_eq!(size_of(&dir.path().join("a")), 150);
        }
    }

    #[test]
    fn a_huge_folder_is_measured_only_up_to_the_budget() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..10 {
            std::fs::write(dir.path().join(format!("f{i}")), [0u8; 10]).unwrap();
        }
        let mut budget = 4;
        assert!(size_within(dir.path(), &mut budget) <= 30);
        assert_eq!(budget, 0);
    }

    #[test]
    fn clearing_empties_downloads_and_pictures_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        std::fs::create_dir_all(paths.downloads()).unwrap();
        std::fs::write(paths.downloads().join("photocraft.msi"), vec![0u8; 300]).unwrap();
        std::fs::write(paths.downloads().join("filmcraft.msi.10.part"), vec![0u8; 10]).unwrap();
        let shot = paths.screenshot("photocraft", "lead");
        std::fs::create_dir_all(shot.parent().unwrap()).unwrap();
        std::fs::write(&shot, vec![0u8; 20]).unwrap();
        std::fs::write(paths.release_cache(), b"{}").unwrap();
        std::fs::create_dir_all(&paths.data).unwrap();
        std::fs::write(paths.manifest(), b"{}").unwrap();

        let before = measure(&[], &paths);
        assert_eq!(before.downloads, 310);
        assert_eq!(before.pictures, 20);
        assert_eq!(before.cache, 332);

        assert_eq!(clear_cache(&paths), Ok(330));
        let after = measure(&[], &paths);
        assert_eq!((after.downloads, after.pictures, after.cache), (0, 0, 2));
        assert!(paths.release_cache().exists());
        assert!(paths.manifest().exists());
        // Nothing left to clear is not an error.
        assert_eq!(clear_cache(&paths), Ok(0));
    }

    #[test]
    fn system_packages_have_no_size_and_launcher_installs_do() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let app = CraftApp::by_id("photocraft").unwrap();
        let deb = Installed { version: None, source: InstallSource::Deb, location: "/usr/bin/photocraft".into(), package: None, upstream: false };
        assert_eq!(installed_paths(app, &deb, None, &paths), None);
        let appimage = dir.path().join("PhotoCraft.AppImage");
        std::fs::write(&appimage, vec![0u8; 77]).unwrap();
        let inst =
            Installed { version: None, source: InstallSource::AppImage, location: appimage.display().to_string(), package: None, upstream: false };
        let usage = measure(&[(app, inst, None)], &paths);
        assert_eq!(usage.apps.get("photocraft"), Some(&77));
        let exe = Installed {
            version: None,
            source: InstallSource::Msi,
            location: r"C:\Apps\PhotoCraft\photocraft.exe".into(),
            package: None,
            upstream: false,
        };
        assert!(installed_paths(app, &exe, None, &paths).is_some());
    }
}
