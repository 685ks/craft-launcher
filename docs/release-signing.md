# Signing releases and the catalog

Arthavan Projects can check two things with an Ed25519 signature in the
[minisign](https://jedisct1.github.io/minisign/) format before it trusts them:

- a release's `SHA256SUMS.txt`, signed as `SHA256SUMS.txt.minisig` by the release workflow. When a
  release carries the signature, every download is checked against the signed list (and must agree
  with GitHub's own checksum); a signature that doesn't verify stops the install. A release without
  one (v0.5.1 and earlier, or a release built without the secret) still installs, against its
  published checksums, and the update dialog says it is **unsigned**;
- the remote catalog, `catalog/catalog.json`, signed as `catalog/catalog.json.minisig`. Without a
  verified copy the launcher keeps using the catalog built into `crates/core/src/catalog.rs`.

Both are checked against one public key compiled into the launcher:
`RELEASE_SIGNING_KEY` in `crates/core/src/catalog.rs`. While it is `None`, nothing is checked and
every release shows as unsigned.

## 1. Make a key pair (once)

Install minisign (`brew install minisign`, `apt install minisign`, `winget install minisign`,
or a release from GitHub), then, on a computer you trust:

```sh
minisign -G -W -p minisign.pub -s minisign.key
```

`-W` leaves the secret key without a password, so the release workflow can use it without a
prompt; the key is protected by being a GitHub secret instead. Keep an offline backup of
`minisign.key`: losing it means shipping a launcher with a new public key, and launchers that
only know the old key will then see your releases as having a bad signature.

## 2. Put the public key in the launcher

`minisign.pub` has two lines; the second (it starts with `RW`) is the key. Set it in
`crates/core/src/catalog.rs`:

```rust
pub const RELEASE_SIGNING_KEY: Option<&str> = Some("RWQ…");
```

Commit that, and release it. From that launcher version on, signatures are checked.

## 3. Give the release workflow the secret key

In the repository's **Settings → Secrets and variables → Actions**, add a secret named
`RELEASE_SIGNING_KEY` holding the whole of `minisign.key` (both lines). The `release` job of
`.github/workflows/release.yml` then signs `SHA256SUMS.txt` with the trusted comment
`Arthavan Projects <tag>` and uploads `SHA256SUMS.txt.minisig` with the other files. The launcher
requires the trusted comment to name the release's tag, so a signed list can't be moved onto
another release.

Without the secret the step is skipped and the release is published unsigned, as before.

To check a release by hand:

```sh
minisign -V -p minisign.pub -m SHA256SUMS.txt
```

## 4. Sign the catalog

`catalog/catalog.json` may only point known apps at another repository under the owner's account
or mark them retired; see `crates/core/src/remote_catalog.rs` for the format. After changing it,
raise `issued_at` (Unix seconds: an older catalog never replaces a newer one) and sign it:

```sh
minisign -S -s minisign.key -m catalog/catalog.json -t "Arthavan Projects catalog $(date +%s)"
```

The trusted comment must contain `catalog`. Commit both files to `main`; launchers fetch them from
`raw.githubusercontent.com`, keep the last copy that verified, and fall back to the built-in
catalog when there is none.

## Rotating the key

Ship a launcher with the new public key first, wait until most installs have updated, then
replace the `RELEASE_SIGNING_KEY` secret and re-sign the catalog. Releases made in between are
signed with the old key; launchers that know only the new key will refuse them, so keep the
window short or publish them unsigned.
