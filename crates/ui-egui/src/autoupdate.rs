//! Smart updating, the UI's half: which apps update by themselves, staging their downloads in the
//! background, putting them in place (now, or once the app quits), "Update all", and the
//! notifications that say an update is ready.
//!
//! The backend does the work and the waiting (`Command::Stage`, `Command::ApplyUpdate`); the
//! schedule is `core::updates::Scheduler`, driven from `LauncherUi::logic`.

use artcraft_launcher_core::InstallSource;
use artcraft_launcher_core::catalog::LAUNCHER_ID;
use artcraft_launcher_core::protocol::Command;

use crate::app::LauncherUi;
use crate::i18n::fmt;
use crate::model::AppEntry;
use crate::settings::Filter;

/// A launcher update found staged this soon after starting was downloaded in an earlier run: it is
/// put in place right away ("on next launch").
const APPLY_LAUNCHER_ON_START: i64 = 3 * 60;

impl LauncherUi {
    /// Whether `e` updates by itself: it has an update, automatic updates are on for it, it was
    /// installed in a way the launcher updates (not an upstream build, which is only switched when
    /// asked, nor an AUR or unknown system binary), and the update isn't a version that was rolled
    /// back for failing to start.
    pub(crate) fn auto_eligible(&self, e: &AppEntry) -> bool {
        let Some(installed) = e.installed.as_ref() else { return false };
        self.caps.manage_apps
            && self.settings.updates_automatically(e.app.id)
            && self.in_filter(e, Filter::Updates)
            && !installed.upstream
            && matches!(
                installed.source,
                InstallSource::Launcher
                    | InstallSource::MacApp
                    | InstallSource::Msi
                    | InstallSource::AppImage
                    | InstallSource::Deb
                    | InstallSource::Rpm
                    | InstallSource::Flatpak
            )
            && e.latest_label().is_some_and(|v| self.settings.rolled_back.get(e.app.id) != Some(&v))
    }

    /// Whether `e`'s staged update can be put in place without asking: the launcher replaces the
    /// files itself. System packages (deb, rpm, Flatpak) need the system's installer and its
    /// password prompt, so they wait for the user ("Update all" or the card's button).
    fn applies_silently(e: &AppEntry) -> bool {
        e.installed.as_ref().is_some_and(|i| i.source.launcher_can_update())
    }

    /// Moves every automatic update one step: download and verify it in the background, then put
    /// it in place (the backend waits for a running app to quit). Each step is tried once per
    /// version per session (a failure retries after "Check for updates").
    pub(crate) fn trigger_auto_updates(&mut self) {
        if !self.caps.manage_apps || !self.settings.auto_update {
            return;
        }
        let mut stage = Vec::new();
        let mut apply = Vec::new();
        for e in &self.entries {
            if !self.auto_eligible(e) || e.job.is_some() || e.queued.is_some() || e.paused.is_some() {
                continue;
            }
            let Some(v) = e.latest_label() else { continue };
            if e.ready {
                if Self::applies_silently(e) && !self.auto_update_attempted.contains(&(format!("apply:{}", e.app.id), v.clone())) {
                    apply.push((e.app.id.to_string(), v));
                }
            } else if !self.auto_update_attempted.contains(&(e.app.id.to_string(), v.clone())) {
                stage.push((e.app.id.to_string(), v));
            }
        }
        for (id, v) in stage {
            log::info!("auto-update: downloading {id} {v} in the background");
            self.auto_update_attempted.insert((id.clone(), v));
            self.background.insert(id.clone());
            self.send(Command::Stage { app: id });
        }
        for (id, v) in apply {
            log::info!("auto-update: putting {id} {v} in place");
            self.auto_update_attempted.insert((format!("apply:{id}"), v));
            self.send(Command::ApplyUpdate { app: id, relaunch: false });
        }
        // The launcher: staged the same way, put in place on the next start or "Restart to update".
        if self.caps.self_update
            && let Some(r) = &self.launcher_update
            && self.launcher_job.is_none()
        {
            let v = r.version_label();
            let now = self.backend.now();
            if self.launcher_staged.as_ref() == Some(&v) {
                let key = (format!("apply:{LAUNCHER_ID}"), v.clone());
                if now - self.started_at < APPLY_LAUNCHER_ON_START && !self.auto_update_attempted.contains(&key) {
                    log::info!("auto-update: the launcher's update to {v} was ready at start; restarting into it");
                    self.auto_update_attempted.insert(key);
                    self.send(Command::UpdateLauncher);
                }
            } else if !self.auto_update_attempted.contains(&(LAUNCHER_ID.to_string(), v.clone())) {
                log::info!("auto-update: downloading Arthavan Projects {v} in the background");
                self.auto_update_attempted.insert((LAUNCHER_ID.to_string(), v));
                self.send(Command::StageLauncher);
            }
        }
    }

