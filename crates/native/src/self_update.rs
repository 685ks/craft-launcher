//! Arthavan Projects updating itself.
//!
//! The new version is downloaded and verified like any Craft App, then swapped in the way this
//! copy was installed. A small helper process waits for this launcher to exit, finishes anything
//! that can't happen while it runs (Windows locks a running exe), and starts the new one. The
//! launcher has to exit first anyway: it holds the single-instance lock, and a new copy started
//! while it runs would only ask it to show its window.
//!
//! | Installed as | Swap |
//! |---|---|
//! | Windows MSI (per-user, any folder) | after exit, run the new MSI (it reinstalls into the same folder), then start the exe |
//! | Windows portable zip | after exit, unzip over this folder (settings in `ArthavanProjectsData` stay) |
//!
//! Anything else (another system, a development build) gets the download link instead: the
//! launcher supports Windows only.

use std::path::{Path, PathBuf};

use artcraft_launcher_core::PackageKind;
use artcraft_launcher_core::catalog::LAUNCHER;
use artcraft_launcher_core::platform::{Platform, pick_asset};
use artcraft_launcher_core::protocol::JobPhase;
use artcraft_launcher_core::release::Release;
use artcraft_launcher_core::version::{self, Version};

use crate::http::{Http, Stop};
use crate::install::{self, Job};
use crate::paths::Paths;
#[cfg(windows)]
use crate::process;

/// How this running launcher was installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Install {
    /// The per-user Windows Installer package; the exe.
    Msi(PathBuf),
    /// The Windows portable zip; the exe (with `portable.txt` beside it).
    Portable(PathBuf),
}

impl Install {
    fn kind(&self) -> PackageKind {
        match self {
            Self::Msi(_) => PackageKind::Msi,
            Self::Portable(_) => PackageKind::PortableZip,
        }
    }
}

/// Makes an installed copy believe it's this version, to try the self-update against the current
/// release. The restart helper drops it, so it lasts one run.
const PRETEND_VAR: &str = "ARTHAVAN_PROJECTS_PRETEND_VERSION";

/// The version this launcher counts as (see [`PRETEND_VAR`]).
pub fn current_version() -> Option<Version> {
    std::env::var(PRETEND_VAR).ok().and_then(|v| version::parse(&v)).or_else(|| version::parse(env!("CARGO_PKG_VERSION")))
}

/// How long a just-installed version stays exempt from being offered again.
const JUST_UPDATED_FOR: i64 = 24 * 3600;

/// Whether `release` is the version this launcher just updated itself to. Offering it again would
/// loop: update, restart, "update available", … That happens if a release's binary reports a
/// different version from its tag, or while a pretend version is set.
pub fn just_updated_to(paths: &Paths, release: &Release) -> bool {
    let Ok(text) = std::fs::read_to_string(marker(paths)) else { return false };
    let mut lines = text.lines();
    let (Some(tag), Some(at)) = (lines.next(), lines.next().and_then(|a| a.parse::<i64>().ok())) else { return false };
    tag == release.tag && crate::now().checked_sub(at).is_some_and(|age| (0..JUST_UPDATED_FOR).contains(&age))
}

fn marker(paths: &Paths) -> PathBuf {
    paths.data.join("self-update.txt")
}

/// How this copy is installed, if it's one the launcher knows how to replace.
pub fn current() -> Option<Install> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    detect(&exe)
}

#[cfg(windows)]
fn detect(exe: &Path) -> Option<Install> {
    let dir = exe.parent()?;
    if dir.join("portable.txt").is_file() {
        return Some(Install::Portable(exe.to_path_buf()));
    }
    // The MSI installs per user, into the folder chosen in its wizard (which it records in the
    // registry) or by default %LOCALAPPDATA%\Programs\Arthavan Projects.
    let canonical = |p: PathBuf| p.canonicalize().unwrap_or(p);
    let recorded = msi_install_dir().map(canonical);
    let default = dirs::data_local_dir().map(|d| canonical(d.join("Programs").join(LAUNCHER.name)));
    [recorded, default].into_iter().flatten().any(|d| d == dir).then(|| Install::Msi(exe.to_path_buf()))
}

