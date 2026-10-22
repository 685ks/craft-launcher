# AGENTS.md: guide for AI agents and contributors

Arthavan Projects installs, updates and launches the **Craft App Suite** (PhotoCraft, LightCraft,
FilmCraft, DesignCraft, VectorCraft, PdfCraft, EffectCraft, WordCraft, DeckCraft, GridCraft,
CADCraft, SoundCraft, FigCraft). It is written in **Rust only** (no JavaScript or TypeScript): the desktop
app is native egui/eframe, and the web build is the same Rust compiled to WebAssembly (trunk +
wasm-bindgen). **No Tauri, Electron or webview shells**, ever.

**Windows only.** The launcher is built, released and supported for Windows, and installs only
Windows packages (the MSI and the portable ZIP). There are no macOS, Linux or FreeBSD builds or
install paths: elsewhere `install::available_kinds()` is empty, `Platform::is_supported()` is
false and every install, update or self-update returns a "not supported on this platform" error.
The workspace still compiles and its tests run on Linux (CI's ubuntu job, and the wasm check), so
keep non-Windows code compiling, but never add a non-Windows install path back.

The product name is always **Arthavan Projects** in user-facing text: UI, window titles, About,
tray menu, notifications, installers, release names, docs prose. The suite is the **Craft App
Suite**. Machine names stay lowercase: the binaries (`arthavan-projects`,
`arthavan-projects-web`), release assets and file names; the reverse-DNS app id is
`app.freedev.ainz.ks.ArthavanProjects` (`catalog::LAUNCHER_BUNDLE_ID`). The Rust crates keep their
internal names (`artcraft-launcher`, `artcraft-launcher-*`); the shipped binary names come from
`[[bin]] name` in `apps/*/Cargo.toml`. The GitHub repo is `685ks/craft-launcher`, a fork of
`storytold/craft-launcher` ("ArtCraft Launcher"); it is not affiliated with or endorsed by
ArtCraft, and nothing user-facing may carry the ArtCraft name or logos.

Standards and learnings shared across the Craft apps live in `../craftrules` (read its
`README.md` and `AGENTS.md`). Contribute reusable learnings there, never code.

## 1. Workspace map

```text
crates/
  core      artcraft-launcher-core      pure and wasm-safe: the app catalog, versions, GitHub
                                        release parsing, picking the right asset per platform
  native    artcraft-launcher-native    desktop only: HTTP, detecting installed apps, download,
                                        verify and install
  ui-egui   artcraft-launcher-ui-egui   the shared egui UI (desktop and web)
apps/
  artcraft-launcher                     desktop shell (eframe): window, tray icon, notifications;
                                        builds the `arthavan-projects` binary
  artcraft-launcher-web                 the same UI in the browser (trunk; catalog and downloads only)
xtask/                                  cargo xtask ci | wasm | version | ico
packaging/                              Windows and web packaging scripts
assets/brand/                           the "AP" placeholder mark (master SVGs, original, MIT/Apache)
assets/app-icon/                        the launcher's own icon renders and tray icons
assets/app-icons/                       the Craft apps' icons, copied from each app's repo
assets/fonts/                           Inter, Geist Mono (OFL-1.1)
```

Layering: `core` depends on no other workspace crate and must build for `wasm32-unknown-unknown`.
`native` may use `core`. `ui-egui` may use `core` (and `native` only behind
`cfg(not(target_arch = "wasm32"))` or through a trait the desktop shell implements). Nothing below
the apps opens windows, tray icons or notifications.

## 2. Golden rules

### Never crash (outranks feature work)

A bad network response, a malformed release, a half-downloaded file, a full disk or a corrupt
settings file must produce an error the user can act on, never a panic. Shared standard:
`../craftrules/standards/never-crash.md`.

- **Non-test code never panics.** No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!` or
  `unimplemented!`. Return the crate's error type and propagate with `?`.
- Every production crate root (`lib.rs`, each binary's `main.rs`, and `xtask`) carries
  `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`.
- **No `unsafe`.** The workspace sets `unsafe_code = "forbid"`.
- **Everything from the network is hostile.** Use `get()` instead of indexing, checked arithmetic
  for sizes and offsets, and cap downloads and JSON sizes.

### Installing other people's software safely

- Download only from the catalog's repos: `https://github.com/<owner>/<app>/releases` (and
  GitHub's asset CDN), where `<owner>` is the one account named in the `github_owner!` macro in
  `crates/core/src/catalog.rs` (today `685ks`, Ks's forks of the `storytold/<app>` apps). Those
  forks are the supported source; the release-asset checks refuse a link to any other repo. The
  one exception is a developer's own loopback mirror (`ARTHAVAN_PROJECTS_DOWNLOAD_BASE`,
  `cargo xtask local`); keep it loopback-only and behind the `local-mirror` cargo feature, which
  release builds never enable.
- Verify each download against the release's `SHA256SUMS.txt` before running or installing it.
- The details drawer's pictures are the one other download: JPEGs only, from a pinned tag of
  `https://raw.githubusercontent.com/storytold/craft-launcher-assets/` (`core::details`),
  capped at 2 MB each, cached, and never run. Pictures are not bundled in this repo.
- Never run an installer with elevated rights silently; tell the user what is about to happen.
- Never delete a user's files outside what the launcher itself installed.
- **Agents never land or approve a third-party app.** A change that adds a catalog row, download
  source or install path for an app whose repo is not under the catalog's owner
  (`github_owner!`, today `685ks`), that points the owner somewhere else, or that loosens the
  rules above to allow one, is a maintainer's call alone. An agent reviewing it flags it at the
  top of its review (who owns the repo, its age and history, whether the builds are signed) and
  stops there: no merge, no approval, no fixes pushed to the PR.
- An app whose fork has no published release (or no repo yet) shows as "Coming soon" with no
  install button, never as an error. An app with no public upstream to fork is marked
  `coming_soon` in the catalog and is never looked up.

### Bundle ids

Every Craft App's reverse-DNS id is `app.freedev.ainz.ks.<DisplayName>`
(`CraftApp::bundle_id`, e.g. `app.freedev.ainz.ks.DeckCraft`): the id the apps' packages and
icons are named by.
Copies installed by upstream builds carry `ai.storyteller.<id>` (`CraftApp::legacy_bundle_ids`);
detection still recognises them, marks them `Installed::upstream` ("installed from upstream,
update to switch") and never switches them automatically.

### Look and feel

The UI follows the house design system (`craft-tokens` from craft-libs): a near-black navy page,
a steel-blue accent, hairline white borders, rounded corners (`theme::R_*`: 10 for icon buttons,
12 for buttons and inputs, 14 for cards and dialogs, pills for badges and bars), soft shadows
under cards, Inter for everything (700 for headings and app names, upper-case 600/700 labels and
buttons with wide tracking), and Geist Mono only for code. Gold (`Tokens::gold`) is reserved for
"you / now / current". Colours come from `theme::Tokens`, whose dark set is built from the tokens. Build new UI from `widgets.rs`; never hard-code a colour.

Animations follow the OS: with Windows' "Animation effects" off or a browser's
`prefers-reduced-motion`, nothing fades, wipes or glides
(`Host::reduce_motion`, `crates/ui-egui/src/motion.rs`). Time a new animation with egui's
`style.animation_time`, or switch it off on `LauncherUi::reduce_motion`. Progress bars and
spinners keep moving.

### Languages

Every user-facing string goes through `crates/ui-egui/src/i18n` (`tl!("…")` for literals, `fmt`
for `{placeholders}`, `tn` for counts). English in the code is the lookup key; adding a string
means adding its row to every catalog marked `complete_menus` (the `*.tsv` files in
`crates/ui-egui/src/i18n/`), which the i18n tests check. Translate clean-room from the meaning of
the English text, never through another catalog. Product and app names (Arthavan Projects, Craft
App Suite, PhotoCraft…) stay in English.
A CJK language is drawn with the system's own CJK font (`crates/ui-egui/src/cjk_font.rs`, desktop
only; English where there is none): never bundle a CJK font, they are 10 MB and more.

### Other rules

1. **Assets need attribution.** Every bundled asset gets a row in `ATTRIBUTION.md` (path, title,
   author, source, license) in the same change. Never add ArtCraft's name, logos or marks:
   they are someone else's trademarks.
2. **Icons are generated.** The app icon, the header mark and the tray icon come from the "AP"
   placeholder in `assets/brand/`; never edit the renders by hand. Run `packaging/icons.sh` (needs `resvg`; runs on any OS) and commit the outputs.
   `assets/app-icons/` is copied verbatim from each app's repo (`assets/app-icon/hicolor/128x128/apps/<bundle id>.png`,
   `ai.storyteller.<app>.png` in repos not yet renamed);
   refresh it from there, never redraw.
