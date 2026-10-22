//! Platforms, package formats, and choosing the right release asset.
//!
//! Every Craft App release names its files `<app>-<version>-<os>-<arch>[-portable].<ext>`, for
//! example `photocraft-0.5.0-windows-x64.msi` or `photocraft-0.5.0-windows-x64-portable.zip`.
//!
//! The launcher supports Windows only. The grammar still reads every package kind older releases
//! shipped (so their names parse the same way), but only Windows packages are ever chosen: other
//! systems are offered nothing.
//!
//! Labels are English. The UI translates [`PackageKind::label`]; OS and architecture names are
//! shown as they are.

use serde::{Deserialize, Serialize};

use crate::release::{Asset, Release};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
    FreeBsd,
}

impl Os {
    pub fn label(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::MacOs => "macOS",
            Self::Linux => "Linux",
            Self::FreeBsd => "FreeBSD",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Arch {
    X64,
    X86,
    Arm64,
    Riscv64,
}

impl Arch {
    pub fn label(self) -> &'static str {
        match self {
            Self::X64 => "x64",
            Self::X86 => "x86",
            Self::Arm64 => "ARM64",
            Self::Riscv64 => "RISC-V",
        }
    }
}

/// A package format a release ships.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PackageKind {
    /// Windows Installer package (installs for all users into Program Files).
    Msi,
    /// Windows zip with the exe at the top level.
    PortableZip,
    /// macOS disk image with the `.app` bundle.
    Dmg,
    AppImage,
    Deb,
    Rpm,
    /// Arch Linux package (`.pkg.tar.zst`).
    ArchPkg,
    Flatpak,
    /// Linux or FreeBSD tarball (`bin/<app>`, `share/…`). Like the other non-Windows kinds,
    /// only read in names of older releases: never installed or offered.
    TarGz,
}

impl PackageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Msi => "Installer (.msi)",
            Self::PortableZip => "Portable (.zip)",
            Self::Dmg => "Disk image (.dmg)",
            Self::AppImage => "AppImage",
            Self::Deb => "Debian package (.deb)",
            Self::Rpm => "RPM package (.rpm)",
            Self::ArchPkg => "Arch package (.pkg.tar.zst)",
            Self::Flatpak => "Flatpak bundle",
            Self::TarGz => "Tarball (.tar.gz)",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Msi => ".msi",
            Self::PortableZip => "-portable.zip",
            Self::Dmg => ".dmg",
            Self::AppImage => ".AppImage",
            Self::Deb => ".deb",
            Self::Rpm => ".rpm",
            Self::ArchPkg => ".pkg.tar.zst",
            Self::Flatpak => ".flatpak",
            Self::TarGz => ".tar.gz",
        }
    }
}

/// Where the launcher is running (or, on the web, the visitor's machine).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    /// The platform this binary was built for, or `None` for wasm and unsupported targets.
    pub fn current() -> Option<Self> {
        let os = if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else if cfg!(target_os = "freebsd") {
            Os::FreeBsd
        } else {
            return None;
        };
        let arch = if cfg!(target_arch = "x86_64") {
            Arch::X64
        } else if cfg!(target_arch = "x86") {
            Arch::X86
        } else if cfg!(target_arch = "aarch64") {
            Arch::Arm64
        } else if cfg!(target_arch = "riscv64") {
            Arch::Riscv64
        } else {
            return None;
        };
        Some(Self { os, arch })
    }

    pub fn label(self) -> String {
        match self.os {
            Os::MacOs => "macOS".into(),
            os => format!("{} {}", os.label(), self.arch.label()),
        }
    }

    /// Whether the launcher supports this OS: Windows only.
    pub fn is_supported(self) -> bool {
        self.os == Os::Windows
    }

    /// Asset platform tokens to try, best first. Windows on ARM and 64-bit Windows can fall back
    /// to emulated builds. Other systems are not supported and get none.
    fn tokens(self) -> &'static [&'static str] {
        match (self.os, self.arch) {
            (Os::Windows, Arch::X64) => &["windows-x64", "windows-x86"],
            (Os::Windows, Arch::X86) => &["windows-x86"],
            (Os::Windows, Arch::Arm64) => &["windows-arm64", "windows-x64", "windows-x86"],
            _ => &[],
        }
    }

    /// Package formats the launcher installs on this OS, in order of preference (none outside
    /// Windows).
    pub fn default_kinds(self) -> &'static [PackageKind] {
        match self.os {
            Os::Windows => &[PackageKind::Msi, PackageKind::PortableZip],
            _ => &[],
        }
    }

    /// Every format offered as a manual download on this OS (the web build's download menu; none
    /// outside Windows).
    pub fn download_kinds(self) -> &'static [PackageKind] {
        self.default_kinds()
    }
}

