//! The Craft App Suite: the thirteen apps the launcher knows about.
//!
//! The list is data so a later version can fetch it from an API; [`SUITE`] is the built-in copy.
//!
//! Text here is English, the source the UI translates from: each `tagline` has a catalog row
//! keyed by the app id (`tr_ctx(app.id, app.tagline)` in `ui-egui`).

/// One app in the suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CraftApp {
    /// Machine name: repo, binary, package and asset stem (`photocraft`).
    pub id: &'static str,
    /// Display name (`PhotoCraft`).
    pub name: &'static str,
    /// What it's for, in a few words (English; the UI translates it by app id).
    pub tagline: &'static str,
    /// Brand colour (sRGB), for surfaces.
    pub color: [u8; 3],
    /// The brand colour for text on a light background (the site's light `--app-ink`).
    pub ink_light: [u8; 3],
    /// The brand colour for text on a dark background (the site's dark `--app-ink`).
    pub ink_dark: [u8; 3],
    /// GitHub `owner/repo`.
    pub repo: &'static str,
    /// Older names its release assets may carry (PdfCraft shipped as PrintCraft until 0.2.1).
    pub former_ids: &'static [&'static str],
    /// Older display names its installers registered (PdfCraft's MSI said PrintCraft until 0.2.1).
    pub former_names: &'static [&'static str],
}

impl CraftApp {
    /// The macOS bundle id, Flatpak id and Linux desktop-file id: `ai.storyteller.<id>` for a
    /// Craft App, [`LAUNCHER_BUNDLE_ID`] for the launcher itself.
    pub fn bundle_id(&self) -> String {
        if self.id == LAUNCHER_ID { LAUNCHER_BUNDLE_ID.to_string() } else { format!("ai.storyteller.{}", self.id) }
    }

    pub fn releases_url(&self) -> String {
        format!("https://github.com/{}/releases", self.repo)
    }

    pub fn repo_url(&self) -> String {
        format!("https://github.com/{}", self.repo)
    }

    /// Asset stems this app's releases may use, current first.
    pub fn stems(&self) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(self.id).chain(self.former_ids.iter().copied())
    }

    /// Display names this app's installers may have registered, current first.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(self.name).chain(self.former_names.iter().copied())
    }

    pub fn by_id(id: &str) -> Option<&'static CraftApp> {
        SUITE.iter().find(|a| a.id == id)
    }
}

/// The GitHub account that owns every repository below: the single place to change it. A macro
/// rather than a `const` so the repo strings can be built from it with `concat!` at compile time.
macro_rules! github_owner {
    () => {
        "685ks"
    };
}

/// `"<owner>/<name>"` under [`GITHUB_OWNER`].
macro_rules! repo {
    ($name:literal) => {
        concat!(github_owner!(), "/", $name)
    };
}

const fn app(
    id: &'static str,
    name: &'static str,
    tagline: &'static str,
    [color, ink_light, ink_dark]: [[u8; 3]; 3],
    repo: &'static str,
) -> CraftApp {
    CraftApp { id, name, tagline, color, ink_light, ink_dark, repo, former_ids: &[], former_names: &[] }
}

/// The Craft App Suite, in display order:
pub const SUITE: [CraftApp; 13] = [
    app("photocraft", "PhotoCraft", "Image editing", [[0x2f, 0x7b, 0xf5], [0x1a, 0x5b, 0xc4], [0x7f, 0xb0, 0xff]], repo!("photocraft")),
    app("vectorcraft", "VectorCraft", "Vector illustration", [[0xe8, 0x57, 0x3f], [0xb8, 0x3a, 0x24], [0xff, 0x8a, 0x70]], repo!("vectorcraft")),
    app("filmcraft", "FilmCraft", "Video editing, color and sound", [[0x8b, 0x5c, 0xf6], [0x6a, 0x3f, 0xd6], [0xb6, 0x9c, 0xff]], repo!("filmcraft")),
    app(
        "lightcraft",
        "LightCraft",
        "Photo library and raw development",
        [[0xf2, 0xa5, 0x16], [0x8a, 0x58, 0x00], [0xff, 0xc1, 0x4d]],
        repo!("lightcraft"),
    ),
    CraftApp {
        former_ids: &["printcraft"],
        former_names: &["PrintCraft"],
        ..app(
            "pdfcraft",
            "PdfCraft",
            "Reading, organizing and protecting PDFs",
            [[0x12, 0xa5, 0x8a], [0x0a, 0x75, 0x63], [0x3f, 0xd6, 0xb8]],
            repo!("pdfcraft"),
        )
    },
    app(
        "effectcraft",
        "EffectCraft",
        "Motion graphics and visual effects",
        [[0xe0, 0x36, 0x8f], [0xb0, 0x20, 0x6c], [0xff, 0x7a, 0xb8]],
        repo!("effectcraft"),
    ),
    app(
        "designcraft",
        "DesignCraft",
        "Page layout and publishing",
        [[0x7b, 0xb5, 0x1c], [0x45, 0x70, 0x08], [0xa6, 0xe0, 0x4a]],
        repo!("designcraft"),
    ),
    app("figcraft", "FigCraft", "Interface design and prototyping", [[0x02, 0xda, 0x96], [0x04, 0x7a, 0x56], [0x4f, 0xe8, 0xb6]], repo!("figcraft")),
    app("wordcraft", "WordCraft", "Writing and document design", [[0x3f, 0x5f, 0xe0], [0x2a, 0x44, 0xb8], [0x8e, 0xa2, 0xff]], repo!("wordcraft")),
    app(
        "soundcraft",
        "SoundCraft",
        "Recording, editing and mixing audio",
        [[0x1f, 0xa5, 0xc9], [0x0e, 0x72, 0x90], [0x5f, 0xd0, 0xee]],
        repo!("soundcraft"),
    ),
    app("deckcraft", "DeckCraft", "Presentations and slide shows", [[0xf0, 0x7a, 0x1e], [0xa8, 0x4f, 0x0a], [0xff, 0xa0, 0x60]], repo!("deckcraft")),
    app(
        "cadcraft",
        "CADCraft",
        "Computer-aided design and drafting",
        [[0x3e, 0xa6, 0xe8], [0x1a, 0x6f, 0xa8], [0x7c, 0xc8, 0xff]],
        repo!("cadcraft"),
    ),
    app("gridcraft", "GridCraft", "Spreadsheets and data", [[0x1f, 0x9d, 0x55], [0x12, 0x70, 0x3a], [0x4f, 0xd6, 0x8a]], repo!("gridcraft")),
];

