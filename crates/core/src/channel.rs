//! Release channels: which of an app's releases count as its latest.
//!
//! - **Stable**: published releases with a plain version (`0.6.0`).
//! - **Beta**: stable releases and GitHub pre-releases (`0.7.0-beta.1`, `0.7.0-rc.2`), whichever
//!   has the highest version.
//! - **Nightly**: everything, nightly builds included (a tag or version naming `nightly`, `dev`
//!   or `canary`), the most recently published first.
//!
//! Each app follows its own channel (Settings keeps them per app); without one it follows the
//! "Include pre-releases" setting: Beta when that's on, Stable otherwise.

use serde::{Deserialize, Serialize};

use crate::release::Release;
use crate::version;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Channel {
    #[default]
    Stable,
    Beta,
    Nightly,
}

impl Channel {
    pub const ALL: [Self; 3] = [Self::Stable, Self::Beta, Self::Nightly];

    /// English, the UI's lookup key.
    pub fn label(self) -> &'static str {
        match self {
            Self::Stable => "Stable",
            Self::Beta => "Beta",
            Self::Nightly => "Nightly",
        }
    }

    /// The channel an app without its own follows.
    pub fn default_for(prereleases: bool) -> Self {
        if prereleases { Self::Beta } else { Self::Stable }
    }

    /// Whether more than stable releases are on offer (GitHub's release list must be read: the
    /// release feed and `releases/latest` only know stable releases).
    pub fn wants_prereleases(self) -> bool {
        self != Self::Stable
    }

    /// Whether `release` belongs to this channel.
    pub fn accepts(self, release: &Release) -> bool {
        let Some(v) = release.version.as_ref() else { return false };
        let stable = !release.prerelease && version::is_stable(v);
        match self {
            Self::Stable => stable,
            Self::Beta => stable || !is_nightly(release),
            Self::Nightly => true,
        }
    }
}

/// Whether `release` is a nightly build: its tag or pre-release label says `nightly`, `dev` or
/// `canary`.
pub fn is_nightly(release: &Release) -> bool {
    let tag = release.tag.to_ascii_lowercase();
    let pre = release.version.as_ref().map(|v| v.pre.as_str().to_ascii_lowercase()).unwrap_or_default();
    ["nightly", "dev", "canary"].iter().any(|w| tag.contains(w) || pre.split('.').any(|part| part.starts_with(w)))
}

/// The latest release of `channel` among `releases`: the highest version for Stable and Beta,
/// the most recently published for Nightly (nightlies often share a version).
pub fn pick(releases: impl IntoIterator<Item = Release>, channel: Channel) -> Option<Release> {
    let mut best: Option<Release> = None;
    for r in releases.into_iter().filter(|r| channel.accepts(r)) {
        let better = match (&best, channel) {
            (None, _) => true,
            (Some(b), Channel::Nightly) if r.published_at.is_some() || b.published_at.is_some() => {
                r.published_at.unwrap_or(i64::MIN) > b.published_at.unwrap_or(i64::MIN)
            }
            (Some(b), _) => match (r.version.as_ref(), b.version.as_ref()) {
                (Some(v), Some(bv)) => version::is_newer(v, bv),
                _ => false,
            },
        };
        if better {
            best = Some(r);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(tag: &str, prerelease: bool, at: i64) -> Release {
        Release { tag: tag.into(), version: version::parse(tag), prerelease, published_at: Some(at), ..Release::default() }
    }

    fn all() -> Vec<Release> {
        vec![
            rel("v0.5.1", false, 10),
            rel("v0.6.0", false, 20),
            rel("v0.7.0-beta.1", true, 30),
            rel("v0.7.0-nightly.20261019", true, 50),
            rel("v0.7.0-rc.1", true, 40),
        ]
    }

    #[test]
    fn each_channel_picks_its_own_latest() {
        assert_eq!(pick(all(), Channel::Stable).unwrap().tag, "v0.6.0");
        assert_eq!(pick(all(), Channel::Beta).unwrap().tag, "v0.7.0-rc.1");
        assert_eq!(pick(all(), Channel::Nightly).unwrap().tag, "v0.7.0-nightly.20261019");
        assert_eq!(pick(Vec::new(), Channel::Beta), None);
    }

    #[test]
    fn a_newer_stable_release_wins_over_an_older_beta() {
        let list = vec![rel("v0.7.0-beta.1", true, 30), rel("v0.7.0", false, 60)];
        assert_eq!(pick(list.clone(), Channel::Beta).unwrap().tag, "v0.7.0");
        assert_eq!(pick(list, Channel::Nightly).unwrap().tag, "v0.7.0");
    }

    #[test]
    fn nightlies_are_recognised_by_tag_or_label() {
        assert!(is_nightly(&rel("v0.7.0-nightly.1", true, 0)));
        assert!(is_nightly(&rel("v0.7.0-dev.3", true, 0)));
        assert!(is_nightly(&Release { tag: "nightly".into(), version: version::parse("0.7.0-1"), ..Release::default() }));
        assert!(!is_nightly(&rel("v0.7.0-beta.1", true, 0)));
        assert!(!is_nightly(&rel("v0.7.0-rc.1", true, 0)));
        // A tagged release marked as a pre-release on GitHub is a beta even with a plain version.
        assert!(!Channel::Stable.accepts(&rel("v0.7.0", true, 0)));
        assert!(Channel::Beta.accepts(&rel("v0.7.0", true, 0)));
    }

    #[test]
    fn the_default_follows_the_prerelease_setting() {
        assert_eq!(Channel::default_for(false), Channel::Stable);
        assert_eq!(Channel::default_for(true), Channel::Beta);
        assert!(!Channel::Stable.wants_prereleases());
        assert!(Channel::Nightly.wants_prereleases());
    }
}