/// What an asset name says about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetName {
    pub stem: String,
    pub version: String,
    pub token: String,
    pub kind: PackageKind,
}

/// Reads `<stem>-<version>-<os>-<arch>[-portable].<ext>`; `None` for checksums, zsync files, CLI
/// and web bundles, or anything else.
pub fn parse_asset_name(name: &str, stems: &[&str]) -> Option<AssetName> {
    const KINDS: [PackageKind; 9] = [
        PackageKind::PortableZip,
        PackageKind::TarGz,
        PackageKind::Msi,
        PackageKind::Dmg,
        PackageKind::AppImage,
        PackageKind::Deb,
        PackageKind::Rpm,
        PackageKind::ArchPkg,
        PackageKind::Flatpak,
    ];
    let (kind, base) = KINDS.iter().find_map(|k| name.strip_suffix(k.suffix()).map(|b| (*k, b)))?;
    for stem in stems {
        let Some(rest) = base.strip_prefix(stem).and_then(|r| r.strip_prefix('-')) else { continue };
        // rest = <version>-<os>-<arch>; os and arch never contain '-', versions may.
        let mut parts = rest.rsplitn(3, '-');
        let (arch, os, version) = (parts.next()?, parts.next()?, parts.next()?);
        if crate::version::parse(version).is_none() || !version.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        return Some(AssetName { stem: (*stem).to_string(), version: version.to_string(), token: format!("{os}-{arch}"), kind });
    }
    None
}

