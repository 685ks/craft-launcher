//! Installing, updating, removing and starting Craft Apps.
//!
//! The launcher installs Windows packages only: the release MSI (Windows Installer) into
//! `C:\Program Files\<App>` or a chosen folder, or the portable ZIP. On every other system
//! [`available_kinds`] is empty and installing reports that the platform isn't supported.
//!
//! Every download is checked against its published SHA-256 before anything is unpacked, a
//! running app is never replaced, and a new copy is fully in place before the old one goes.

use std::path::{Path, PathBuf};

use artcraft_launcher_core::launch::LaunchOptions;
use artcraft_launcher_core::protocol::{CANCELLED, INSTALL_CANCELLED, InstallLocations, JobPhase};
use artcraft_launcher_core::release::{self, Asset, Release};
use artcraft_launcher_core::{CraftApp, InstallSource, Installed, PackageKind, version};

use crate::http::{Http, Stop, Transfer};
use crate::manifest::Record;
use crate::paths::Paths;
use crate::process;

/// What an install needs from the backend.
pub struct Job<'a> {
    pub http: &'a Http,
    pub paths: &'a Paths,
    pub stop: &'a Stop,
    pub phase: &'a mut dyn FnMut(JobPhase),
    pub install_locations: &'a InstallLocations,
    /// Write over whatever [`occupied`] found.
    pub replace: bool,
    /// Install this file (a package kept for rolling back, already checked) instead of
    /// downloading the asset. It is left where it is afterwards.
    pub local: Option<PathBuf>,
    /// After a successful install, keep the downloaded file for rolling back ([`crate::keep`])
    /// instead of deleting it.
    pub keep: bool,
}

/// Downloads and installs `asset` of `release`. Returns a record when the launcher owns the
/// result (portable ZIP); MSI installs are found in the registry.
/// `previous` is the launcher's record of the copy being replaced, if it installed it.
pub fn install(
    app: &CraftApp,
    release: &Release,
    asset: &Asset,
    kind: PackageKind,
    existing: Option<&Installed>,
    previous: Option<&Record>,
    job: &mut Job<'_>,
) -> Result<Option<Record>, String> {
    install_at(app, release, asset, kind, existing, previous, job, None)
}

type RecordCommit<'a> = Option<&'a mut dyn FnMut(&Record) -> Result<(), String>>;

#[allow(clippy::too_many_arguments)]
pub fn install_at(
    app: &CraftApp,
    release: &Release,
    asset: &Asset,
    kind: PackageKind,
    existing: Option<&Installed>,
    previous: Option<&Record>,
    job: &mut Job<'_>,
    commit: RecordCommit<'_>,
) -> Result<Option<Record>, String> {
    #[cfg(windows)]
    let mut commit = commit;
    #[cfg(not(windows))]
    let _ = commit;
    #[allow(unused_variables)]
    let destination = destination(app, kind, existing, previous, job.install_locations)?;
    if let Some(inst) = existing
        && inst.source.launcher_can_update()
        && process::is_running(Path::new(&inst.location))
    {
        return Err(format!("{} is running. Quit it, then try again.", app.name));
    }
    if !job.replace
        && let Some(inst) = existing
        && let (Some(rel_v), Some(inst_v)) = (release.version.as_ref(), inst.version.as_ref())
        && version::is_newer(inst_v, rel_v)
    {
        return Err(format!("Cannot update {} to {}: installed version {} is newer", app.name, release.version_label(), inst_v));
    }
    log::info!("{}: starting {} to {} ({})", app.name, if existing.is_some() { "update" } else { "install" }, release.version_label(), kind.label());
    let file = match &job.local {
        Some(file) => file.clone(),
        None => download(release, asset, job)?,
    };
    log::info!("{}: download ready ({}), running installer...", app.name, file.display());
    // Past this point the install can't be interrupted halfway.
    (job.phase)(JobPhase::Installing);
    let version = release.version_label();
    let result = match kind {
        #[cfg(windows)]
        PackageKind::Msi => windows::run_msi(&file, destination.as_deref(), job.stop, &mut *job.phase).map(|_| None),
        #[cfg(windows)]
        PackageKind::PortableZip => {
            crate::portable::install(app, &file, &version, previous, job.paths, destination.as_deref(), job.replace, &mut |record| {
                if release.version.is_some() && crate::detect::windows::executable_version(&record.path).as_ref() != release.version.as_ref() {
                    return Err("The portable executable does not match the selected version. The installation was rolled back.".into());
                }
                match commit.as_deref_mut() {
                    Some(commit) => commit(record),
                    None => Ok(()),
                }
            })
            .map(Some)
        }
        other => Err(format!("{} isn't supported on this system: the launcher installs Windows packages only", other.label())),
    };
    match &result {
        Ok(_) => log::info!("{}: successfully installed/updated to {}", app.name, release.version_label()),
        Err(e) if e == CANCELLED => log::info!("{}: installation cancelled", app.name),
        Err(e) => log::error!("{}: installation failed: {e}", app.name),
    }
    // A failed install keeps the download: the next try checks it against its published checksum
    // again and skips the transfer.
    if result.is_ok() && job.local.is_none() {
        let kept = job.keep && crate::keep::can_roll_back(kind);
        let kept = kept
            && crate::keep::keep(job.paths, app.id, &version, kind, &file)
                .inspect_err(|e| log::warn!("{}: couldn't keep the package for rolling back: {e}", app.name))
                .is_ok();
        if !kept {
            let _ = std::fs::remove_file(&file);
        }
    }
    // The installer's own prompt was dismissed: the download itself wasn't cancelled.
    result.map_err(|e| if e == CANCELLED { INSTALL_CANCELLED.to_string() } else { e })
}

