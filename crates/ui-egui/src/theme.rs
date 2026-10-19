//! Colours, type and shapes from the house design system (`craft-tokens`): a near-black navy
//! page, a steel-blue accent, hairline white borders, rounded panels, and Inter for everything
//! but code. Dark is the house look; the light appearance is kept as an alternate. Widgets read
//! [`Tokens::get`]; nothing hard-codes a colour.

use std::sync::Arc;

use egui::epaint::text::VariationCoords;
use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Stroke, TextStyle, Visuals};
use serde::{Deserialize, Serialize};

use craft_tokens::color;

use crate::i18n::Lang;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    /// Follow the system's light or dark setting.
    System,
    /// The house look, and the default.
    #[default]
    Dark,
    Light,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];

    /// The English key translated at draw time into the current UI language
    /// (the shell sets it per frame via `i18n::set_current`). Same-crate lookup,
    /// pure and wasm-safe; no new dependency. Call sites stay unchanged.
    pub fn label(self) -> &'static str {
        match self {
            Self::System => crate::i18n::t("Match system"),
            Self::Dark => crate::i18n::t("Dark"),
            Self::Light => crate::i18n::t("Light"),
        }
    }
}

/// Colour tokens for one appearance. The dark one is the house design system's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    /// Page and card surface.
    pub bg: Color32,
    /// Badges, wells and row hovers.
    pub sunken: Color32,
    /// Body text.
    pub ink: Color32,
    /// Headings.
    pub ink_strong: Color32,
    /// Secondary text.
    pub muted: Color32,
    /// Labels and tertiary text.
    pub faint: Color32,
    /// Hairlines.
    pub line: Color32,
    /// Outlined buttons and stronger rules.
    pub line_strong: Color32,
    /// The brand blue, for surfaces.
    pub accent: Color32,
    /// The brand blue, for text and icons.
    pub accent_ink: Color32,
    /// Primary buttons and hover inversion.
    pub invert_bg: Color32,
    pub invert_fg: Color32,
    pub good: Color32,
    pub warn: Color32,
    pub bad: Color32,
}

const fn ink_alpha(rgb: u32, alpha: u8) -> Color32 {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color32::from_rgba_unmultiplied_const(r, g, b, alpha)
}

const fn hex(rgb: u32) -> Color32 {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color32::from_rgb(r, g, b)
}

impl Tokens {
    pub const DARK: Self = Self {
        dark: true,
        // The page (`--bg`); cards draw [`Tokens::panel`] on it.
        bg: color::BG_BASE.color32(),
        // Wells and row hovers: `--fill-input` on the page.
        sunken: hex(0x101216),
        ink: color::TEXT_INK.color32(),
        ink_strong: color::TEXT_STRONG.color32(),
        muted: color::TEXT_MUTED.color32(),
        faint: color::TEXT_FAINT.color32(),
        line: color::BORDER_LINE.color32(),
        line_strong: ink_alpha(0xffffff, 56),
        accent: color::ACCENT_BASE.color32(),
        accent_ink: color::ACCENT_BRIGHT.color32(),
        // The primary button: `--neon-hi` with dark text.
        invert_bg: color::TEXT_STRONG.color32(),
        invert_fg: color::TEXT_ON_ACCENT.color32(),
        good: color::STATUS_OK.color32(),
        warn: color::STATUS_WARN.color32(),
        bad: color::STATUS_ERROR.color32(),
    };

    pub const LIGHT: Self = Self {
        dark: false,
        bg: hex(0xf2f1ee),
        sunken: hex(0xe9e8e4),
        ink: hex(0x101014),
        ink_strong: hex(0x000000),
        muted: ink_alpha(0x101014, 158),
        faint: ink_alpha(0x101014, 107),
        line: ink_alpha(0x101014, 41),
        line_strong: ink_alpha(0x101014, 102),
        accent: hex(0x2d81ff),
        accent_ink: hex(0x1659c4),
        invert_bg: hex(0x101014),
        invert_fg: hex(0xf2f1ee),
        good: hex(0x0a7563),
        warn: hex(0x8a5800),
        bad: hex(0xb3261e),
    };

