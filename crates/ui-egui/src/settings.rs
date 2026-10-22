//! The user's choices, saved between runs by the shell.

use std::collections::BTreeMap;

use artcraft_launcher_core::PackageKind;
use serde::{Deserialize, Serialize};

use crate::i18n::t;
use crate::theme::ThemeChoice;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    #[default]
    Grid,
    Table,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Filter {
    #[default]
    All,
    Installed,
    Updates,
    NotInstalled,
    /// Apps the user chose to hide from the other lists.
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortKey {
    /// Suite order: classic apps, then new. Settings saved with the removed "Inspired by" sort load as this.
    #[default]
    #[serde(alias = "InspiredBy")]
    Suite,
    Name,
    Status,
    /// Installed version.
    Installed,
    /// Latest released version.
    Latest,
    /// Release date.
    Released,
    /// How often the app has been opened from here, most first.
    Frequent,
}

impl SortKey {
    /// The direction a first click sorts in: newest and highest first for dates and versions,
    /// A to Z for text.
    pub fn starts_descending(self) -> bool {
        matches!(self, Self::Installed | Self::Latest | Self::Released | Self::Frequent)
    }
}

/// How often to look for updates in the background (besides on start and when the network
/// comes back; see `core::updates::Scheduler`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckEvery {
    Hourly,
    /// The default. Settings saved when the only choice was "Automatically" (`Hour`, and the older
    /// `FourHours`) load as this.
    #[serde(alias = "Hour", alias = "FourHours")]
    SixHours,
    #[serde(alias = "Day")]
    Daily,
    Never,
}

impl CheckEvery {
    pub const ALL: [Self; 4] = [Self::Hourly, Self::SixHours, Self::Daily, Self::Never];

    pub fn label(self) -> String {
        match self {
            Self::Hourly => t("Every hour").to_string(),
            Self::SixHours => t("Every 6 hours").to_string(),
            Self::Daily => t("Every day").to_string(),
            Self::Never => t("Only when I ask").to_string(),
        }
    }

    /// How often to look. Every check asks GitHub about each app (revalidated with ETags, which
    /// cost no quota when nothing changed), so the default is a few times a day.
    pub fn seconds(self) -> Option<i64> {
        match self {
            Self::Hourly => Some(3600),
            Self::SixHours => Some(artcraft_launcher_core::updates::DEFAULT_CHECK_EVERY),
            Self::Daily => Some(24 * 3600),
            Self::Never => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub install_locations: artcraft_launcher_core::protocol::InstallLocations,
    pub view: ViewMode,
    pub filter: Filter,
    pub sort: SortKey,
    pub sort_descending: bool,
    pub theme: ThemeChoice,
    /// Interface language: `auto` (follow the system) or a language code from the registry
    /// (`en`, `ru`, `zh-CN`, `de`, `fr`, `pt-BR`, `pl`).
    pub language: String,
    /// Closing the window keeps the launcher in the tray (menu bar) instead of quitting.
    pub close_to_tray: bool,
    /// macOS: showing the window brings it to the current desktop (Space) instead of switching to
    /// the desktop it was on.
    pub show_on_current_desktop: bool,
    pub check_every: CheckEvery,
    /// Download updates in the background and put them in place when the app isn't running
    /// (on by default; each app can opt out with `manual_updates`).
    pub auto_update: bool,
    /// Apps whose "Update automatically" is switched off (catalog ids).
    pub manual_updates: std::collections::BTreeSet<String>,
    /// Versions rolled back because they kept failing to start: app id → version. Not installed
    /// automatically again (a newer one is).
    pub rolled_back: BTreeMap<String, String>,
    /// Show a system notification when an installed app has an update.
    pub notify: bool,
    pub prereleases: bool,
    /// The package format to prefer for new installs on Windows: the MSI or the portable ZIP
    /// (empty: the MSI).
    pub preferred_kinds: Vec<PackageKind>,
    /// Announcements and alerts from the release feed the user dismissed (by notice id).
    pub dismissed_notices: std::collections::BTreeSet<String>,
    /// Updates already announced, so each version is announced once: app id → version.
    pub notified: BTreeMap<String, String>,
    /// Unix seconds of the last finished check.
    pub last_check: Option<i64>,
    /// Catalog ids of apps hidden from the main lists (`photocraft`, …).
    pub hidden: std::collections::BTreeSet<String>,
    /// How many times each app was opened from the launcher (catalog id → count), for "Frequently used".
    pub launches: BTreeMap<String, u32>,
    /// Release channel per app id; an app without one follows `prereleases`.
    pub channels: BTreeMap<String, artcraft_launcher_core::channel::Channel>,
    /// Launch arguments and environment variables per app id (the app's "Launch options").
    pub launch: BTreeMap<String, artcraft_launcher_core::launch::LaunchProfile>,
    /// Write a crash report on this computer when the launcher stops unexpectedly (opt-in).
    pub crash_reports: bool,
}

impl Settings {
    /// Whether `app` updates by itself: automatic updates are on and the app hasn't opted out.
    pub fn updates_automatically(&self, app: &str) -> bool {
        self.auto_update && !self.manual_updates.contains(app)
    }

    /// Machine-readable code for "follow the system language".
    pub const LANGUAGE_AUTO: &'static str = "auto";

    /// Display name for a language `code`: a language's name in itself, from the registry
    /// ([`LANGUAGES`](crate::i18n::LANGUAGES)). A language needing a CJK font is named in English,
    /// with its own name added only while `installed` — the CJK face in the current font stack — is
    /// that language (no other face draws it); "Auto" for `auto` and unknown codes, like
    /// [`Lang::from_pref`](crate::i18n::Lang::from_pref).
    pub fn language_label(code: &str, installed: Option<crate::i18n::Lang>) -> String {
        match crate::i18n::Lang::from_code(code) {
            Some(lang) if lang.needs_cjk_font() && installed == Some(lang) => format!("{} ({})", lang.english_name(), lang.name()),
            Some(lang) if lang.needs_cjk_font() => lang.english_name().to_string(),
            Some(lang) => lang.name().to_string(),
            None => t("Auto").to_string(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            install_locations: Default::default(),
            view: ViewMode::Grid,
            filter: Filter::All,
            sort: SortKey::Suite,
            sort_descending: false,
            theme: ThemeChoice::System,
            language: "auto".to_string(),
            close_to_tray: true,
            show_on_current_desktop: true,
            check_every: CheckEvery::SixHours,
            auto_update: true,
            manual_updates: std::collections::BTreeSet::new(),
            rolled_back: BTreeMap::new(),
            notify: true,
            prereleases: false,
            preferred_kinds: Vec::new(),
            dismissed_notices: std::collections::BTreeSet::new(),
            notified: BTreeMap::new(),
            last_check: None,
            hidden: std::collections::BTreeSet::new(),
            launches: BTreeMap::new(),
            channels: BTreeMap::new(),
            launch: BTreeMap::new(),
            crash_reports: false,
        }
    }
}