/// What a new install would write over: something already at its destination that the launcher
/// didn't put there.
pub fn occupied(
    app: &CraftApp,
    _asset: &Asset,
    kind: PackageKind,
    existing: Option<&Installed>,
    previous: Option<&Record>,
    locations: &InstallLocations,
    paths: &Paths,
) -> Result<Option<PathBuf>, String> {
    if existing.is_some() || previous.is_some() {
        return Ok(None);
    }
    let folder = destination(app, kind, None, None, locations)?;
    let target = match kind {
        PackageKind::Msi => folder,
        PackageKind::PortableZip => Some(folder.unwrap_or_else(|| paths.apps()).join(app.id)),
        _ => None,
    };
    Ok(target.filter(|t| std::fs::symlink_metadata(t).is_ok()))
}

/// The folder a new copy goes in: beside the launcher's copy it replaces, else the user's choice,
/// else the platform default (`None`).
fn destination(
    app: &CraftApp,
    kind: PackageKind,
    existing: Option<&Installed>,
    previous: Option<&Record>,
    locations: &InstallLocations,
) -> Result<Option<PathBuf>, String> {
    if kind == PackageKind::PortableZip && previous.is_some_and(|record| record.kind == PackageKind::PortableZip) {
        return Ok(None);
    }
    if let Some(inst) = existing {
        let exe = Path::new(&inst.location);
        return Ok(match kind {
            PackageKind::Msi => exe.parent().filter(|_| exe.is_absolute()).map(Path::to_path_buf),
            _ => None,
        });
    }
    let Some(folder) = locations.folder(app.id) else { return Ok(None) };
    if !matches!(kind, PackageKind::Msi | PackageKind::PortableZip) {
        return Err("custom install folders aren't supported on this system".into());
    }
    let parent = selected_folder(folder, kind != PackageKind::Msi)?;
    Ok(Some(if kind == PackageKind::Msi { parent.join(app.name) } else { parent }))
}