3. **Never break wasm.** `cargo xtask wasm` must pass. File-system, process and OS code is
   `cfg(not(target_arch = "wasm32"))`.
4. **Tests are the gate.** Release parsing and asset picking in `core` get unit tests with real
   GitHub release JSON fixtures (small, committed under `crates/core/tests/fixtures/`).
5. When fetching anything, use a generic `ArthavanProjects/<version>` User-Agent and never put
   a person's name, email or other personal details in requests.

## 3. Before you finish a task

```sh
cargo xtask ci              # fmt --check, clippy -D warnings, test, wasm check
```

Or piecemeal: `cargo test -p <crate>`, `cargo clippy -p <crate> --all-targets -- -D warnings`,
`cargo xtask wasm`. If you touched packaging: `shellcheck packaging/*.sh packaging/*/*.sh` and
`actionlint`.

## 4. Releasing

The version lives only in `[workspace.package] version` in the root `Cargo.toml`
(`cargo xtask version`, `cargo xtask version set X.Y.Z[-pre]`). Pushing a tag `v<version>` (it must
match `Cargo.toml`) runs `.github/workflows/release.yml`, which builds Windows x64 with the scripts in
`packaging/` and publishes the GitHub Release with `SHA256SUMS.txt`.
Besides the versioned files below, each build is also attached under a stable name
(`<APP_SLUG>-windows-x64.zip`, `-windows-x64.msi`) for `releases/latest/download/…` links; the product name and slug are set
once, in the workflow's `env`. `site/index.html` (deployed by `.github/workflows/pages.yml`) is the
download page that links to them. The canonical recipe is
`../craftrules/release/playbook.md`; cutting a release is `../craftrules/release/cut-a-release.md`.

