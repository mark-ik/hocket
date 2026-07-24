# Auto-Update Plan

**2026-07-24.** Give Hocket a real, configurable auto-update capability, and
use Hocket as the pressure vessel for the update layer the rest of the family
will share.

Mark's framing: auto-update is a big deal and it has to work *everywhere*.
This plan builds it so the parts that must behave identically on every host
(policy, status, decisions) are platform-neutral and testable, and only the
mechanism (install, download, swap, restart) is per-platform.

The cross-family research and the Velopack-vs-composed decision live in
[mere's auto-update brief](../../mere/design_docs/2026-07-22_auto-update_brief.md),
whose findings this plan implements.

## Requirements (from the brief, restated as acceptance)

1. **Configurable, never a checkbox.** Policy is a real setting:
   off / check-and-notify / download-then-ask / fully automatic, plus channel
   and check cadence. Not a boolean "auto-update yes/no".
2. **Honest status.** Every state the user sees is a state the system is
   actually in: idle, checking, available, downloading, ready-to-restart,
   up-to-date, failed-with-reason. Never a placebo spinner.
3. **Signed.** Verified before apply.
4. **Safe apply.** A failed update leaves a working previous version.
5. **Respect the install channel.** A build that did not come from our
   installer (a `cargo run` dev build, a distro package) must know that and
   refuse to self-update rather than corrupt someone else's install.

## Shape

Two layers, so "works everywhere" is a property of the design and not of
each app remembering to behave:

```text
UpdatePolicy + UpdateStatus + decide()      <- platform-neutral, unit-tested
                  |
            UpdateTransport (trait)
                  |
   Velopack (desktop)   [later: web SW, embassy-boot firmware, store-mediated]
```

- **Layer 1** is pure Rust: no I/O, no platform calls, no Velopack. It owns
  the policy enum, the status state machine, and `decide()` — given a policy
  and a check result, what should happen. This is the part that must be
  identical on Windows, macOS, Linux, and later on other surfaces, so it is
  the part with tests.
- **Layer 2** is a `UpdateTransport` trait. `VelopackTransport` implements it
  for desktop. A surface Velopack does not serve (a PWA service worker, an
  embassy-boot firmware offer) implements the same trait instead of
  reimplementing the policy.

Layer 1 is written with no Hocket coupling so promoting it to a shared crate
later is a file move, not a rewrite. It incubates here per the pressure-vessel
pattern rather than being speculatively extracted first.

## Phases

- **H1. The update module.** Layer 1 + the transport trait + a Velopack
  implementation, with the pure logic unit-tested. No UI yet.
- **H2. Host wiring.** `VelopackApp::build().run()` as the first statement in
  `main` (it may restart or exit the process for install/update hooks, so
  nothing may precede it), plus a background check that never blocks the audio
  thread or the event loop, reporting status back through the existing
  host-event channel.
- **H3. Status in the UI.** Surface the real state, and the actions the
  current policy permits (check now, download, restart to finish).
- **H4. Real cycles on real hosts.** A v0.1 -> v0.2 install-and-update cycle on
  Windows, then Linux over SSH. macOS needs Mark at the keyboard.
- **H5. Signing.** Resolve Authenticode-vs-ed25519 (see the brief's finding)
  and sign real releases.

## Non-goals

Delta packages (until a large-binary need is measured), staged rollouts, and
a settings *UI* beyond what H3 needs — the policy is a real setting from the
start, but its editor can come with the broader settings work.

## Findings

- 2026-07-24: Hocket did not build when this started, for two reasons worth
  recording because both were silent. The gitignored `.cargo/config.toml`
  patched `graphshell` at `mere/crates/graphshell/graphshell`, which moved to
  `mere/ports/graphshell` — and a `[patch]` path that does not exist is a hard
  error even when the patch is unused. Then `cambium`/`sprigging` were pinned
  at 0.2.0/0.1.0 while the local genet checkout had moved to 0.3.0/0.2.0, so
  those patches went *unused* and the published pair dragged a second
  `paint_list_api` into the graph. The symptom was a `PaintCmd` type mismatch
  in `leaves.rs`, which looks like a code bug and is actually a stale pin.
  Bumped to upstream-current (both versions are published).

## Progress

- 2026-07-24: plan written; hocket build unblocked (see Findings).
- 2026-07-24: **H1 + H2 done, H3 partly.** 21 update tests green (35 in the
  bin overall), clippy clean.
  - `update.rs` is layer 1: `UpdatePolicy` (four behaviours, not a boolean),
    `UpdateChannel`, `UpdateSettings`, `UpdateStatus` (a variant per real
    state, failures keeping their reason), `decide()`, and the
    `UpdateTransport` trait. No I/O, no Velopack, no Hocket coupling — so it
    moves to a shared crate as a file when the family wants it.
  - `update/velopack_transport.rs` is the desktop mechanism. It refuses to
    act on a build our installer did not place (`is_installed()` looks for
    Velopack's `Update.exe`/`UpdateMac`/`update` beside the parent of the
    exe's directory), so a `cargo run` build or a distro package reports
    `Unsupported::NotInstalled` instead of corrupting a layout it does not
    own. The feed comes from `HOCKET_UPDATE_FEED`; a directory is a file
    feed, anything else HTTP(S). It also refuses to install a version other
    than the one the user was told about, if the feed moves underneath.
  - `update/worker.rs` is an armillary actor shaped like `project_io`, so
    update I/O never touches the UI or audio threads. Tested against a fake
    transport for each policy: notify-only never downloads, automatic
    downloads *and stages without restarting* (Hocket can be recording — an
    update is offered, never forced), failures keep their reason, and an
    uninstalled build refuses every command.
  - `main` runs `VelopackApp::build().run()` as its first statement (it can
    restart or exit the process, so a window or audio device must not exist
    yet), spawns the worker, and kicks one check if policy allows. Statuses
    drain through the existing project-update path into `AppState`.
  - The top bar shows the status, deliberately silent when idle/up-to-date so
    the indicator means something when it speaks.
- Two real defects the tests caught rather than review: `can_check()` offered
  a check action on installations that provably cannot update, and the
  `update_worker` handle is load-bearing (dropping it ends the actor), which
  is now documented where it is held.

- 2026-07-24 **direction change (Mark): a Rust-native pipeline, no .NET
  anywhere.** Velopack's runtime is Rust but its packer needs a .NET
  runtime on the build machine. Ruled: fork `cargo-packager-updater` as
  **luggage**, homed in **mere** (`crates/system/luggage`); packing uses
  upstream's `cargo-packager` CLI (`cargo install`able, pure Rust).
  Luggage T1 landed same day: pluggable feeds (HTTP with upstream
  templating, local **directory** holding `luggage.json`, and
  **github:owner/repo** resolving to the latest release's manifest asset),
  plus an optional per-platform **BLAKE3** digest verified before the
  minisign signature — the content-addressing seam for the planned P2P
  distribution lane (iroh-blobs chunk dedup as implicit delta), carried in
  the manifest from day one. 19 luggage tests green in mere, including
  in-process minisign signing, digest mismatch, and tampered-bytes cases.
- 2026-07-24: **`LuggageTransport` is Hocket's default transport**
  (`update/luggage_transport.rs`), with Velopack selectable via
  `HOCKET_UPDATE_TRANSPORT=velopack` for the A/B until one retires.
  Config: `HOCKET_UPDATE_FEED` (any `Feed::parse` form) +
  `HOCKET_UPDATE_PUBKEY` (minisign; refusing to run unsigned keeps
  "signed" a fact, not an option). Dev-build detection is a
  target-dir-path heuristic, honestly documented as such. check/download/
  apply map onto luggage with staged bytes held between download and
  restart, and a version-drift refusal if the feed moves between deciding
  and fetching. 40 bin tests green.
- CROSS-REPO: hocket's committed manifest takes luggage from mere's GitHub
  main, which does not carry it until mere is pushed; the gitignored
  `.cargo/config.toml` patch supplies the sibling checkout meanwhile (the
  same posture as the personae fold).

### Next

- **H3 remainder: the actions.** The status is visible but not yet
  actionable. `AppState` needs the worker handle threaded through
  `new`/`from_project_parts` to offer "Check now", "Download", and "Restart
  to finish". Deferred deliberately rather than bolted on with a setter.
- **H4: the real cycle on hocket, luggage-first.** Pack with upstream
  `cargo-packager` (NSIS on Windows), generate `luggage.json` (version,
  per-platform url/signature/blake3; sign with `cargo-packager`'s minisign
  signing or `rsign2`), point `HOCKET_UPDATE_FEED` at the feed dir, and run
  v0.1 -> v0.2. Then Linux over SSH (AppImage), then macOS (`.app.tar.gz`;
  rustup/cargo approved for the Mac, and no .NET needed anymore).
- **T2 (luggage): staged-swap apply mechanics** replacing
  installer-over-yourself, Velopack-grade, portable Rust.
- **T3 (luggage): the P2P lane** — signed manifests + artifacts as
  content-addressed blobs over iroh-blobs (mere-transport's BlobStore
  exists); retinue carries manifest announcements, IP lanes carry bytes.
- **H5: signing** — artifact authenticity is now solved (minisign is
  built into the pipeline). Remaining is OS install trust: Developer ID +
  notarization on the Mac (credentials exist; no signing cert installed in
  its keychain yet), Authenticode-or-not on Windows.