/// The best asset of `release` for `platform`, trying `kinds` in order and, within a kind, the
/// platform's tokens in order (native before emulated).
pub fn pick_asset<'a>(release: &'a Release, stems: &[&str], platform: Platform, kinds: &[PackageKind]) -> Option<(&'a Asset, PackageKind)> {
    let parsed: Vec<(&Asset, AssetName)> = release.assets.iter().filter_map(|a| parse_asset_name(&a.name, stems).map(|n| (a, n))).collect();
    for kind in kinds {
        for token in platform.tokens() {
            if let Some((asset, _)) = parsed.iter().find(|(_, n)| n.kind == *kind && n.token == *token) {
                return Some((asset, *kind));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(names: &[&str]) -> Release {
        Release {
            assets: names.iter().map(|n| Asset { name: (*n).into(), size: 1, url: format!("https://x/{n}"), sha256: None }).collect(),
            ..Release::default()
        }
    }

    #[test]
    fn release_extras_are_never_packages() {
        for extra in [
            "update-feed.json",
            "photocraft-0.5.0-sbom.cdx.json",
            "photocraft-0.5.0-linux-x86_64.AppImage.zsync",
            "SHA256SUMS.txt",
            "SHA256SUMS.txt.minisig",
        ] {
            assert!(parse_asset_name(extra, &["photocraft"]).is_none(), "{extra}");
        }
        let r = release(&["update-feed.json", "photocraft-0.5.0-sbom.cdx.json", "photocraft-0.5.0-linux-x86_64.AppImage.zsync"]);
        for p in [
            Platform { os: Os::Linux, arch: Arch::X64 },
            Platform { os: Os::MacOs, arch: Arch::Arm64 },
            Platform { os: Os::Windows, arch: Arch::X64 },
        ] {
            assert!(pick_asset(&r, &["photocraft"], p, p.download_kinds()).is_none(), "{p:?}");
        }
    }

    const PHOTO: &[&str] = &[
        "photocraft-0.5.0-freebsd-x86_64.tar.gz",
        "photocraft-0.5.0-linux-aarch64.AppImage",
        "photocraft-0.5.0-linux-aarch64.AppImage.zsync",
        "photocraft-0.5.0-linux-aarch64.deb",
        "photocraft-0.5.0-linux-riscv64.tar.gz",
        "photocraft-0.5.0-linux-x86_64.AppImage",
        "photocraft-0.5.0-linux-x86_64.tar.gz",
        "photocraft-0.5.0-macos-universal.dmg",
        "photocraft-0.5.0-windows-x64-portable.zip",
        "photocraft-0.5.0-windows-x64.msi",
        "photocraft-0.5.0-windows-x86.msi",
        "photocraft-cli-0.5.0-macos-universal.zip",
        "photocraft-web-0.5.0.zip",
        "SHA256SUMS.txt",
    ];

    #[test]
    fn only_windows_is_supported_and_other_systems_are_offered_nothing() {
        let r = release(PHOTO);
        for os in [Os::MacOs, Os::Linux, Os::FreeBsd] {
            for arch in [Arch::X64, Arch::X86, Arch::Arm64, Arch::Riscv64] {
                let p = Platform { os, arch };
                assert!(!p.is_supported());
                assert!(p.default_kinds().is_empty() && p.download_kinds().is_empty(), "{p:?}");
                let every = [
                    PackageKind::Msi,
                    PackageKind::PortableZip,
                    PackageKind::Dmg,
                    PackageKind::AppImage,
                    PackageKind::Deb,
                    PackageKind::Rpm,
                    PackageKind::ArchPkg,
                    PackageKind::Flatpak,
                    PackageKind::TarGz,
                ];
                assert_eq!(pick_asset(&r, &["photocraft"], p, &every), None, "{p:?}");
            }
        }
        assert!(Platform { os: Os::Windows, arch: Arch::X64 }.is_supported());
    }

    #[test]
    fn reads_asset_names() {
        let n = parse_asset_name("photocraft-0.2.0-rc.1-windows-arm64-portable.zip", &["photocraft"]).unwrap();
        assert_eq!((n.version.as_str(), n.token.as_str(), n.kind), ("0.2.0-rc.1", "windows-arm64", PackageKind::PortableZip));
        let n = parse_asset_name("printcraft-0.2.1-linux-x86_64.tar.gz", &["pdfcraft", "printcraft"]).unwrap();
        assert_eq!((n.stem.as_str(), n.kind), ("printcraft", PackageKind::TarGz));
        for other in
            ["photocraft-cli-0.5.0-macos-universal.zip", "photocraft-web-0.5.0.zip", "SHA256SUMS.txt", "photocraft-0.5.0-linux-x86_64.AppImage.zsync"]
        {
            assert_eq!(parse_asset_name(other, &["photocraft"]), None, "{other}");
        }
        // FigCraft also ships its collaboration server; that tarball is not the app.
        assert_eq!(parse_asset_name("figcraft-server-0.5.1-linux-x86_64.tar.gz", &["figcraft"]), None);
        assert!(parse_asset_name("figcraft-0.5.1-linux-x86_64.tar.gz", &["figcraft"]).is_some());
    }

    #[test]
    fn picks_the_native_build_first_and_falls_back_to_emulation() {
        let r = release(PHOTO);
        let pick = |os, arch, kinds: &[PackageKind]| pick_asset(&r, &["photocraft"], Platform { os, arch }, kinds).map(|(a, _)| a.name.clone());
        assert_eq!(pick(Os::Windows, Arch::X64, &[PackageKind::Msi]).as_deref(), Some("photocraft-0.5.0-windows-x64.msi"));
        // No arm64 MSI in this release: Windows on ARM runs the x64 build.
        assert_eq!(pick(Os::Windows, Arch::Arm64, &[PackageKind::Msi]).as_deref(), Some("photocraft-0.5.0-windows-x64.msi"));
        // Preference order across kinds.
        assert_eq!(
            pick(Os::Windows, Arch::X64, &[PackageKind::PortableZip, PackageKind::Msi]).as_deref(),
            Some("photocraft-0.5.0-windows-x64-portable.zip")
        );
        assert_eq!(pick(Os::Windows, Arch::X86, &[PackageKind::PortableZip, PackageKind::Msi]).as_deref(), Some("photocraft-0.5.0-windows-x86.msi"));
    }
}
