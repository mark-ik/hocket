# Releasing Hocket

The pipeline is Rust end to end: [`cargo-packager`](https://crates.io/crates/cargo-packager)
builds the installer, [`luggage`](https://github.com/mark-ik/mere/tree/main/crates/system/luggage)
(mere) checks the feed and applies the update. No .NET, on any host.

Verified on Windows 2026-07-24 (see the auto-update plan's H4 receipts).

## Once per machine

```sh
cargo install cargo-packager --locked
cargo install --path <mere>/crates/system/luggage --bin luggage-manifest
```

## Once, ever: the release key

```sh
cargo packager signer generate --path <somewhere outside every repo>/hocket.key
```

Keep the private key offline and out of every repo. The `.pub` beside it is
the `HOCKET_UPDATE_PUBKEY` value shipped to clients; it is already in the
base64 form luggage wants.

## Per release, per host

Hosts pack their own format: `nsis` on Windows, `app` on macOS, `appimage`
on Linux. Each host adds its entry to the same `luggage.json`.

```sh
# 1. Bump the version in crates/hocket-genet/Cargo.toml.

# 2. Pack + sign. The manifest's before-packaging-command rebuilds first —
#    see the traps below for why that is not optional.
export CARGO_PACKAGER_SIGN_PRIVATE_KEY=<path to hocket.key>
export CARGO_PACKAGER_SIGN_PRIVATE_KEY_PASSWORD=
cargo packager -p hocket-genet --release --formats nsis   # or app / appimage

# 3. Stage the artifact + its .sig in the feed, then add this host's entry.
luggage-manifest --artifact <feed>/hocket-genet_<ver>_x64-setup.exe \
                 --version <ver> --format nsis
```

`luggage-manifest` computes the BLAKE3 digest, reads the `.sig`, and merges
into an existing `luggage.json` when the version matches — so the Mac and
Linux entries can be added later without redoing the Windows one.

### Per-host artifact shapes

What luggage downloads is not always what `cargo packager` emits:

| Host | Pack format | Artifact the manifest points at |
| --- | --- | --- |
| Windows | `nsis` | the `…-setup.exe` as packed |
| Linux | `appimage` | the `.AppImage` itself (luggage writes the downloaded bytes straight over the running AppImage) |
| macOS | `app` | a **gzipped tar of the `.app`**, which must be made and signed separately |

macOS needs the extra step because `cargo packager --formats app` produces
only the bundle, while luggage's macOS install path gunzips and untars (and
a tarball is what preserves the bundle's structure and permissions):

```sh
cargo packager -p hocket-genet --release --formats app
cd target/release
tar czf Hocket.app.tar.gz Hocket.app
cargo packager signer sign --private-key-path <hocket.key> Hocket.app.tar.gz
luggage-manifest --artifact Hocket.app.tar.gz --version <ver> --format app
```

Sign the **tarball**, not the bundle: the signature must cover the bytes
that are actually downloaded.

For a directory feed the default `file://` URL is right. For a published
feed pass `--url` with the URL the artifact will actually live at (for a
GitHub release, `https://github.com/<owner>/<repo>/releases/download/v<ver>/<file>`),
and upload `luggage.json` as a release asset so `github:owner/repo` finds it.

## Running the update

```sh
export HOCKET_UPDATE_FEED=<dir | https://… | github:owner/repo>
export HOCKET_UPDATE_PUBKEY=$(cat hocket.key.pub)
export HOCKET_UPDATE_POLICY=automatic     # off | notify | download-then-ask | automatic
hocket-genet --update-now                 # or just launch the app
```

`--update-now` runs the flow in the terminal and prints each real state; the
app does the same in the background and shows it in the top bar. Both honour
the policy, so `notify` reports an available version without fetching it.

## Traps, both found the hard way on 2026-07-24

1. **`cargo packager` does not build your app.** `--release` only tells it
   where to look for a binary. Without a `before-packaging-command` it will
   package whatever is already in `target/release`.
2. **`cargo build` does not rebuild when only the version changed.** A
   version bump with no source change leaves the old binary in place, so the
   installer is *named* for the new version while the binary inside still
   reports the old one through `env!("CARGO_PKG_VERSION")` — and an update
   to it looks like it silently did nothing.

Both are handled by the `before-packaging-command` in
`crates/hocket-genet/Cargo.toml`, which cleans that one crate and rebuilds
before packing. Do not "simplify" it away.

**After packing, check the binary matches the label:**

```sh
./target/release/hocket-genet --update-now | head -1   # must print the new version
```

## What is not automated yet

- OS install trust: Authenticode (Windows) and Developer ID + notarization
  (macOS). The minisign signature above is *artifact* authenticity, which is
  a different job; see mere's auto-update brief.
- Publishing the feed (upload to GitHub Releases / a host).
- The macOS and Linux legs have not been run end to end yet.