fn selected_folder(folder: &str, writable: bool) -> Result<PathBuf, String> {
    let path = if folder == "~" {
        dirs::home_dir().ok_or("no home folder")?
    } else if let Some(relative) = folder.strip_prefix("~/") {
        dirs::home_dir().ok_or("no home folder")?.join(relative)
    } else {
        PathBuf::from(folder)
    };
    let invalid = |reason: &str| format!("couldn't use install folder {}: {reason}", path.display());
    if folder.chars().any(|c| c.is_control() || (cfg!(windows) && c == '"')) {
        return Err(invalid("the path contains invalid characters"));
    }
    if !path.is_absolute() {
        return Err(invalid("choose an absolute path"));
    }
    if !path.is_dir() {
        return Err(invalid("the folder is unavailable. Create it or reconnect its drive first"));
    }
    if writable {
        let probe = path.join(format!(
            ".arthavan-projects-write-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())
        ));
        let output = std::fs::OpenOptions::new().write(true).create_new(true).open(&probe).map_err(|e| invalid(&e.to_string()))?;
        drop(output);
        std::fs::remove_file(&probe).map_err(|e| invalid(&e.to_string()))?;
    }
    Ok(path)
}

#[cfg(any(windows, test))]
fn msi_folder_argument(path: &Path) -> Result<String, String> {
    let value = path.to_str().ok_or("unusual characters in a path")?;
    if value.chars().any(|c| c == '"' || c.is_control()) {
        return Err(format!("couldn't use install folder {value}: the path contains invalid characters"));
    }
    Ok(format!("INSTALLFOLDER=\"{value}\""))
}

/// Where `asset` is downloaded to (and its partial file kept while unfinished).
pub fn download_path(paths: &Paths, asset: &Asset) -> PathBuf {
    paths.downloads().join(&asset.name)
}

#[allow(dead_code)]
fn record(version: &str, kind: PackageKind, path: PathBuf, files: Vec<PathBuf>, needs_root: bool) -> Record {
    Record { version: version.to_string(), kind, path, installed_at: crate::now(), files, needs_root }
}

/// The package format to install `app` with: the format of the copy that's there (an update
/// stays an MSI update), else the user's choice, else the best this system supports.
pub fn choose_kinds(existing: Option<&Installed>, record: Option<&Record>, preferred: &[PackageKind]) -> Vec<PackageKind> {
    choose_kinds_with(existing, record, preferred, &available_kinds())
}

pub(crate) fn choose_kinds_with(
    existing: Option<&Installed>,
    record: Option<&Record>,
    preferred: &[PackageKind],
    available: &[PackageKind],
) -> Vec<PackageKind> {
    if let Some(inst) = existing {
        let kind = match inst.source {
            InstallSource::Launcher => record.map(|r| r.kind),
            InstallSource::Msi => Some(PackageKind::Msi),
            _ => None,
        };
        if let Some(k) = kind.filter(|k| available.contains(k)) {
            return vec![k];
        }
    }
    let mut kinds: Vec<PackageKind> = preferred.iter().copied().filter(|k| available.contains(k)).collect();
    for k in available {
        if !kinds.contains(k) {
            kinds.push(*k);
        }
    }
    kinds
}

/// Package formats this machine can install, best first: the MSI and the portable ZIP on
/// Windows, nothing anywhere else (the launcher installs Windows packages only).
pub fn available_kinds() -> Vec<PackageKind> {
    if cfg!(windows) { vec![PackageKind::Msi, PackageKind::PortableZip] } else { Vec::new() }
}

/// Fetches `asset` into the download folder, checked against its published SHA-256: the feed's
/// or GitHub's per-file digest, else the release's `SHA256SUMS.txt`. A release with neither is
/// refused before anything is fetched: a file nothing vouches for is never installed (#90).
pub(crate) fn download(release: &Release, asset: &Asset, job: &mut Job<'_>) -> Result<PathBuf, String> {
    if !release::valid_file_name(&asset.name) {
        return Err("the release has an unexpected file name".into());
    }
    // A signed checksum list, when the release has one, is the one that counts.
    let signed = crate::signing::signed_sums(release, job.http)?;
    let sha = match (&signed, &asset.sha256) {
        (Some(list), published) => Some(crate::signing::checksum_from_signed(list, &asset.name, published.as_deref())?),
        (None, Some(s)) => Some(s.clone()),
        (None, None) => published_checksum(release, asset, job.http)?,
    };
    if signed.is_some() {
        log::info!("{}: checking against the signed checksums", asset.name);
    }
    let Some(sha) = sha else {
        return Err(format!(
            "the release publishes no checksum for {}, so it can't be verified and wasn't installed. Try again later, or report it on GitHub.",
            asset.name
        ));
    };
    let dest = download_path(job.paths, asset);
    let phase = &mut *job.phase;
    job.http.download(&asset.url, &dest, asset.size, &sha, job.stop, &mut |t| match t {
        Transfer::Bytes { done, total } => phase(JobPhase::Downloading { done, total }),
        Transfer::Reconnecting { done, total } => phase(JobPhase::Reconnecting { done, total }),
        Transfer::Waiting => phase(JobPhase::Queued),
    })?;
    (job.phase)(JobPhase::Verifying);
    Ok(dest)
}

