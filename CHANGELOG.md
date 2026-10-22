# Changelog

All notable changes to Arthavan Projects. Versions follow [Semantic Versioning](https://semver.org).

## 0.6.0 (2026-10-22)

The launcher now installs and keeps up to date every Craft App from Ks's forks under
[`685ks`](https://github.com/685ks), the supported source.

### Smart updating

- Updates are looked for on start, every 6 hours (Settings → Updates: every hour, every 6 hours,
  every day, or only when asked), and again as soon as the network is back after it was lost
  (retried with backoff while offline). GitHub's answers are revalidated with ETags, and a
  403/429 rate limit holds every check off until GitHub says it may ask again. Each app keeps
  its own release channel.
- **Update automatically** (on by default, and per app in its ⋯ menu): the update downloads in
  the background, is verified exactly like a manual install (signed checksums when the release
  has them, the published SHA-256 otherwise), then goes in place while the app isn't running.
  A running app gets **Restart to update**: the update waits until it quits, or is applied first
  when the app is opened from the launcher. Updates of system packages (deb, rpm, Flatpak)
  download in the background and wait for you, since they need the system's password prompt.
- **Update all** updates every app with an update (running ones once they quit). The tray shows
  how many updates are waiting, and a desktop notification says when updates are ready.
- Arthavan Projects updates itself the same way: downloaded and verified in the background,
  put in place on the next start or with **Restart to update**.
- Downloads resume where they stopped (HTTP range requests), and at most two run at once.
- Crash-loop protection: a freshly updated app that fails to start within 20 seconds twice in
  a row is rolled back to the version kept from before the update, with a notice; that version
  isn't installed automatically again.

### Apps

- Apps whose fork has no published release yet show as **Coming soon** with no install button.
  FigCraft has no public upstream to fork and is marked coming soon.
- Bundle ids are now `app.freedev.ainz.ks.<App>` (for example `app.freedev.ainz.ks.DeckCraft`)
  for macOS bundles, Flatpaks, menu entries, icons and the releases' own packages. Copies
  installed from upstream builds (`ai.storyteller.<app>`, or Windows Installer entries published
  by Learning Machines LLC) are still recognised and shown as "From upstream · update to switch";
  they are never switched automatically.
- Windows Installer entries published by "Arthavan Projects" (the forks' MSIs) are detected.

### Under the hood

- `craft-tokens` is pinned to the craft-libs `v0.1.0` tag.

Before this release can be cut, each app's fork needs a published release built with the
forks' bundle ids.
