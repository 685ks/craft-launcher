//! Finding Craft Apps that are already installed, however they got there.
//!
//! On Windows: the Windows Installer's uninstall entries (publisher `Arthavan Projects` for the
//! forks' builds, `Learning Machines LLC` for upstream ones), and portable copies the launcher
//! unpacked. The launcher supports Windows only, so elsewhere it finds nothing but the copies its
//! own records list.

use std::path::{Path, PathBuf};

use artcraft_launcher_core::{CraftApp, InstallSource, Installed, version};

use crate::manifest::Manifest;

/// One look at the system, shared by every app's detection. Windows has nothing to read up
/// front (each app's uninstall entries are looked up on their own), so it is empty for now.
#[derive(Default)]
pub struct Snapshot {}

impl Snapshot {
    pub fn take() -> Self {
        Self::default()
    }
}

/// What's installed of `app`, best source first.
pub fn detect(app: &CraftApp, manifest: &Manifest, snapshot: &Snapshot) -> Option<Installed> {
    let _ = snapshot;
    if let Some(found) = from_manifest(app, manifest) {
        return Some(found);
    }
    #[cfg(windows)]
    return windows::detect(app);
    #[cfg(not(windows))]
    None
}

fn from_manifest(app: &CraftApp, manifest: &Manifest) -> Option<Installed> {
    let rec = manifest.apps.get(app.id)?;
    if !rec.path.exists() {
        return None;
    }
    Some(Installed {
        version: version::parse(&rec.version),
        source: InstallSource::Launcher,
        location: rec.path.display().to_string(),
        package: None,
        upstream: false,
    })
}

/// Who the forks' Windows Installer packages name as their publisher (`packaging/app.toml`).
#[cfg_attr(not(windows), allow(dead_code))]
const PUBLISHER: &str = "Arthavan Projects";
/// The publisher upstream (`storytold/<app>`) MSIs register: still recognised, as upstream installs.
#[cfg_attr(not(windows), allow(dead_code))]
const UPSTREAM_PUBLISHER: &str = "Learning Machines LLC";

/// Whether an uninstall entry's `DisplayName` and `Publisher` are `app`'s, under any name its
/// installers have registered: `Some(upstream)` when they are.
#[cfg_attr(not(windows), allow(dead_code))]
fn installer_entry(app: &CraftApp, display_name: &str, publisher: &str) -> Option<bool> {
    let upstream = match publisher {
        PUBLISHER => false,
        UPSTREAM_PUBLISHER => true,
        _ => return None,
    };
    app.names().any(|n| n == display_name).then_some(upstream)
}

#[cfg(test)]
fn is_installer_entry(app: &CraftApp, display_name: &str, publisher: &str) -> bool {
    installer_entry(app, display_name, publisher).is_some()
}

/// Where a Windows install of `app` may keep its exe, in the order to try: the entry's
/// `InstallLocation`, then `<App>` under each Program Files root for each name the app has had,
/// and in each folder `<stem>.exe` for each stem, current first. Doesn't touch the disk.
#[cfg_attr(not(windows), allow(dead_code))]
fn exe_candidates(app: &CraftApp, location: Option<&Path>, program_files: &[PathBuf]) -> Vec<PathBuf> {
    let in_program_files = program_files.iter().flat_map(|root| app.names().map(move |n| root.join(n)));
    let dirs = location.map(Path::to_path_buf).into_iter().chain(in_program_files);
    dirs.flat_map(|d| app.stems().map(move |s| d.join(format!("{s}.exe")))).collect()
}