/// The file's line in the release's `SHA256SUMS.txt`. `Ok(None)` only when the release really
/// publishes no checksum for it; a list that exists but can't be fetched is an error, so a
/// network hiccup never downgrades the check.
fn published_checksum(release: &Release, asset: &Asset, http: &Http) -> Result<Option<String>, String> {
    let Some(sums) = release.sums_asset() else { return Ok(None) };
    let Some(text) = http.get_text(&sums.url).map_err(|e| format!("couldn't fetch the release's checksums ({e}). Try again"))? else {
        return Ok(None);
    };
    Ok(release::parse_sha256sums(&text).into_iter().find(|(_, n)| *n == asset.name).map(|(h, _)| h))
}

/// When `installed` is owned by something the launcher doesn't drive, what to tell the user
/// instead of installing a second copy.
pub fn managed_elsewhere(app: &CraftApp, installed: &Installed, uninstall: bool) -> Option<String> {
    let _ = uninstall;
    match installed.source {
        InstallSource::Launcher | InstallSource::Msi => None,
        _ => Some(format!("{} was installed outside the launcher. Update or remove it the way it was installed.", app.name)),
    }
}

/// Removes an installed copy. `phase` hears when the removal has to wait for another install;
/// `stop` cancels it while it waits.
pub fn uninstall(
    app: &CraftApp,
    installed: &Installed,
    record: Option<&Record>,
    paths: &Paths,
    stop: &Stop,
    phase: &mut dyn FnMut(JobPhase),
) -> Result<(), String> {
    let _ = (record, stop, &phase, paths);
    let path = PathBuf::from(&installed.location);
    if let Some(msg) = managed_elsewhere(app, installed, true) {
        return Err(msg);
    }
    if matches!(installed.source, InstallSource::Msi | InstallSource::Launcher) && process::is_running(&path) {
        return Err(format!("{} is running. Quit it, then try again.", app.name));
    }
    match installed.source {
        #[cfg(windows)]
        InstallSource::Launcher if record.is_some_and(|r| r.kind == PackageKind::PortableZip) => {
            crate::portable::remove(app, record.ok_or("the launcher has no record of these files")?)
        }
        #[cfg(windows)]
        InstallSource::Msi => windows::uninstall_msi(app, stop, phase),
        other => Err(format!("{} can't be removed from here", other.label())),
    }
}

/// Starts an installed app, with the user's launch options (`options`).
/// `on_exit` hears when the program started ends.
pub fn launch(
    app: &CraftApp,
    installed: &Installed,
    paths: &Paths,
    options: &LaunchOptions,
    on_exit: Option<process::ExitHook>,
) -> Result<(), String> {
    let _ = paths;
    let path = Path::new(&installed.location);
    if !path.exists() {
        return Err(format!("Couldn't open {}: {} isn't where it was installed ({})", app.name, app.name, path.display()));
    }
    process::launch_binary(path, &options.args, &options.env, on_exit).map_err(|e| format!("Couldn't open {}: {e}", app.name))
}

/// The Windows Installer runs one job at a time; the others wait their turn here.
#[cfg(windows)]
static INSTALLER: crate::queue::Queue = crate::queue::Queue::new();