    pub fn get(ctx: &egui::Context) -> Self {
        if ctx.theme() == egui::Theme::Dark { Self::DARK } else { Self::LIGHT }
    }

    /// Card and panel bodies: the house `--panel` flattened onto the page in the dark look.
    pub fn panel(&self) -> Color32 {
        if self.dark { hex(0x0c0f15) } else { self.bg }
    }

    /// Reserved for "you / now / current" (the house gold); the light look uses its warn ink.
    pub fn gold(&self) -> Color32 {
        if self.dark { color::ACCENT_GOLD.color32() } else { self.warn }
    }

    pub fn hairline(&self) -> Stroke {
        Stroke::new(1.0, self.line)
    }

    /// An app's text colour (`--app-ink`): readable on this appearance's background.
    pub fn app_ink(&self, app: &artcraft_launcher_core::CraftApp) -> Color32 {
        rgb(if self.dark { app.ink_dark } else { app.ink_light })
    }
}

/// Mixes `c` into the background at `alpha` (0â€“1): tinted washes.
pub fn tint(c: Color32, alpha: f32) -> Color32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// The shadow under menus, dialogs and toasts: the house panel shadow, lighter in the light look.
pub fn shadow(t: &Tokens) -> egui::epaint::Shadow {
    let house: egui::epaint::Shadow = craft_tokens::shadow::PANEL.into();
    if t.dark { house } else { egui::epaint::Shadow { color: Color32::from_black_alpha(36), ..house } }
}

/// The dim over the page behind a drawer; `amount` runs from 0 (none) to 1.
pub fn scrim(t: &Tokens, amount: f32) -> Color32 {
    let alpha = if t.dark { 170.0 } else { 105.0 } * amount.clamp(0.0, 1.0);
    Color32::from_black_alpha(alpha.round() as u8)
}

/// Behind an enlarged picture: near-black in both appearances, as pictures are judged on black.
pub fn backdrop() -> Color32 {
    Color32::from_black_alpha(232)
}

/// A frame standing in for a picture, over [`backdrop`].
pub fn backdrop_panel() -> Color32 {
    Color32::from_white_alpha(14)
}

/// Ink on [`backdrop`], at `alpha` out of 255.
pub fn backdrop_ink(alpha: u8) -> Color32 {
    Color32::from_white_alpha(alpha)
}

fn visuals(t: &Tokens) -> Visuals {
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = t.bg;
    v.window_fill = t.panel();
    v.extreme_bg_color = t.sunken;
    v.text_edit_bg_color = Some(t.sunken);
    v.faint_bg_color = t.sunken;
    v.code_bg_color = t.sunken;
    v.window_stroke = Stroke::new(1.0, t.line_strong);
    v.window_corner_radius = CornerRadius::same(R_PANEL);
    v.menu_corner_radius = CornerRadius::same(R_FIELD);
    v.window_shadow = shadow(t);
    v.popup_shadow = shadow(t);
    v.override_text_color = Some(t.ink);
    v.hyperlink_color = t.accent_ink;
    v.selection.bg_fill = t.accent.gamma_multiply(0.3);
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.warn_fg_color = t.warn;
    v.error_fg_color = t.bad;
    for (w, fill, stroke, fg) in [
        (&mut v.widgets.noninteractive, t.bg, t.line, t.muted),
        (&mut v.widgets.inactive, t.sunken, t.line, t.ink),
        (&mut v.widgets.hovered, t.sunken, t.accent.gamma_multiply(0.45), t.ink_strong),
        (&mut v.widgets.active, t.sunken, t.accent, t.ink_strong),
        (&mut v.widgets.open, t.sunken, t.accent.gamma_multiply(0.45), t.ink_strong),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::new(1.0, stroke);
        w.corner_radius = CornerRadius::same(R_FIELD);
        w.fg_stroke = Stroke::new(1.0, fg);
        w.expansion = 0.0;
    }
    v
}

