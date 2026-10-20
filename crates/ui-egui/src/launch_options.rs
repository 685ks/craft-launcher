//! The "Launch options" dialog: extra arguments and environment variables an app starts with.

use artcraft_launcher_core::launch::LaunchProfile;
use egui::{RichText, Ui};

use crate::app::{Dialog, LauncherUi};
use crate::theme::{self, Tokens};
use crate::widgets::{self, Kind};

impl LauncherUi {
    /// Opens the dialog for `id`, filled with what's saved.
    pub(crate) fn open_launch_options(&mut self, id: &str) {
        let profile = self.settings.launch.get(id).cloned().unwrap_or_default();
        self.dialog = Some(Dialog::LaunchOptions { app: id.to_string(), args: profile.args, env: profile.env });
    }

    pub(crate) fn launch_options_dialog(&mut self, ui: &mut Ui, id: &str, args: &mut String, env: &mut String) -> bool {
        let Some(app) = self.entry(id).map(|e| e.app) else { return true };
        let t = Tokens::get(ui.ctx());
        ui.set_width(520.0f32.min(ui.ctx().content_rect().width() - 80.0));
        Self::title(ui, crate::icons::app(id), app.name, tl!("Launch options"));

        let label = ui.label(RichText::new(tl!("Arguments")).color(t.ink_strong));
        ui.add(
            egui::TextEdit::singleline(args)
                .id_salt("launch-args")
                .font(egui::FontId::new(13.0, theme::mono()))
                .hint_text("--verbose")
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id);
        ui.label(
            RichText::new(tl!("Added after the app's name when it starts, as on a command line. Put quotes around an argument with spaces."))
                .size(12.0)
                .color(t.muted),
        );
        ui.add_space(10.0);
        let label = ui.label(RichText::new(tl!("Environment variables")).color(t.ink_strong));
        ui.add(
            egui::TextEdit::multiline(env)
                .id_salt("launch-env")
                .font(egui::FontId::new(13.0, theme::mono()))
                .hint_text("RUST_LOG=info")
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id);
        ui.label(RichText::new(tl!("One NAME=value per line. Lines starting with # are ignored.")).size(12.0).color(t.muted));

        let profile = LaunchProfile { args: args.clone(), env: env.clone() };
        let problem = profile.parse().err();
        if let Some(e) = &problem {
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(crate::i18n::msg(e)).size(12.5).color(t.bad)).wrap());
        }
        ui.add_space(16.0);
        let mut close = false;
        ui.horizontal(|ui| {
            if widgets::button(ui, tl!("Clear all"), Kind::Secondary, 0.0).clicked() {
                args.clear();
                env.clear();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let save = ui.add_enabled_ui(problem.is_none(), |ui| widgets::button(ui, tl!("Save"), Kind::Primary, 90.0)).inner;
                if save.clicked() {
                    if profile.is_empty() {
                        self.settings.launch.remove(id);
                    } else {
                        self.settings.launch.insert(id.to_string(), profile.clone());
                    }
                    self.configure();
                    close = true;
                }
                if widgets::button(ui, tl!("Cancel"), Kind::Secondary, 90.0).clicked() {
                    close = true;
                }
            });
        });
        close
    }
}
