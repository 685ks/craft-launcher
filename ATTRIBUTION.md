# Attribution

Every non-code asset in this repository, with its author, source and license. Arthavan
Projects' own code and original assets are MIT OR Apache-2.0 (see [`LICENSE-MIT`](LICENSE-MIT),
[`LICENSE-APACHE`](LICENSE-APACHE) and [`NOTICE`](NOTICE)). When you add an asset, add a row here
in the same change; third-party assets must be permissively licensed and keep their license file
next to them.

## Bundled in the app

| Path | Title | Author | Source | License |
|---|---|---|---|---|
| `assets/brand/arthavan-projects-mark.svg`, `arthavan-projects-mark-template.svg` | Arthavan Projects placeholder mark: the "AP" monogram in the steel-blue accent on a dark rounded square (and the monogram alone, for the macOS menu bar) | Arthavan Projects contributors | Original work | MIT OR Apache-2.0, like the code |
| `assets/app-icon/` (all files) | Arthavan Projects app and tray icons | Arthavan Projects contributors | Rendered from `assets/brand/` by `packaging/icons.sh`, see [`assets/app-icon/README.md`](assets/app-icon/README.md) | MIT OR Apache-2.0, like the code |
| `assets/social/github.svg`, `assets/social/globe.svg` | GitHub logo (linking to the owner's GitHub) and a globe (linking to the project's home page) | [Simple Icons](https://simpleicons.org/) contributors; [Lucide](https://lucide.dev) contributors | Simple Icons `github.svg` and Lucide `globe.svg` (lucide-static 0.468.0), white for tinting; see [`assets/social/README.md`](assets/social/README.md) | CC0 1.0 (GitHub logo; a trademark of GitHub, Inc.); ISC (globe) |
| (not bundled) the pictures on each app's details page | Screenshots of the Craft Apps, fetched when a page is opened and kept in the launcher's cache folder | The apps' contributors | [storytold/craft-launcher-assets](https://github.com/storytold/craft-launcher-assets), pinned to a tag (`SCREENSHOTS_REV` in `crates/core/src/details.rs`); each picture's source and license are listed in that repository's `ATTRIBUTION.md` | Each app's own license for the interface (MIT OR Apache-2.0); the work shown in them is public domain or made in the app, as that file says |
| `assets/fonts/Inter-Variable.ttf` | Inter 4.001, variable (body text) | The Inter Project Authors (Rasmus Andersson) | Google Fonts, <https://github.com/google/fonts/tree/main/ofl/inter> (`Inter[opsz,wght].ttf`, renamed) | SIL OFL 1.1, [`assets/fonts/OFL-Inter.txt`](assets/fonts/OFL-Inter.txt) |
| `assets/fonts/GeistMono-Variable.ttf` | Geist Mono 1.701, variable (labels and buttons) | The Geist Project Authors (Vercel) | Google Fonts, <https://github.com/google/fonts/tree/main/ofl/geistmono> (`GeistMono[wght].ttf`, renamed) | SIL OFL 1.1, [`assets/fonts/OFL-GeistMono.txt`](assets/fonts/OFL-GeistMono.txt) |

### Craft App Suite icons

The launcher shows each Craft app with that app's own icon, a 128 px render of the master SVG in
each app's repository (`assets/app-icon/`). The twelve app icons are original Arthavan Projects
house-style artwork, licensed like those apps. FigCraft's icon is still the upstream one, copied
unmodified. Refresh them from the source path, never redraw them here.

| Path | Title | Author | Source | License |
|---|---|---|---|---|
| `assets/app-icons/photocraft.png` | PhotoCraft app icon, 128 px | PhotoCraft contributors | [`685ks/photocraft`](https://github.com/685ks/photocraft) `assets/app-icon/photocraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (PhotoCraft's [`LICENSE-MIT`](https://github.com/685ks/photocraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/photocraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/lightcraft.png` | LightCraft app icon, 128 px | LightCraft contributors | [`685ks/lightcraft`](https://github.com/685ks/lightcraft) `assets/app-icon/lightcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (LightCraft's [`LICENSE-MIT`](https://github.com/685ks/lightcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/lightcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/filmcraft.png` | FilmCraft app icon, 128 px | FilmCraft contributors | [`685ks/filmcraft`](https://github.com/685ks/filmcraft) `assets/app-icon/filmcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (FilmCraft's [`LICENSE-MIT`](https://github.com/685ks/filmcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/filmcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/designcraft.png` | DesignCraft app icon, 128 px | DesignCraft contributors | [`685ks/designcraft`](https://github.com/685ks/designcraft) `assets/app-icon/designcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (DesignCraft's [`LICENSE-MIT`](https://github.com/685ks/designcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/designcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/vectorcraft.png` | VectorCraft app icon, 128 px | VectorCraft contributors | [`685ks/vectorcraft`](https://github.com/685ks/vectorcraft) `assets/app-icon/vectorcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (VectorCraft's [`LICENSE-MIT`](https://github.com/685ks/vectorcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/vectorcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/pdfcraft.png` | PdfCraft app icon, 128 px | PdfCraft contributors | [`685ks/pdfcraft`](https://github.com/685ks/pdfcraft) `assets/app-icon/pdfcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (PdfCraft's [`LICENSE-MIT`](https://github.com/685ks/pdfcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/pdfcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/effectcraft.png` | EffectCraft app icon, 128 px | EffectCraft contributors | [`685ks/effectcraft`](https://github.com/685ks/effectcraft) `assets/app-icon/effectcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (EffectCraft's [`LICENSE-MIT`](https://github.com/685ks/effectcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/effectcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/wordcraft.png` | WordCraft app icon, 128 px | WordCraft contributors | [`685ks/wordcraft`](https://github.com/685ks/wordcraft) `assets/app-icon/wordcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (WordCraft's [`LICENSE-MIT`](https://github.com/685ks/wordcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/wordcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/deckcraft.png` | DeckCraft app icon, 128 px | DeckCraft contributors | [`685ks/deckcraft`](https://github.com/685ks/deckcraft) `assets/app-icon/deckcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (DeckCraft's [`LICENSE-MIT`](https://github.com/685ks/deckcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/deckcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/gridcraft.png` | GridCraft app icon, 128 px | GridCraft contributors | [`685ks/gridcraft`](https://github.com/685ks/gridcraft) `assets/app-icon/gridcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (GridCraft's [`LICENSE-MIT`](https://github.com/685ks/gridcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/gridcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/cadcraft.png` | CADCraft app icon, 128 px | CADCraft contributors | [`685ks/cadcraft`](https://github.com/685ks/cadcraft) `assets/app-icon/cadcraft.svg`, rendered at 128 px | MIT OR Apache-2.0 (CADCraft's [`LICENSE-MIT`](https://github.com/685ks/cadcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/cadcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/soundcraft.png` | SoundCraft app icon, 128 px | SoundCraft contributors | [`685ks/soundcraft`](https://github.com/685ks/soundcraft) `assets/app-icon/icon.svg`, rendered at 128 px | MIT OR Apache-2.0 (SoundCraft's [`LICENSE-MIT`](https://github.com/685ks/soundcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/685ks/soundcraft/blob/main/LICENSE-APACHE)) |
| `assets/app-icons/figcraft.png` | FigCraft app icon (chameleon), 128 px | ArtCraft Team and the FigCraft contributors | [`storytold/figcraft`](https://github.com/storytold/figcraft) `assets/app-icon/hicolor/128x128/apps/ai.storyteller.figcraft.png`, copied unmodified | MIT OR Apache-2.0 (FigCraft's [`LICENSE-MIT`](https://github.com/storytold/figcraft/blob/main/LICENSE-MIT), [`LICENSE-APACHE`](https://github.com/storytold/figcraft/blob/main/LICENSE-APACHE)) |

## Documentation and README

| Path | Title | Author | Source | License |
|---|---|---|---|---|
| README app icons (linked, not copied) | Each Craft app's 64 px icon | ArtCraft Team and each app's contributors | `raw.githubusercontent.com/storytold/<app>/main/assets/app-icon/hicolor/64x64/apps/ai.storyteller.<app>.png` | MIT OR Apache-2.0 (each app's license) |