/// Applies both appearances (egui switches between them with the theme preference).
pub fn apply(ctx: &egui::Context, choice: ThemeChoice) {
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&Tokens::DARK));
    ctx.set_visuals_of(egui::Theme::Light, visuals(&Tokens::LIGHT));
    ctx.set_theme(match choice {
        ThemeChoice::System => egui::ThemePreference::System,
        ThemeChoice::Dark => egui::ThemePreference::Dark,
        ThemeChoice::Light => egui::ThemePreference::Light,
    });
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        // Between a checkbox (or radio button) and its label.
        style.spacing.icon_spacing = 10.0;
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = CONTROL_H;
        style.spacing.menu_margin = egui::Margin::same(4);
        style.spacing.window_margin = egui::Margin::same(4);
        style.text_styles = [
            (TextStyle::Heading, FontId::new(24.0, display())),
            (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.5, medium())),
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
    });
}

/// Height of buttons, inputs and filter cells.
pub const CONTROL_H: f32 = 36.0;

/// Corner radii from the house design system: icon buttons and toggles, inputs and buttons,
/// cards and dialogs, and pills (badges, tags, progress bars).
pub const R_CONTROL: u8 = craft_tokens::radius::LG as u8;
pub const R_FIELD: u8 = craft_tokens::radius::XL as u8;
pub const R_PANEL: u8 = craft_tokens::radius::PANEL as u8;
pub const R_PILL: u8 = u8::MAX;

/// The gap between cards in the grid.
pub const CARD_GAP: f32 = 14.0;

/// Labels: the house field label, 11 px Inter 600, upper case, 0.22 em apart.
pub const HUD_SIZE: f32 = 11.0;
pub const HUD_SPACING: f32 = HUD_SIZE * craft_tokens::font::LETTER_SPACING_LABEL_EM;
/// Buttons: 11 px Inter 700, upper case, 0.08 em apart (the house chip tracking).
pub const BUTTON_SPACING: f32 = HUD_SIZE * craft_tokens::font::LETTER_SPACING_CHIP_EM;

// ---- layout ------------------------------------------------------------------------------------
// The app page and the large dialogs (Settings, release notes) share these, so they read as one
// family; the toolbar and the footer use the same steps.

/// Padding inside the app page and the dialogs, round their headers, bodies and footers.
pub const PAD: f32 = 24.0;
/// The gap between the two columns of a page or dialog.
pub const COLUMN_GAP: f32 = 32.0;
/// Padding inside a hairline box: a group of settings, a feature, a release fact.
pub const BOX_MARGIN: egui::Margin = egui::Margin::symmetric(14, 12);
/// Below this width the toolbar moves the sort and "Update all" to a second row.
pub const TOOLBAR_COMPACT_BELOW: f32 = 640.0;
/// From this width the large dialogs read in two columns.
pub const DIALOG_COLUMNS_FROM: f32 = 760.0;
/// From this width the app page reads in two columns (its pictures need the room).
pub const PAGE_COLUMNS_FROM: f32 = 900.0;
/// Title sizes, largest first: an app page's name, a large dialog's title, a small prompt's title.
pub const PAGE_TITLE: f32 = 32.0;
pub const SHEET_TITLE: f32 = 26.0;
pub const DIALOG_TITLE: f32 = 22.0;
/// A row of icon links (the footer, Settings): the room round each logo, and the gap between their
/// targets; neighbours' logos sit `2 × ICON_LINK_PAD + ICON_LINK_GAP` apart.
pub const ICON_LINK_PAD: f32 = 6.0;
pub const ICON_LINK_GAP: f32 = 2.0;

/// Inter 500.
pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
}

/// Inter 600.
pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Inter 700: headings and app wordmarks (the house type uses one family throughout).
pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}

/// Inter 600: labels and badges.
pub fn mono() -> FontFamily {
    FontFamily::Name("mono".into())
}

/// Inter 700: buttons and app number tabs.
pub fn mono_bold() -> FontFamily {
    FontFamily::Name("mono-bold".into())
}