/// The folder the MSI installed into, as it records it (`packaging/windows/arthavan-projects.wxs`).
#[cfg(windows)]
fn msi_install_dir() -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Arthavan Projects").ok()?;
    let dir: String = key.get_value("InstallDir").ok()?;
    let dir = dir.trim();
    (!dir.is_empty()).then(|| PathBuf::from(dir))
}

/// Elsewhere there is nothing to update: the launcher supports Windows only.
#[cfg(not(windows))]
fn detect(_exe: &Path) -> Option<Install> {
    None
}

/// Downloads `release` for `platform`, puts it in place and starts the helper that relaunches
/// it once this process has exited. On success the caller must quit promptly. `last` is called
/// once the download is ready and returns when the launcher may replace itself and restart.
/// Downloads and verifies the newer launcher (signed checksums when the release has them) without
/// putting it in place: [`update`] then finds it ready and skips the download.
pub fn stage(
    install: &Install,
    release: &Release,
    platform: Platform,
    http: &Http,
    paths: &Paths,
    stop: &Stop,
    phase: &mut dyn FnMut(JobPhase),
) -> Result<(), String> {
    if let (Some(rel_v), Some(cur_v)) = (release.version.as_ref(), current_version().as_ref())
        && !version::is_newer(rel_v, cur_v)
    {
        return Err(format!("Arthavan Projects {} is not newer than currently running version {}", release.version_label(), cur_v));
    }
    download(install, release, platform, http, paths, stop, phase).map(|_| ())
}

/// The launcher's own release file for `install`, downloaded (or found already downloaded) and verified.
fn download(
    install: &Install,
    release: &Release,
    platform: Platform,
    http: &Http,
    paths: &Paths,
    stop: &Stop,
    phase: &mut dyn FnMut(JobPhase),
) -> Result<PathBuf, String> {
    let (asset, _) = pick_asset(release, &[LAUNCHER.id], platform, &[install.kind()])
        .ok_or_else(|| format!("Arthavan Projects {} has no {} for {}", release.version_label(), install.kind().label(), platform.label()))?;
    let asset = asset.clone();
    let mut job = Job { http, paths, stop, phase, install_locations: &Default::default(), replace: false, local: None, keep: false };
    install::download(release, &asset, &mut job)
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    install: &Install,
    release: &Release,
    platform: Platform,
    http: &Http,
    paths: &Paths,
    stop: &Stop,
    phase: &mut dyn FnMut(JobPhase),
    last: &dyn Fn() -> Result<(), String>,
) -> Result<(), String> {
    if let (Some(rel_v), Some(cur_v)) = (release.version.as_ref(), current_version().as_ref())
        && !version::is_newer(rel_v, cur_v)
    {
        return Err(format!("Arthavan Projects {} is not newer than currently running version {}", release.version_label(), cur_v));
    }
    let file = download(install, release, platform, http, paths, stop, phase)?;
    // The restart ends this process: nothing else may be under way.
    last()?;
    phase(JobPhase::Installing);
    if let Err(e) = crate::paths::write_atomic(&marker(paths), format!("{}\n{}\n", release.tag, crate::now()).as_bytes()) {
        log::warn!("couldn't record the self-update: {e}");
    }
    let pid = std::process::id();
    match install {
        #[cfg(windows)]
        Install::Msi(exe) => {
            crate::authenticode::verify(&install::msi_path(&file)?)?;
            windows_helper(pid, &msi_update_step(&file)?, exe)
        }
        #[cfg(windows)]
        Install::Portable(exe) => {
            let dir = exe.parent().ok_or("the launcher isn't in a folder")?;
            let tmp = paths.cache.join("launcher-update");
            windows_helper(
                pid,
                &format!(
                    "$t = {tmp}; Remove-Item -LiteralPath $t -Recurse -Force -ErrorAction SilentlyContinue; Expand-Archive -LiteralPath {zip} -DestinationPath $t -Force; \
                     $top = Get-ChildItem -LiteralPath $t | Select-Object -First 1; Copy-Item -Path (Join-Path $top.FullName '*') -Destination {dir} -Recurse -Force; \
                     Remove-Item -LiteralPath $t -Recurse -Force -ErrorAction SilentlyContinue; Remove-Item -LiteralPath {zip} -ErrorAction SilentlyContinue",
                    tmp = ps_quote(&tmp),
                    zip = ps_quote(&file),
                    dir = ps_quote(dir),
                ),
                exe,
            )
        }
        #[cfg(not(windows))]
        _ => {
            let _ = (pid, &file, paths);
            Err("Arthavan Projects updates itself on Windows only; download the new version instead".into())
        }
    }
}

