//! "Start Arthavan Projects when I log in": a value under `HKCU\…\CurrentVersion\Run`.
//!
//! The launcher starts hidden in the tray when it's started this way (`--hidden`). It supports
//! Windows only: elsewhere the setting is off and turning it on reports that.

/// The command-line flag a login start passes.
pub const HIDDEN_FLAG: &str = "--hidden";

/// The name of the startup entry.
#[cfg(windows)]
const RUN_VALUE: &str = "Arthavan Projects";

pub fn is_enabled() -> bool {
    #[cfg(windows)]
    return windows_run().and_then(|k| k.get_value::<String, _>(RUN_VALUE).ok()).is_some();
    #[cfg(not(windows))]
    return false;
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
            .map_err(|e| format!("couldn't change the startup list ({e})"))?;
        if on {
            let exe = std::env::current_exe().map_err(|e| format!("couldn't find this program ({e})"))?;
            let value = format!("\"{}\" {HIDDEN_FLAG}", exe.display());
            key.set_value(RUN_VALUE, &value).map_err(|e| format!("couldn't change the startup list ({e})"))
        } else {
            match key.delete_value(RUN_VALUE) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("couldn't change the startup list ({e})")),
            }
        }
    }
    #[cfg(not(windows))]
    {
        if on { Err("Starting at login is supported on Windows only".into()) } else { Ok(()) }
    }
}

#[cfg(windows)]
fn windows_run() -> Option<winreg::RegKey> {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run").ok()
}