/// For [`INSTALLER`]: reports [`JobPhase::Waiting`] while a job waits, then `working` again.
#[cfg(windows)]
fn turn<'a>(phase: &'a mut dyn FnMut(JobPhase), working: JobPhase) -> impl FnMut(bool) + 'a {
    move |waiting| phase(if waiting { JobPhase::Waiting } else { working })
}

/// For [`INSTALLER`]: whether the user cancelled the job while it waits.
#[cfg(windows)]
fn cancelled(stop: &Stop) -> impl Fn() -> bool + '_ {
    || stop.check().is_err_and(|e| e == artcraft_launcher_core::protocol::CANCELLED)
}

/// Builds the Windows Installer command separately so its package argument can be checked
/// without starting an installer.
#[cfg_attr(not(windows), allow(dead_code))]
fn msi_command(program: &Path, msi: &Path) -> Result<std::process::Command, String> {
    let msi = msi_path(msi)?;
    let mut command = process::command(program);
    command.arg("/i").arg(msi).args(["/passive", "/norestart"]);
    Ok(command)
}

/// Windows Installer's storage APIs can fail on paths through junctions or symlinks (#338).
/// Keep ordinary drive/UNC syntax when possible: Windows canonicalization alone introduces
/// a `\\?\` prefix that older APIs also cannot handle.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn msi_path(msi: &Path) -> Result<PathBuf, String> {
    #[cfg(windows)]
    let resolved = dunce::canonicalize(msi);
    #[cfg(not(windows))]
    let resolved = std::fs::canonicalize(msi);
    resolved.map_err(|e| format!("couldn't resolve the Windows Installer package {} ({e})", msi.display()))
}

#[cfg(windows)]
pub mod windows {
    use artcraft_launcher_core::protocol::CANCELLED;

    use super::*;

