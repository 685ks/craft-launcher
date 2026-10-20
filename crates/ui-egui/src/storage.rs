//! The disk usage dialog: what each installed app and the launcher's cache take, and clearing
//! the cache after asking.

use artcraft_launcher_core::protocol::{Command, DiskUsage};
use egui::{RichText, Ui};

use crate::app::{Dialog, LauncherUi};
use crate::i18n::fmt;
use crate::theme::Tokens;
use crate::widgets::{self, Kind};

/// One line of the usage list: a name at the left, its size at the right.
fn usage_row(ui: &mut Ui, name: &str, value: &str, strong: bool) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).color(if strong { t.ink_strong } else { t.ink }));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).family(crate::theme::mono()).color(t.muted));
        });
    });
}

impl LauncherUi {
    /// Opens the disk usage dialog and asks the backend for fresh numbers.
    pub(crate) fn open_storage(&mut self) {
        self.dialog = Some(Dialog::Storage);
        self.send(Command::MeasureDisk);
    }

    pub(crate) fn storage_dialog(&mut self, ui: &mut Ui) -> bool {
        let t = Tokens::get(ui.ctx());
        ui.set_width(460.0f32.min(ui.ctx().content_rect().width() - 80.0));
        Self::title(ui, Some(crate::icons::mark()), tl!("Disk usage"), "");
        let Some(usage) = self.disk_usage.clone() else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(tl!("Measuring…")).color(t.muted));
            });
            ui.add_space(14.0);
            let mut close = false;
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = widgets::button(ui, tl!("Close"), Kind::Secondary, 90.0).clicked();
                });
            });
            return close;
        };
        widgets::hud(ui, tl!("Installed apps"), t.faint);
        ui.add_space(4.0);
        let installed: Vec<(&'static str, &'static str)> =
            self.entries.iter().filter(|e| e.installed.is_some()).map(|e| (e.app.id, e.app.name)).collect();
        if installed.is_empty() {
            ui.label(RichText::new(tl!("Nothing installed yet")).color(t.muted));
        }
        for (id, name) in installed {
            let size = usage.apps.get(id).map_or_else(|| tl!("Managed by the system").to_string(), |b| widgets::human_bytes(*b));
            usage_row(ui, name, &size, false);
        }
        ui.add_space(12.0);
        widgets::hud(ui, tl!("Cache"), t.faint);
        ui.add_space(4.0);
        usage_row(ui, tl!("Downloads"), &widgets::human_bytes(usage.downloads), false);
        usage_row(ui, tl!("Saved pictures"), &widgets::human_bytes(usage.pictures), false);
        usage_row(ui, tl!("Cache in all"), &widgets::human_bytes(usage.cache), true);
        ui.add_space(18.0);
        let mut close = false;
        ui.horizontal(|ui| {
            let busy = self.busy();
            let clearable = usage.downloads.saturating_add(usage.pictures) > 0;
            let clear = ui
                .add_enabled_ui(clearable && !busy, |ui| widgets::button(ui, tl!("Clear cache…"), Kind::Danger, 0.0))
                .inner
                .on_disabled_hover_text(if busy { tl!("Wait for downloads and installs to finish.") } else { tl!("The cache is empty.") });
            if clear.clicked() {
                // Takes the dialog's place (see `dialogs`).
                self.dialog = Some(Dialog::ConfirmClearCache);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close = widgets::button(ui, tl!("Close"), Kind::Secondary, 90.0).clicked();
            });
        });
        close
    }

    pub(crate) fn confirm_clear_cache_dialog(&mut self, ui: &mut Ui) -> bool {
        let t = Tokens::get(ui.ctx());
        ui.set_width(440.0f32.min(ui.ctx().content_rect().width() - 80.0));
        Self::title(ui, Some(crate::icons::mark()), tl!("Clear the cache?"), "");
        let size = self.disk_usage.as_ref().map_or(0, |u| u.downloads.saturating_add(u.pictures));
        let text = fmt(
            tl!(
                "This removes {size} of downloads and saved pictures. Paused downloads start again from the beginning. Installed apps aren't affected."
            ),
            &[("size", &widgets::human_bytes(size))],
        );
        ui.add(egui::Label::new(RichText::new(text).color(t.muted)).wrap());
        ui.add_space(20.0);
        // Both buttons lead back to the usage list rather than closing.
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, tl!("Clear cache"), Kind::Danger, 0.0).clicked() {
                    self.send(Command::ClearCache);
                    self.disk_usage = None;
                    self.dialog = Some(Dialog::Storage);
                }
                if widgets::button(ui, tl!("Cancel"), Kind::Secondary, 90.0).clicked() {
                    self.dialog = Some(Dialog::Storage);
                }
            });
        });
        false
    }

    /// The cache was cleared (or wasn't): say so.
    pub(crate) fn cache_cleared(&mut self, ctx: &egui::Context, result: Result<u64, String>) {
        match result {
            Ok(freed) => self.toast(ctx, fmt(tl!("Cache cleared: {size} freed"), &[("size", &widgets::human_bytes(freed))]), false),
            Err(e) => self.toast(ctx, e, true),
        }
    }

    pub(crate) fn set_disk_usage(&mut self, usage: DiskUsage) {
        self.disk_usage = Some(usage);
    }
}
