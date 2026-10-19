# Arthavan Projects app icon

The app icon is a **simple original placeholder**: the "AP" monogram in the steel-blue accent
(`#8fb8d8`) on a dark rounded square (`#07090d`), from
[`assets/brand/arthavan-projects-mark.svg`](../brand/arthavan-projects-mark.svg). Every file in
this directory is rendered from it (and from `arthavan-projects-mark-template.svg`, the monogram
alone in black) by `packaging/icons.sh`. Like the code, it is MIT OR Apache-2.0.

## Files

- `arthavan-projects.svg`: a copy of `assets/brand/arthavan-projects-mark.svg`; also the hicolor scalable icon.
- `arthavan-projects-1024.png`: 1024 px render, with extra transparent margin for Apple's icon grid.
- `arthavan-projects.icns`: macOS bundle icon (`CFBundleIconFile`).
- `arthavan-projects.ico`: Windows icon, 16–256 px, embedded in the `.exe` by
  `apps/artcraft-launcher/build.rs` and used by the MSI.
- `hicolor/<size>/apps/app.freedev.ainz.ks.ArthavanProjects.png` (16–512) and
  `hicolor/scalable/apps/app.freedev.ainz.ks.ArthavanProjects.svg`: Linux icon theme.
- `tray/template-44.png`: the macOS menu-bar template (22 pt @2x; macOS tints it), and
  `tray/mark-64.png`: the tray icon on Windows, Linux and FreeBSD.

## Regenerate

Run `packaging/icons.sh` on any OS. It needs `resvg`; the `.ico` and `.icns` are packed by
`cargo xtask ico` / `cargo xtask icns`. Never edit these files by hand: change the SVGs in
`assets/brand/` and run the script.
