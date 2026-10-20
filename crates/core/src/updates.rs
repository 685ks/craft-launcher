//! Smart updating, as pure logic: when to look for updates ([`Scheduler`]), which staged updates
//! may be put in place now and which wait for their app to quit ([`PendingApply`]), and when a
//! freshly updated app that keeps failing to start is rolled back ([`CrashGuard`]).
//!
//! No clocks, threads or I/O here: the callers pass in the time and what they see, so every rule
//! is tested with plain numbers.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// How often to look for updates unless the user picks otherwise: every six hours.
pub const DEFAULT_CHECK_EVERY: i64 = 6 * 3600;
/// The first retry after a check found no network; it doubles each time…
pub const OFFLINE_RETRY_FIRST: i64 = 60;
/// …up to this.
pub const OFFLINE_RETRY_MAX: i64 = 30 * 60;

/// A check still unfinished after this long is given up on.
pub const STALE_CHECK: i64 = 10 * 60;

/// How a check ended, as far as the schedule cares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckOutcome {
    /// Nothing could be reached: every answer came from saved results because of errors.
    pub offline: bool,
    /// GitHub asked us to hold off until then (Unix seconds).
    pub rate_limited_until: Option<i64>,
}

/// A check the schedule wants run now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Due {
    /// Skip the backend's freshness windows: set for the retries after the network went away,
    /// so the first one that gets through counts as "network regained" and asks everything.
    pub force: bool,
}

/// When to look for updates: once on start, then every `every` seconds (none: only when asked),
/// sooner when the network comes back (retried with backoff while it's away), and never while
/// GitHub has asked us to hold off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scheduler {
    every: Option<i64>,
    /// The last check that reached the network (Unix seconds).
    last_ok: Option<i64>,
    /// A check is under way, so it isn't started again.
    running: bool,
    started: bool,
    /// The last check found no network: retries back off from [`OFFLINE_RETRY_FIRST`].
    offline_since: Option<i64>,
    retries: u32,
    last_attempt: Option<i64>,
    hold_until: Option<i64>,
}

impl Scheduler {
    /// `last_ok` is the saved time of the last good check, so a restart doesn't reset the clock.
    pub fn new(every: Option<i64>, last_ok: Option<i64>) -> Self {
        Self { every, last_ok, running: false, started: false, offline_since: None, retries: 0, last_attempt: None, hold_until: None }
    }

    pub fn set_every(&mut self, every: Option<i64>) {
        self.every = every;
    }

    pub fn every(&self) -> Option<i64> {
        self.every
    }

    /// The network was lost on the last check.
    pub fn offline(&self) -> bool {
        self.offline_since.is_some()
    }

    /// Whether a check should start now, and how.
    pub fn due(&self, now: i64) -> Option<Due> {
        // A check the backend never finished (it was refused, or the answer got lost) stops
        // counting as running after a while.
        if self.running && self.last_attempt.is_some_and(|t| now - t < STALE_CHECK) {
            return None;
        }
        if !self.started {
            return Some(Due { force: false });
        }
        if self.hold_until.is_some_and(|t| now < t) {
            return None;
        }
        let every = self.every?;
        if self.offline_since.is_some() {
            let wait = OFFLINE_RETRY_FIRST.saturating_mul(1i64 << self.retries.min(10)).min(OFFLINE_RETRY_MAX);
            return self.last_attempt.is_none_or(|t| now - t >= wait).then_some(Due { force: true });
        }
        // A hold that has just ended: catch up at once.
        if self.hold_until.is_some() {
            return Some(Due { force: false });
        }
        self.last_ok.is_none_or(|t| now - t >= every).then_some(Due { force: false })
    }

    /// A check started (scheduled or asked for).
    pub fn started(&mut self, now: i64) {
        self.started = true;
        self.running = true;
        self.last_attempt = Some(now);
    }

    /// A check ended.
    pub fn finished(&mut self, now: i64, outcome: CheckOutcome) {
        self.started = true;
        self.running = false;
        self.last_attempt = Some(now);
        self.hold_until = outcome.rate_limited_until.filter(|t| *t > now);
        if outcome.offline {
            if self.offline_since.is_some() {
                self.retries = self.retries.saturating_add(1);
            } else {
                self.offline_since = Some(now);
                self.retries = 0;
            }
        } else {
            self.offline_since = None;
            self.retries = 0;
            self.last_ok = Some(now);
        }
    }
}

/// An update downloaded and checked, waiting for its app to quit before it is put in place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub version: String,
    /// Open the app again once it's updated ("Restart to update").
    pub relaunch: bool,
}

/// Updates waiting for their apps to quit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingApply {
    apps: BTreeMap<String, Pending>,
}