/// Starts a hidden PowerShell that waits for `pid` to exit, runs `step`, then starts `exe`.
#[cfg(windows)]
fn windows_helper(pid: u32, step: &str, exe: &Path) -> Result<(), String> {
    let script = format!("Wait-Process -Id {pid} -Timeout 60 -ErrorAction SilentlyContinue; {step}; Start-Process -FilePath {}", ps_quote(exe));
    let mut cmd = process::command("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-Command", &script]).env_remove(PRETEND_VAR);
    process::spawn_detached(cmd).map_err(|e| format!("couldn't schedule the update ({e})"))
}

/// A PowerShell single-quoted literal.
#[cfg_attr(not(windows), allow(dead_code))]
fn ps_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

/// A PowerShell literal for an msiexec argument: the path in double quotes, as msiexec expects.
#[cfg_attr(not(windows), allow(dead_code))]
fn ps_quote_arg(path: &Path) -> String {
    format!("'\"{}\"'", path.display().to_string().replace('\'', "''"))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn msi_update_step(file: &Path) -> Result<String, String> {
    let file = install::msi_path(file)?;
    Ok(format!(
        "Start-Process -FilePath (Join-Path $env:SystemRoot 'System32\\msiexec.exe') -ArgumentList '/i',{},'/passive','/norestart' -Wait; Remove-Item -LiteralPath {} -ErrorAction SilentlyContinue",
        ps_quote_arg(&file),
        ps_quote(&file)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn msi_self_update_uses_the_target_of_a_symlinked_download() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Launcher O'Brien.msi");
        std::fs::write(&file, b"test package").unwrap();
        let link = dir.path().join("cached.msi");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        let step = msi_update_step(&link).unwrap();
        let target = file.canonicalize().unwrap();
        assert!(step.contains(&format!("-ArgumentList '/i',{},", ps_quote_arg(&target))), "{step}");
        assert!(step.contains(&format!("Remove-Item -LiteralPath {} ", ps_quote(&target))), "{step}");
    }

    #[test]
    fn a_missing_msi_cannot_schedule_a_self_update() {
        let dir = tempfile::tempdir().unwrap();
        assert!(msi_update_step(&dir.path().join("missing.msi")).is_err());
    }

    #[test]
    fn a_corrupt_self_update_timestamp_does_not_overflow() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let release = Release { tag: "v0.2.0".into(), ..Release::default() };
        crate::paths::write_atomic(&marker(&paths), format!("v0.2.0\n{}\n", i64::MIN).as_bytes()).unwrap();
        assert!(!just_updated_to(&paths, &release));
    }

    #[test]
    fn a_future_self_update_timestamp_does_not_suppress_updates() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let release = Release { tag: "v0.2.0".into(), ..Release::default() };
        for at in [crate::now() + JUST_UPDATED_FOR, i64::MAX] {
            crate::paths::write_atomic(&marker(&paths), format!("v0.2.0\n{at}\n").as_bytes()).unwrap();
            assert!(!just_updated_to(&paths, &release), "future timestamp {at}");
        }
    }

    #[test]
    fn a_version_just_installed_is_not_offered_again() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let r = Release { tag: "v0.2.0".into(), ..Release::default() };
        assert!(!just_updated_to(&paths, &r));
        crate::paths::write_atomic(&marker(&paths), format!("v0.2.0\n{}\n", crate::now()).as_bytes()).unwrap();
        assert!(just_updated_to(&paths, &r));
        assert!(!just_updated_to(&paths, &Release { tag: "v0.3.0".into(), ..Release::default() }));
        // A day later the guard lapses.
        crate::paths::write_atomic(&marker(&paths), format!("v0.2.0\n{}\n", crate::now() - JUST_UPDATED_FOR - 1).as_bytes()).unwrap();
        assert!(!just_updated_to(&paths, &r));
    }

    #[test]
    fn powershell_quoting_survives_apostrophes() {
        let p = Path::new(r"C:\Users\O'Brien\AppData\Local\Arthavan Projects\x.msi");
        assert_eq!(ps_quote(p), r"'C:\Users\O''Brien\AppData\Local\Arthavan Projects\x.msi'");
        assert_eq!(ps_quote_arg(p), r#"'"C:\Users\O''Brien\AppData\Local\Arthavan Projects\x.msi"'"#);
    }

    #[test]
    fn a_development_build_does_not_update_itself() {
        // `cargo test` runs from target/, which no installer puts anywhere.
        assert_eq!(current(), None);
    }

    #[test]
    fn the_launcher_picks_its_own_release_files() {
        let names = [
            "arthavan-projects-0.2.0-windows-x64.msi",
            "arthavan-projects-0.2.0-windows-x64-portable.zip",
            "arthavan-projects-0.2.0-linux-x86_64.AppImage",
            "arthavan-projects-0.2.0-linux-x86_64.deb",
            "arthavan-projects-0.2.0-linux-x86_64.flatpak",
            "arthavan-projects-0.2.0-linux-x86_64.pkg.tar.zst",
            "arthavan-projects-web-0.2.0.zip",
        ];
        // release.yml also attaches each build under a stable, unversioned name
        // (`<APP_SLUG>-<platform>.<ext>`, APP_SLUG = LAUNCHER_ID): listed first, they must never be
        // picked, so self-update always gets the versioned file.
        let stable = [
            "arthavan-projects-macos-universal.dmg",
            "arthavan-projects-windows-x64.msi",
            "arthavan-projects-windows-x64.zip",
            "arthavan-projects-linux-x86_64.AppImage",
            "arthavan-projects-linux-x86_64.tar.gz",
        ];
        assert_eq!(LAUNCHER.id, artcraft_launcher_core::catalog::LAUNCHER_ID);
        let release = Release {
            assets: stable
                .iter()
                .chain(names.iter())
                .map(|n| artcraft_launcher_core::release::Asset { name: (*n).into(), size: 1, url: String::new(), sha256: None })
                .collect(),
            ..Release::default()
        };
        use artcraft_launcher_core::{Arch, Os};
        let pick = |os, kind| pick_asset(&release, &[LAUNCHER.id], Platform { os, arch: Arch::X64 }, &[kind]).map(|(a, _)| a.name.clone());
        assert_eq!(pick(Os::Windows, Install::Msi(PathBuf::new()).kind()).as_deref(), Some(names[0]));
        assert_eq!(pick(Os::Windows, Install::Portable(PathBuf::new()).kind()).as_deref(), Some(names[1]));
        // Older releases' Linux and macOS files are never picked: the launcher supports Windows only.
        for os in [Os::Linux, Os::MacOs] {
            for kind in [PackageKind::Msi, PackageKind::PortableZip, PackageKind::AppImage, PackageKind::Deb, PackageKind::Dmg] {
                assert_eq!(pick(os, kind), None, "{os:?} {kind:?}");
            }
        }
    }

    #[test]
    fn update_refuses_to_downgrade_when_release_is_older() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let http = Http::new().unwrap();
        let mut phase = |_| ();
        let last = || Ok(());
        let stop = Stop::default();
        let release = Release {
            version: version::parse("0.1.0"),
            tag: "v0.1.0".into(),
            assets: vec![artcraft_launcher_core::release::Asset {
                name: "arthavan-projects-0.1.0-windows-x64-portable.zip".into(),
                size: 1,
                url: String::new(),
                sha256: None,
            }],
            ..Release::default()
        };
        let install = Install::Portable(dir.path().join("arthavan-projects.exe"));
        use artcraft_launcher_core::{Arch, Os};
        let platform = Platform { os: Os::Windows, arch: Arch::X64 };
        let result = update(&install, &release, platform, &http, &paths, &stop, &mut phase, &last);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("is not newer than currently running version"));
    }
}