Release assets (keep these names exact; the launcher's own self-update and the README rely on them):

| Platform | Files |
|---|---|
| Windows | `arthavan-projects-<ver>-windows-{x64,x86,arm64}.msi`, `…-windows-{x64,x86,arm64}-portable.zip` |
| Web | `arthavan-projects-web-<ver>.zip` |

Packaging facts the code relies on:

- The Windows MSI is **per-user** (no UAC prompt), by default into
  `%LOCALAPPDATA%\Programs\Arthavan Projects`. Double-clicked it opens a WixUI setup wizard
  (Welcome; a folder page with a "Create a desktop shortcut" box; Ready; progress; Finish with
  "Launch"), whose art `package.ps1` draws at build time; self-update runs it with `/passive`, no
  wizard. The folder and the shortcut choice are recorded under
  `HKCU\Software\Arthavan Projects` (`InstallDir`,
  `DesktopShortcutChoice`) and reused by the next version, and `self_update.rs` reads `InstallDir`
  to know the running copy came from the MSI. Its shortcuts set AppUserModelID
  `app.freedev.ainz.ks.ArthavanProjects`, which toast notifications need. `portable.txt` beside the exe switches on portable mode
  (`ArthavanProjectsData\` next to the exe).
- `build.rs` reads `ARTHAVAN_PROJECTS_BUILD_SHA` / `ARTHAVAN_PROJECTS_BUILD_DATE` and, on Windows,
  embeds `assets/app-icon/arthavan-projects.ico`, failing the build when
  `ARTHAVAN_PROJECTS_REQUIRE_WINRES=1` and it can't.

## 5. Parallel agents

- Use your own target dir (`CARGO_TARGET_DIR=target/agent-<name>`) and edit only the files you own.
- Keep every `Cargo.toml` valid at all times: the `crates/*` and `apps/*` globs mean one broken
  manifest breaks everyone's build. Write manifests to a temp file and `mv` them into place.
- If someone else's in-progress edit breaks the build, wait and retry; don't fix their files.