impl PendingApply {
    /// Queues `app`'s update to `version` (replacing an older queued one). `relaunch` sticks once set.
    pub fn queue(&mut self, app: &str, version: &str, relaunch: bool) {
        let relaunch = relaunch || self.apps.get(app).is_some_and(|p| p.relaunch && p.version == version);
        self.apps.insert(app.to_string(), Pending { version: version.to_string(), relaunch });
    }

    pub fn get(&self, app: &str) -> Option<&Pending> {
        self.apps.get(app)
    }

    pub fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }

    pub fn ids(&self) -> Vec<String> {
        self.apps.keys().cloned().collect()
    }

    /// Takes out `app`'s queued update: it's being applied now (the user opened the app from the
    /// launcher, or it quit), or no longer wanted.
    pub fn take(&mut self, app: &str) -> Option<Pending> {
        self.apps.remove(app)
    }

    /// Takes out every queued update whose app isn't running now.
    pub fn ready(&mut self, running: &dyn Fn(&str) -> bool) -> Vec<(String, Pending)> {
        let done: Vec<String> = self.apps.keys().filter(|id| !running(id)).cloned().collect();
        done.into_iter().filter_map(|id| self.apps.remove(&id).map(|p| (id, p))).collect()
    }
}

/// How long a freshly updated app must keep running to count as started.
pub const START_WINDOW_SECS: u64 = 20;
/// Failed starts in a row, after an update, that roll it back.
pub const CRASH_STRIKES: u8 = 2;

/// What [`CrashGuard::exited`] concluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing to do: not freshly updated, or it ran fine.
    Fine,
    /// A failed start; this many so far.
    Strike(u8),
    /// It failed to start [`CRASH_STRIKES`] times since updating to `from`: put `to` back.
    RollBack { from: String, to: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Watch {
    version: String,
    previous: String,
    strikes: u8,
}

/// Watches apps just updated by the launcher: an app that exits with an error within
/// [`START_WINDOW_SECS`] of being opened, [`CRASH_STRIKES`] times in a row, is rolled back to the
/// version it was updated from. Running for longer once settles it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrashGuard {
    apps: BTreeMap<String, Watch>,
}

impl CrashGuard {
    /// `app` was just updated from `previous` to `version` (and `previous` is kept to roll back to).
    pub fn updated(&mut self, app: &str, version: &str, previous: &str) {
        self.apps.insert(app.to_string(), Watch { version: version.to_string(), previous: previous.to_string(), strikes: 0 });
    }

    pub fn forget(&mut self, app: &str) {
        self.apps.remove(app);
    }

    pub fn watching(&self, app: &str) -> bool {
        self.apps.contains_key(app)
    }