#[cfg(windows)]
pub mod windows {
    use std::path::PathBuf;

    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};

    use super::*;

    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";

    /// Reads VERSIONINFO without executing the downloaded application. The path is data in
    /// a child-only environment variable, never interpolated into PowerShell source.
    pub fn executable_version(path: &std::path::Path) -> Option<artcraft_launcher_core::version::Version> {
        let path = std::fs::canonicalize(path).ok()?;
        let mut command = crate::process::command("powershell.exe");
        command.args([
            "-NoLogo", "-NoProfile", "-NonInteractive", "-Command",
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; [System.Diagnostics.FileVersionInfo]::GetVersionInfo($env:ARTHAVAN_PROJECTS_VERSION_FILE).ProductVersion",
        ]).env("ARTHAVAN_PROJECTS_VERSION_FILE", path);
        let output = crate::process::run_command(command, "powershell.exe", std::time::Duration::from_secs(5)).ok()?;
        version::parse(output.trim())
    }

    /// An MSI install found in the registry.
    pub struct MsiInstall {
        pub product_code: String,
        pub version: Option<artcraft_launcher_core::version::Version>,
        pub exe: Option<PathBuf>,
        /// Registered by an upstream build's installer.
        pub upstream: bool,
    }

    /// Every Windows Installer entry for `app`, newest first.
    pub fn msi_installs(app: &CraftApp) -> Vec<MsiInstall> {
        let mut out = Vec::new();
        for (hive, flags) in [(HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY), (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY), (HKEY_CURRENT_USER, 0)] {
            let Ok(root) = RegKey::predef(hive).open_subkey_with_flags(UNINSTALL, KEY_READ | flags) else { continue };
            for code in root.enum_keys().flatten() {
                let Ok(key) = root.open_subkey_with_flags(&code, KEY_READ | flags) else { continue };
                let name: String = key.get_value("DisplayName").unwrap_or_default();
                let publisher: String = key.get_value("Publisher").unwrap_or_default();
                let Some(upstream) = installer_entry(app, &name, &publisher) else { continue };
                if out.iter().any(|m: &MsiInstall| m.product_code == code) {
                    continue;
                }
                let version = key.get_value::<String, _>("DisplayVersion").ok().and_then(|v| version::parse(&v));
                let location: String = key.get_value("InstallLocation").unwrap_or_default();
                let exe = exe_path(app, (!location.is_empty()).then(|| PathBuf::from(location)).or_else(|| install_dir(app)));
                out.push(MsiInstall { product_code: code, version, exe, upstream });
            }
        }
        // The forks' own copy first, then the newest.
        out.sort_by(|a, b| {
            a.upstream.cmp(&b.upstream).then(match (&b.version, &a.version) {
                (Some(x), Some(y)) => version::compare(x, y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
        });
        out
    }

    /// The folder the app's MSI remembers for its next upgrade, which is where it went when the
    /// launcher chose the folder.
    fn install_dir(app: &CraftApp) -> Option<PathBuf> {
        [KEY_WOW64_64KEY, KEY_WOW64_32KEY].into_iter().find_map(|flags| {
            let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(format!(r"SOFTWARE\{}", app.name), KEY_READ | flags).ok()?;
            key.get_value::<String, _>("InstallDir").ok().filter(|d| !d.is_empty()).map(PathBuf::from)
        })
    }

    /// The app's exe: from `App Paths`, the install folder, or Program Files.
    fn exe_path(app: &CraftApp, location: Option<PathBuf>) -> Option<PathBuf> {
        for stem in app.stems() {
            let app_paths = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{stem}.exe");
            for flags in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
                if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(&app_paths, KEY_READ | flags)
                    && let Ok(path) = key.get_value::<String, _>("")
                {
                    let p = PathBuf::from(path.trim_matches('"'));
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
        let mut roots = Vec::new();
        for var in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(pf) = std::env::var_os(var) {
                roots.push(PathBuf::from(pf));
            }
        }
        exe_candidates(app, location.as_deref(), &roots).into_iter().find(|p| p.exists())
    }

    pub fn detect(app: &CraftApp) -> Option<Installed> {
        let m = msi_installs(app).into_iter().next()?;
        Some(Installed {
            version: m.version,
            source: InstallSource::Msi,
            location: m.exe.map_or_else(|| app.name.to_string(), |p| p.display().to_string()),
            package: Some(m.product_code),
            upstream: m.upstream,
        })
    }
}

#[cfg(test)]
mod tests {
    use artcraft_launcher_core::SUITE;

    use super::*;

    const LEARNING_MACHINES: &str = "Learning Machines LLC";

    #[test]
    fn the_forks_msis_and_upstream_msis_are_both_found_and_told_apart() {
        let photo = CraftApp::by_id("photocraft").unwrap();
        assert_eq!(installer_entry(photo, "PhotoCraft", "Arthavan Projects"), Some(false));
        assert_eq!(installer_entry(photo, "PhotoCraft", LEARNING_MACHINES), Some(true));
        assert_eq!(installer_entry(photo, "PhotoCraft", "Someone Else"), None);
        assert_eq!(installer_entry(photo, "WordCraft", "Arthavan Projects"), None);
    }

    fn roots() -> Vec<PathBuf> {
        vec![PathBuf::from(r"C:\Program Files"), PathBuf::from(r"C:\Program Files (x86)")]
    }

    #[test]
    fn pdfcraft_matches_its_former_printcraft_installer_name() {
        let pdf = CraftApp::by_id("pdfcraft").unwrap();
        assert!(is_installer_entry(pdf, "PrintCraft", LEARNING_MACHINES), "a PrintCraft entry from {LEARNING_MACHINES} should be PdfCraft's");
    }

    #[test]
    fn pdfcraft_still_matches_its_own_installer_name() {
        let pdf = CraftApp::by_id("pdfcraft").unwrap();
        assert!(is_installer_entry(pdf, "PdfCraft", LEARNING_MACHINES));
    }

    #[test]
    fn printcraft_entries_match_nothing_else() {
        let pdf = CraftApp::by_id("pdfcraft").unwrap();
        assert!(!is_installer_entry(pdf, "PrintCraft", "Some Other Publisher"));
        assert!(!is_installer_entry(pdf, "PhotoCraft", LEARNING_MACHINES));
        for a in SUITE.iter().filter(|a| a.id != "pdfcraft") {
            assert!(!is_installer_entry(a, "PrintCraft", LEARNING_MACHINES), "PrintCraft matched {}", a.name);
        }
    }

    #[test]
    fn pdfcraft_exe_is_sought_under_every_name_it_has_shipped_as() {
        let pdf = CraftApp::by_id("pdfcraft").unwrap();
        let location = PathBuf::from(r"D:\Apps\PrintCraft");
        let roots = roots();
        let mut want = vec![location.join("pdfcraft.exe"), location.join("printcraft.exe")];
        for root in &roots {
            for dir in ["PdfCraft", "PrintCraft"] {
                want.push(root.join(dir).join("pdfcraft.exe"));
                want.push(root.join(dir).join("printcraft.exe"));
            }
        }
        assert_eq!(exe_candidates(pdf, Some(&location), &roots), want);
    }

    #[test]
    fn apps_without_former_names_match_and_find_their_exe_as_before() {
        let roots = roots();
        let location = PathBuf::from(r"D:\Apps\Somewhere");
        for a in SUITE.iter().filter(|a| a.former_ids.is_empty() && a.former_names.is_empty()) {
            assert!(is_installer_entry(a, a.name, LEARNING_MACHINES), "{}", a.name);
            assert!(!is_installer_entry(a, a.name, "Some Other Publisher"), "{}", a.name);
            assert!(!is_installer_entry(a, &a.name.to_lowercase(), LEARNING_MACHINES), "{}", a.name);
            let exe = format!("{}.exe", a.id);
            let in_program_files: Vec<PathBuf> = roots.iter().map(|r| r.join(a.name).join(&exe)).collect();
            assert_eq!(exe_candidates(a, None, &roots), in_program_files);
            let with_location: Vec<PathBuf> = std::iter::once(location.join(&exe)).chain(in_program_files).collect();
            assert_eq!(exe_candidates(a, Some(&location), &roots), with_location);
        }
    }
}