    fn msiexec() -> PathBuf {
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from).join(r"System32\msiexec.exe")
    }

    fn explain(code: Option<i32>) -> Result<(), String> {
        match code {
            Some(0 | 3010 | 1641) => Ok(()),
            Some(1602) => Err("cancelled".into()),
            Some(1618) => Err("another installation is in progress. Wait for it to finish, then try again.".into()),
            Some(1925 | 1303) => Err("Windows didn't allow the install (administrator permission is needed)".into()),
            Some(c) => Err(format!("the Windows Installer stopped with error {c}")),
            None => Err("the Windows Installer was interrupted".into()),
        }
    }

    /// Runs an MSI with a progress window and no restart. Windows asks for permission itself.
    pub fn run_msi(msi: &Path, destination: Option<&Path>, stop: &Stop, phase: &mut dyn FnMut(JobPhase)) -> Result<(), String> {
        in_turn(stop, phase, JobPhase::Installing, || {
            use std::os::windows::process::CommandExt;
            // Nothing runs whose Authenticode signature fails.
            crate::authenticode::verify(&msi_path(msi)?)?;
            let mut command = msi_command(&msiexec(), msi)?;
            if let Some(dir) = destination {
                command.raw_arg(msi_folder_argument(dir)?);
            }
            let status = command.status().map_err(|e| format!("couldn't start the Windows Installer ({e})"))?;
            explain(status.code())
        })
    }

    /// Runs `work` once no other Windows Installer job of ours is running (a second one would
    /// stop with error 1618). Cancelled while it waits, it never runs.
    fn in_turn(stop: &Stop, phase: &mut dyn FnMut(JobPhase), working: JobPhase, work: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
        INSTALLER.alone(&cancelled(stop), &mut turn(phase, working), work).unwrap_or_else(|| Err(CANCELLED.into()))
    }

    pub fn uninstall_msi(app: &CraftApp, stop: &Stop, phase: &mut dyn FnMut(JobPhase)) -> Result<(), String> {
        let installs = crate::detect::windows::msi_installs(app);
        if installs.is_empty() {
            return Err(format!("{} isn't registered with the Windows Installer", app.name));
        }
        in_turn(stop, phase, JobPhase::Removing, || {
            for m in installs {
                let status = process::command(msiexec())
                    .args(["/x", &m.product_code, "/passive", "/norestart"])
                    .status()
                    .map_err(|e| format!("couldn't start the Windows Installer ({e})"))?;
                explain(status.code())?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn msi_install_uses_the_package_target_instead_of_a_symlinked_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("persistent cache");
        std::fs::create_dir(&cache).unwrap();
        let file = cache.join("PhotoCraft.msi");
        std::fs::write(&file, b"test package").unwrap();
        let link = dir.path().join("current cache");
        std::os::unix::fs::symlink(&cache, &link).unwrap();
        let command = msi_command(Path::new("msiexec.exe"), &link.join("PhotoCraft.msi")).unwrap();
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [
                std::ffi::OsStr::new("/i"),
                file.canonicalize().unwrap().as_os_str(),
                std::ffi::OsStr::new("/passive"),
                std::ffi::OsStr::new("/norestart")
            ]
        );
    }

    #[test]
    fn an_unavailable_msi_is_refused_before_starting_the_installer() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.msi");
        let error = msi_command(Path::new("msiexec.exe"), &missing).unwrap_err();
        assert!(error.contains("couldn't resolve the Windows Installer package"), "{error}");
    }

    #[cfg(windows)]
    #[test]
    fn a_short_local_msi_path_keeps_ordinary_windows_syntax() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("package.msi");
        std::fs::write(&file, b"test package").unwrap();
        let resolved = msi_path(&file).unwrap();
        assert!(!resolved.as_os_str().to_string_lossy().starts_with(r"\\?\"), "{resolved:?}");
        assert_eq!(std::fs::read(resolved).unwrap(), b"test package");
    }

    #[test]
    fn selected_folders_must_exist_and_leave_no_probe_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(selected_folder(dir.path().to_str().unwrap(), true).unwrap(), dir.path());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        #[cfg(unix)]
        {
            let quoted = dir.path().join("Craft \"Apps\"");
            std::fs::create_dir(&quoted).unwrap();
            assert_eq!(selected_folder(quoted.to_str().unwrap(), true).unwrap(), quoted);
        }
        let missing = dir.path().join("unavailable/Apps");
        assert!(selected_folder(missing.to_str().unwrap(), true).is_err());
        assert!(!missing.exists());
        assert!(selected_folder("relative/folder", true).is_err());
        assert!(selected_folder("bad\nfolder", true).is_err());
        let app = CraftApp::by_id("photocraft").unwrap();
        let locations = InstallLocations { root: Some(dir.path().display().to_string()), ..Default::default() };
        assert_eq!(destination(app, PackageKind::Msi, None, None, &locations).unwrap(), Some(dir.path().join("PhotoCraft")));
        assert_eq!(destination(app, PackageKind::PortableZip, None, None, &locations).unwrap(), Some(dir.path().to_path_buf()));
        let old = record("0.5.0", PackageKind::PortableZip, dir.path().join("PhotoCraft/photocraft.exe"), vec![], false);
        let unavailable = InstallLocations { root: Some(dir.path().join("missing").display().to_string()), ..Default::default() };
        assert_eq!(destination(app, PackageKind::PortableZip, None, Some(&old), &unavailable).unwrap(), None);
        let msi = Installed {
            version: None,
            source: InstallSource::Msi,
            location: dir.path().join("MSI/PhotoCraft/photocraft.exe").display().to_string(),
            package: None,
            upstream: false,
        };
        assert_eq!(destination(app, PackageKind::Msi, Some(&msi), Some(&old), &unavailable).unwrap(), Some(dir.path().join("MSI/PhotoCraft")));
        assert!(destination(app, PackageKind::TarGz, None, None, &locations).unwrap_err().contains("aren't supported"));
    }

    #[test]
    fn msi_folder_assignment_preserves_spaces_unicode_and_root_separators() {
        for (value, argument) in [
            (r"D:\", r#"INSTALLFOLDER="D:\""#),
            (r"D:\Craft Apps\PhotoCraft", r#"INSTALLFOLDER="D:\Craft Apps\PhotoCraft""#),
            (r"\\server\apps\绘画", r#"INSTALLFOLDER="\\server\apps\绘画""#),
            (r"D:\Craft Apps\", r#"INSTALLFOLDER="D:\Craft Apps\""#),
        ] {
            assert_eq!(msi_folder_argument(Path::new(value)).unwrap(), argument);
        }
        assert!(msi_folder_argument(Path::new("D:\\bad\" TRANSFORMS=evil")).is_err());
        assert!(msi_folder_argument(Path::new("D:\\bad\nfolder")).is_err());
    }

    #[test]
    fn a_new_install_reports_what_it_would_write_over() {
        let dir = tempfile::tempdir().unwrap();
        let app = CraftApp::by_id("photocraft").unwrap();
        let paths = Paths::under(dir.path());
        let locations = InstallLocations { root: Some(dir.path().display().to_string()), ..Default::default() };
        let asset = Asset { name: "photocraft.zip".into(), ..Default::default() };
        let occupied = |previous| occupied(app, &asset, PackageKind::PortableZip, None, previous, &locations, &paths).unwrap();
        assert_eq!(occupied(None), None);
        std::fs::create_dir(dir.path().join("photocraft")).unwrap();
        assert_eq!(occupied(None), Some(dir.path().join("photocraft")));
        let old = record("0.5.0", PackageKind::PortableZip, dir.path().join("photocraft/photocraft.exe"), vec![], false);
        assert_eq!(occupied(Some(&old)), None);
    }

    #[test]
    fn a_failed_install_keeps_the_download_for_the_next_try() {
        use crate::http::tests::{Reply, body, serve};
        let app = CraftApp::by_id("wordcraft").unwrap();
        let (body, sha) = body();
        let (url, requests) = serve(body.clone(), vec![Reply::Full, Reply::Full]);
        let asset = Asset { name: "wordcraft-0.3.0-test.pkg".into(), size: body.len() as u64, url, sha256: Some(sha) };
        let release = Release { assets: vec![asset.clone()], ..Release::default() };
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let http = Http::new().unwrap();
        // A format no build of the launcher installs here stands in for any failing install step.
        let kind = if cfg!(windows) { PackageKind::Dmg } else { PackageKind::Msi };
        let mut phases = Vec::new();
        for _ in 0..2 {
            let mut phase = |p: JobPhase| phases.push(p);
            let mut job = Job {
                http: &http,
                paths: &paths,
                stop: &Stop::default(),
                phase: &mut phase,
                install_locations: &InstallLocations::default(),
                replace: false,
                local: None,
                keep: false,
            };
            let err = install(app, &release, &asset, kind, None, None, &mut job).unwrap_err();
            assert!(err.contains("isn't supported"), "{err}");
            assert_eq!(std::fs::read(download_path(&paths, &asset)).unwrap(), body);
        }
        assert_eq!(requests.lock().unwrap().len(), 1, "the second try reuses the kept file");
        assert_eq!(phases.iter().filter(|p| **p == JobPhase::Installing).count(), 2);
    }

    #[test]
    fn an_unavailable_custom_folder_is_rejected_before_downloading() {
        use crate::http::tests::{Reply, body, serve};
        let app = CraftApp::by_id("wordcraft").unwrap();
        let (body, sha) = body();
        let (url, requests) = serve(body.clone(), vec![Reply::Full]);
        let asset = Asset { name: "wordcraft-0.3.0-windows-x64-portable.zip".into(), size: body.len() as u64, url, sha256: Some(sha) };
        let release = Release { assets: vec![asset.clone()], ..Release::default() };
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("unavailable/Apps");
        let locations = InstallLocations { root: Some(missing.display().to_string()), ..Default::default() };
        let paths = Paths::under(dir.path());
        let http = Http::new().unwrap();
        let mut phases = vec![];
        let mut phase = |p| phases.push(p);
        let mut job = Job {
            http: &http,
            paths: &paths,
            stop: &Stop::default(),
            phase: &mut phase,
            install_locations: &locations,
            replace: false,
            local: None,
            keep: false,
        };
        let error = install(app, &release, &asset, PackageKind::PortableZip, None, None, &mut job).unwrap_err();
        assert!(error.contains("folder is unavailable"), "{error}");
        assert!(requests.lock().unwrap().is_empty());
        assert!(phases.is_empty());
        assert!(!missing.exists());
    }

    #[test]
    fn a_release_without_a_checksum_is_refused_before_anything_is_fetched() {
        use crate::http::tests::{Reply, body, serve};
        let app = CraftApp::by_id("wordcraft").unwrap();
        let (body, _) = body();
        let (url, requests) = serve(body.clone(), vec![Reply::Full]);
        // Neither a per-file digest nor a SHA256SUMS.txt in the release.
        let asset = Asset { name: "wordcraft-0.3.1-test.pkg".into(), size: body.len() as u64, url, sha256: None };
        let release = Release { assets: vec![asset.clone()], ..Release::default() };
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let http = Http::new().unwrap();
        let kind = if cfg!(windows) { PackageKind::Dmg } else { PackageKind::Msi };
        let mut phases = Vec::new();
        let mut phase = |p: JobPhase| phases.push(p);
        let mut job = Job {
            http: &http,
            paths: &paths,
            stop: &Stop::default(),
            phase: &mut phase,
            install_locations: &InstallLocations::default(),
            replace: false,
            local: None,
            keep: false,
        };
        let err = install(app, &release, &asset, kind, None, None, &mut job).unwrap_err();
        assert!(err.contains("publishes no checksum"), "{err}");
        assert!(!download_path(&paths, &asset).exists());
        assert!(requests.lock().unwrap().is_empty(), "nothing was fetched");
        assert!(!phases.contains(&JobPhase::Installing));
    }

    #[test]
    fn updates_keep_the_installed_format() {
        let inst = |source| Installed { version: None, source, location: String::new(), package: None, upstream: false };
        let app = artcraft_launcher_core::CraftApp::by_id("photocraft").unwrap();
        let available = available_kinds();
        assert_eq!(available.is_empty(), !cfg!(windows), "{available:?}");
        if let Some(first) = available.first() {
            assert_eq!(choose_kinds(None, None, &[]).first(), Some(first));
        }
        #[cfg(windows)]
        assert_eq!(choose_kinds(Some(&inst(InstallSource::Msi)), None, &[]), [PackageKind::Msi]);
        let windows = [PackageKind::Msi, PackageKind::PortableZip];
        assert_eq!(choose_kinds_with(Some(&inst(InstallSource::Msi)), None, &[], &windows), [PackageKind::Msi]);
        assert_eq!(choose_kinds_with(None, None, &[PackageKind::PortableZip], &windows), [PackageKind::PortableZip, PackageKind::Msi]);
        // A copy from somewhere else is never updated in another format.
        assert_eq!(choose_kinds_with(Some(&inst(InstallSource::Deb)), None, &[], &windows), windows);
        assert!(managed_elsewhere(app, &inst(InstallSource::Deb), false).is_some());
        assert_eq!(managed_elsewhere(app, &inst(InstallSource::Msi), false), None);
    }

    #[test]
    fn install_refuses_to_downgrade_when_target_release_is_older() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        let http = Http::new().unwrap();
        let phase = &mut |_| ();
        let locations = InstallLocations::default();
        let app = CraftApp::by_id("photocraft").unwrap();
        let release = Release { version: version::parse("0.4.0"), tag: "v0.4.0".into(), ..Release::default() };
        let asset = Asset { name: "photocraft-0.4.0-windows-x64-portable.zip".into(), ..Asset::default() };
        let installed = Installed {
            version: version::parse("0.5.0"),
            source: InstallSource::Launcher,
            location: dir.path().join("PhotoCraft/photocraft.exe").display().to_string(),
            package: None,
            upstream: false,
        };
        let stop = Stop::default();
        let mut job = Job { http: &http, paths: &paths, stop: &stop, phase, install_locations: &locations, replace: false, local: None, keep: false };
        let result = install_at(app, &release, &asset, PackageKind::PortableZip, Some(&installed), None, &mut job, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("installed version 0.5.0 is newer"));
    }
}