/// A font registered under `name`: its file, its variation coordinates and its family.
type Face<'a> = (&'a str, &'static [u8], &'a [([u8; 4], f32)], FontFamily);

/// Registers Inter at the weights the house type uses, and Geist Mono for code.
/// `cjk`: the CJK language to draw, whose system font is added behind everything else, or `None`
/// for a language that needs no CJK font.
pub fn install_fonts(ctx: &egui::Context, cjk: Option<Lang>) {
    const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Variable.ttf");
    const GEIST_MONO: &[u8] = include_bytes!("../../../assets/fonts/GeistMono-Variable.ttf");
    let mut fonts = FontDefinitions::default();
    // egui's own fonts stay behind ours, for symbols and emoji ours don't have.
    let fallback: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let faces: [Face<'_>; 7] = [
        ("Inter", INTER, &[(*b"wght", 400.0), (*b"opsz", 14.0)], FontFamily::Proportional),
        ("Inter-Medium", INTER, &[(*b"wght", 500.0), (*b"opsz", 14.0)], medium()),
        ("Inter-SemiBold", INTER, &[(*b"wght", 600.0), (*b"opsz", 14.0)], semibold()),
        ("Inter-Display", INTER, &[(*b"wght", 700.0), (*b"opsz", 32.0)], display()),
        ("GeistMono", GEIST_MONO, &[(*b"wght", 400.0)], FontFamily::Monospace),
        ("Inter-Label", INTER, &[(*b"wght", 600.0), (*b"opsz", 14.0)], mono()),
        ("Inter-Button", INTER, &[(*b"wght", 700.0), (*b"opsz", 14.0)], mono_bold()),
    ];
    for (name, bytes, coords, family) in faces {
        // No subpixel binning: these fonts carry no hinting instructions, so a glyph drawn at a
        // fractional x smears each stem across two pixels, leaving small text blurry at 100 %.
        let tweak = FontTweak { coords: VariationCoords::new(coords.iter().copied()), subpixel_binning: Some(false), ..Default::default() };
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes).tweak(tweak)));
        let mut stack = vec![name.to_owned()];
        stack.extend(fallback.iter().cloned());
        fonts.families.insert(family, stack);
    }
    // The CJK face, behind everything else, from the system: only when asked for, as the font is
    // large.
    if let Some(lang) = cjk
        && let Some(font) = cjk_font(lang)
    {
        fonts.font_data.insert("CJK".to_owned(), font);
        for stack in fonts.families.values_mut() {
            stack.push("CJK".to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

/// The system's font for a CJK language, read once per language (tens of MB). Desktop only: the
/// web build has none, so CJK text can't be drawn there.
pub fn cjk_font(lang: Lang) -> Option<Arc<FontData>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        CJK_FONTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).entry(lang.code()).or_insert_with(|| load_cjk(lang)).clone()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = lang;
        None
    }
}

/// Loads the system font for the CJK language `lang`: the face for its script, or `None` where the
/// system has none. Nothing falls back to another script's face — a face for a different script
/// leaves its characters as tofu — so a language without its font stays in English, like any other.
#[cfg(not(target_arch = "wasm32"))]
fn load_cjk(lang: Lang) -> Option<Arc<FontData>> {
    crate::cjk_font::load(lang.script()?).map(Arc::new)
}

/// The system fonts looked for so far, by language code.
#[cfg(not(target_arch = "wasm32"))]
static CJK_FONTS: std::sync::Mutex<std::collections::BTreeMap<&'static str, Option<Arc<FontData>>>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// Has the system font for `lang` already been loaded? Never loads it.
pub fn cjk_font_loaded(lang: Lang) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        CJK_FONTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(lang.code()).is_some_and(Option::is_some)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = lang;
        false
    }
}

/// Can the CJK language `lang` be drawn? Loads the system font on the first call ([`cjk_font`]).
pub fn cjk_font_available(lang: Lang) -> bool {
    cjk_font(lang).is_some()
}