    /// `app` at `version` exited after `ran_for` seconds, successfully or not.
    pub fn exited(&mut self, app: &str, version: Option<&str>, ran_for: u64, success: bool) -> Verdict {
        let Some(watch) = self.apps.get_mut(app) else { return Verdict::Fine };
        if version.is_some_and(|v| v != watch.version) {
            // Something else is installed now: the watch is stale.
            self.apps.remove(app);
            return Verdict::Fine;
        }
        if ran_for >= START_WINDOW_SECS {
            self.apps.remove(app);
            return Verdict::Fine;
        }
        if success {
            // Quit quickly on purpose: neither a crash nor proof it works.
            return Verdict::Fine;
        }
        watch.strikes = watch.strikes.saturating_add(1);
        if watch.strikes >= CRASH_STRIKES {
            let w = watch.clone();
            self.apps.remove(app);
            return Verdict::RollBack { from: w.version, to: w.previous };
        }
        Verdict::Strike(watch.strikes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3600;

    #[test]
    fn checks_on_start_then_every_six_hours() {
        let mut s = Scheduler::new(Some(DEFAULT_CHECK_EVERY), Some(1000));
        // On start, however recent the last check.
        assert_eq!(s.due(1001), Some(Due { force: false }));
        s.started(1001);
        assert_eq!(s.due(1002), None, "a running check isn't started twice");
        assert!(s.clone().due(1001 + 6 * H).is_some(), "but one that never finished doesn't block the schedule");
        s.finished(1010, CheckOutcome::default());
        assert_eq!(s.due(1010 + 6 * H - 1), None);
        assert_eq!(s.due(1010 + 6 * H), Some(Due { force: false }));
        s.set_every(Some(H));
        assert_eq!(s.due(1010 + H), Some(Due { force: false }));
    }

    #[test]
    fn only_when_asked_still_checks_once_on_start() {
        let mut s = Scheduler::new(None, None);
        assert!(s.due(0).is_some());
        s.started(0);
        s.finished(1, CheckOutcome::default());
        assert_eq!(s.due(100 * H), None);
    }

    #[test]
    fn offline_retries_back_off_and_the_first_success_is_network_regained() {
        let mut s = Scheduler::new(Some(DEFAULT_CHECK_EVERY), None);
        s.started(0);
        s.finished(0, CheckOutcome { offline: true, rate_limited_until: None });
        assert!(s.offline());
        assert_eq!(s.due(59), None);
        assert_eq!(s.due(60), Some(Due { force: true }));
        s.started(60);
        s.finished(60, CheckOutcome { offline: true, rate_limited_until: None });
        assert_eq!(s.due(60 + 119), None);
        assert_eq!(s.due(60 + 120), Some(Due { force: true }));
        // Many failures: capped at half an hour.
        for _ in 0..20 {
            s.finished(1000, CheckOutcome { offline: true, rate_limited_until: None });
        }
        assert_eq!(s.due(1000 + OFFLINE_RETRY_MAX - 1), None);
        assert!(s.due(1000 + OFFLINE_RETRY_MAX).is_some());
        // Back online: the regular schedule again, counted from this check.
        s.finished(5000, CheckOutcome::default());
        assert!(!s.offline());
        assert_eq!(s.due(5000 + 60), None);
        assert!(s.due(5000 + 6 * H).is_some());
    }

    #[test]
    fn a_rate_limit_holds_every_check_off_until_it_ends() {
        let mut s = Scheduler::new(Some(H), None);
        s.started(0);
        s.finished(0, CheckOutcome { offline: false, rate_limited_until: Some(2 * H) });
        assert_eq!(s.due(H), None, "due by the clock, but GitHub said wait");
        assert_eq!(s.due(2 * H), Some(Due { force: false }));
        // Offline retries wait for it too.
        s.finished(0, CheckOutcome { offline: true, rate_limited_until: Some(H) });
        assert_eq!(s.due(H - 1), None);
        assert!(s.due(H).is_some());
    }

    #[test]
    fn queued_updates_wait_while_their_app_runs() {
        let mut p = PendingApply::default();
        p.queue("photocraft", "0.6.0", false);
        p.queue("wordcraft", "0.4.0", true);
        let running = |id: &str| id == "photocraft";
        let ready = p.ready(&running);
        assert_eq!(ready, [("wordcraft".to_string(), Pending { version: "0.4.0".into(), relaunch: true })]);
        assert_eq!(p.ids(), ["photocraft"]);
        assert!(p.ready(&running).is_empty(), "still running: still queued");
        // "Restart to update" sticks to the same version, a newer version resets it.
        p.queue("photocraft", "0.6.0", true);
        p.queue("photocraft", "0.6.0", false);
        assert!(p.get("photocraft").unwrap().relaunch);
        p.queue("photocraft", "0.6.1", false);
        assert!(!p.get("photocraft").unwrap().relaunch);
        // It quit: applied.
        assert_eq!(p.ready(&|_: &str| false).len(), 1);
        assert!(p.is_empty());
        // Opened from the launcher: taken out to apply first.
        p.queue("deckcraft", "1.0.0", false);
        assert_eq!(p.take("deckcraft").map(|p| p.version), Some("1.0.0".into()));
        assert!(p.take("deckcraft").is_none());
    }

    #[test]
    fn two_failed_starts_after_an_update_roll_it_back() {
        let mut g = CrashGuard::default();
        g.updated("photocraft", "0.6.0", "0.5.2");
        assert_eq!(g.exited("photocraft", Some("0.6.0"), 3, false), Verdict::Strike(1));
        assert_eq!(g.exited("photocraft", Some("0.6.0"), 2, false), Verdict::RollBack { from: "0.6.0".into(), to: "0.5.2".into() });
        assert!(!g.watching("photocraft"));
        assert_eq!(g.exited("photocraft", Some("0.6.0"), 1, false), Verdict::Fine, "only once");
    }

    #[test]
    fn a_good_start_or_a_quick_clean_quit_is_not_a_crash() {
        let mut g = CrashGuard::default();
        g.updated("photocraft", "0.6.0", "0.5.2");
        assert_eq!(g.exited("photocraft", Some("0.6.0"), 1, true), Verdict::Fine);
        assert!(g.watching("photocraft"), "a clean quick quit proves nothing");
        assert_eq!(g.exited("photocraft", Some("0.6.0"), 1, false), Verdict::Strike(1));
        // Ran past the window: settled, a later crash isn't the update's fault.
        assert_eq!(g.exited("photocraft", Some("0.6.0"), START_WINDOW_SECS, false), Verdict::Fine);
        assert!(!g.watching("photocraft"));
        // Another version installed since: the watch is dropped.
        g.updated("wordcraft", "0.4.0", "0.3.0");
        assert_eq!(g.exited("wordcraft", Some("0.3.0"), 1, false), Verdict::Fine);
        assert!(!g.watching("wordcraft"));
        assert_eq!(g.exited("nothing", None, 0, false), Verdict::Fine);
    }

    #[test]
    fn the_guard_survives_a_restart() {
        let mut g = CrashGuard::default();
        g.updated("photocraft", "0.6.0", "0.5.2");
        let _ = g.exited("photocraft", Some("0.6.0"), 1, false);
        let back: CrashGuard = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(back, g);
    }
}