/// The GitHub account that owns every repository below: the only one the launcher
/// installs from, whatever the release feed says. Spelled out once, in the `github_owner!` macro above.
pub const GITHUB_OWNER: &str = github_owner!();
/// The owner's GitHub account: every Craft App's source.
pub const GITHUB_URL: &str = concat!("https://github.com/", github_owner!());
/// Arthavan Projects' home page: its own GitHub repository.
pub const WEBSITE_URL: &str = concat!("https://github.com/", repo!("craft-launcher"));
/// Where to ask questions and report problems: the repository's issues.
pub const SUPPORT_URL: &str = concat!("https://github.com/", repo!("craft-launcher"), "/issues");

/// Arthavan Projects itself, described like a Craft App so it can update itself with the same
/// machinery (asset names `arthavan-projects-<ver>-…`, bundle id `app.freedev.ainz.ks.ArthavanProjects`,
/// `Arthavan Projects.app`). Not part of [`SUITE`].
pub const LAUNCHER: CraftApp = app(
    "arthavan-projects",
    "Arthavan Projects",
    "Installs and updates the Craft App Suite",
    [[0x0b, 0x0b, 0x0c], [0x0b, 0x0b, 0x0c], [0xf2, 0xf1, 0xee]],
    repo!("craft-launcher"),
);

/// Who may sign the Craft Apps' and the launcher's Windows installers: the common names (`CN=`)
/// of the Authenticode certificates accepted before Windows Installer runs an MSI. Empty while
/// the releases aren't signed: an unsigned MSI is then installed (its SHA-256 is checked against
/// the release first), but a broken, tampered or untrusted signature still stops the install.
/// Once the releases are signed, name the publisher here and unsigned installers are refused.
pub const WINDOWS_SIGNERS: &[&str] = &[];

/// The minisign public key (the `RW…` line of `minisign.pub`) that signs release checksum lists
/// (`SHA256SUMS.txt.minisig`) and the remote catalog. `None` until the owner makes a key pair
/// (`docs/release-signing.md`): every release then counts as unsigned and the built-in catalog is
/// used as it is.
pub const RELEASE_SIGNING_KEY: Option<&str> = None;

/// The launcher's own repo, checked so it can say when a newer launcher is out.
pub const LAUNCHER_REPO: &str = repo!("craft-launcher");
/// The launcher's own asset stem (and binary, package and data-folder name).
pub const LAUNCHER_ID: &str = "arthavan-projects";
/// The launcher's reverse-DNS id: macOS bundle id, Flatpak id, desktop-file and AppStream id,
/// Windows AppUserModelID.
pub const LAUNCHER_BUNDLE_ID: &str = "app.freedev.ainz.ks.ArthavanProjects";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_has_thirteen_apps_with_unique_ids() {
        assert_eq!(SUITE.len(), 13);
        let mut ids: Vec<_> = SUITE.iter().map(|a| a.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SUITE.len());
        for a in &SUITE {
            assert_eq!(a.repo, format!("{GITHUB_OWNER}/{}", a.id));
            assert_eq!(a.name.to_lowercase(), a.id);
        }
    }

    #[test]
    fn every_repo_lives_under_the_owner() {
        assert_eq!(GITHUB_URL, format!("https://github.com/{GITHUB_OWNER}"));
        assert_eq!(LAUNCHER_REPO, format!("{GITHUB_OWNER}/craft-launcher"));
        assert_eq!(LAUNCHER.repo, LAUNCHER_REPO);
        assert_eq!(WEBSITE_URL, format!("https://github.com/{LAUNCHER_REPO}"));
        assert_eq!(SUPPORT_URL, format!("https://github.com/{LAUNCHER_REPO}/issues"));
    }

    #[test]
    fn launcher_ids_match() {
        assert_eq!(LAUNCHER.id, LAUNCHER_ID);
        assert_eq!(LAUNCHER.bundle_id(), LAUNCHER_BUNDLE_ID);
        assert_eq!(CraftApp::by_id("photocraft").unwrap().bundle_id(), "ai.storyteller.photocraft");
    }

    #[test]
    fn pdfcraft_also_answers_to_printcraft() {
        let pdf = CraftApp::by_id("pdfcraft").unwrap();
        assert_eq!(pdf.stems().collect::<Vec<_>>(), ["pdfcraft", "printcraft"]);
        assert_eq!(pdf.names().collect::<Vec<_>>(), ["PdfCraft", "PrintCraft"]);
    }
}