    /// Apps "Update all" would update: every installed app with an update and nothing under way.
    pub(crate) fn updatable_ids(&self) -> Vec<&'static str> {
        self.entries.iter().filter(|e| self.in_filter(e, Filter::Updates) && e.job.is_none() && e.queued.is_none()).map(|e| e.app.id).collect()
    }

    /// "Update all": every app with an update, now or (if it runs) once it quits. Downloads go two
    /// at a time.
    pub(crate) fn update_all(&mut self) {
        for id in self.updatable_ids() {
            self.background.remove(id);
            self.send(Command::ApplyUpdate { app: id.to_string(), relaunch: false });
        }
    }

    /// "Restart to update": the app is running; once it quits, it's updated and opened again.
    pub(crate) fn restart_to_update(&mut self, id: &str) {
        self.send(Command::ApplyUpdate { app: id.to_string(), relaunch: true });
    }

    /// A freshly updated app kept failing to start and was rolled back.
    pub(crate) fn rolled_back(&mut self, ctx: &egui::Context, app: &str, from: &str, to: &str) {
        let Some(name) = self.entry(app).map(|e| e.app.name) else { return };
        self.settings.rolled_back.insert(app.to_string(), from.to_string());
        let text = fmt(tl!("{name} {from} kept failing to start, so it was rolled back to {to}."), &[("name", name), ("from", from), ("to", to)]);
        self.toast(ctx, text.clone(), true);
        self.notify_queue.push((name.to_string(), text));
    }

    /// Notifications for updates that are ready: downloaded and waiting for their app to quit
    /// ("Restart to update"), downloaded and waiting for the system installer, or the launcher's
    /// own. Each version once per session.
    pub(crate) fn announce_ready(&mut self, host: &mut dyn crate::app::Host) {
        let mut ready: Vec<(String, String, String)> = Vec::new();
        for e in &self.entries {
            let Some(v) = e.latest_label() else { continue };
            if !self.in_filter(e, Filter::Updates) {
                continue;
            }
            let body = if e.queued.is_some() {
                fmt(tl!("Restart {name} to finish updating."), &[("name", e.app.name)])
            } else if e.is_staged() && !Self::applies_silently(e) && self.auto_eligible(e) {
                tl!("Open Arthavan Projects to update.").to_string()
            } else {
                continue;
            };
            if !self.ready_announced.contains(&(e.app.id.to_string(), v.clone())) {
                ready.push((e.app.id.to_string(), fmt(tl!("{name} {v} is ready"), &[("name", e.app.name), ("v", &v)]), body));
                self.ready_announced.insert((e.app.id.to_string(), v));
            }
        }
        // Staged at start with automatic updates on: it goes in place at once, nothing to announce.
        let applying_now = self.settings.auto_update && self.backend.now() - self.started_at < APPLY_LAUNCHER_ON_START;
        if let Some(v) = self.launcher_staged.clone()
            && self.launcher_job.is_none()
            && !applying_now
            && !self.ready_announced.contains(&(LAUNCHER_ID.to_string(), v.clone()))
        {
            self.ready_announced.insert((LAUNCHER_ID.to_string(), v.clone()));
            ready.push((
                LAUNCHER_ID.to_string(),
                fmt(tl!("{name} {v} is ready"), &[("name", "Arthavan Projects"), ("v", &v)]),
                tl!("Restart to update").to_string(),
            ));
        }
        let queued = std::mem::take(&mut self.notify_queue);
        if !self.settings.notify {
            return;
        }
        for (_, title, body) in ready {
            host.notify(&title, &body);
        }
        for (title, body) in queued {
            host.notify(&title, &body);
        }
    }
}
