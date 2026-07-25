# Hocket Repository Guide

## Product

Hocket is a cross-platform loop recorder with asynchronous turn-taking. The
core musical gesture is adding a layer to a short loop, then handing a session
to another person when sharing is available. It is not an Ableton-shaped DAW
and it is not a real-time network jam tool.

The default looper-pedal profile starts with four tracks whose unmuted layers
sum. The named Deeler profile starts with ten tracks and selects one active
layer per track. Counts and capture settings are stored in the session so they
remain configurable.

`design_docs/PROJECT_DESCRIPTION.md` is maintainer-owned product authority.
Read `design_docs/DOC_README.md` before planning or changing subsystem scope.

## Workspace

```
crates/
  hocket-model/     Session and history authority; framework independent
  hocket-engine/    Firewheel capture, playback, click, and media abstraction
  hocket-headless/  Scripted audio-engine harness
  hocket-genet/    Genet/winit application host and recorder UI
```

Run the desktop application with `cargo run -p hocket-genet`. The retired
Masonry application and `hocket-widgets` crate are not part of this workspace.

The sibling `../woodshed/crates/audio-primitives` path dependency provides
shared pure DSP helpers. Do not couple Hocket to a Woodshed application crate.

## Boundaries

- Keep `hocket-model` independent of UI and audio frameworks. Session edits
  that must survive undo or synchronization belong in `Edit` and `History`.
- Keep `hocket-engine` as a runtime projection. It can own real-time graph and
  device concerns, but not authoritative session state.
- Keep `hocket-genet` thin. Host-local presentation state is acceptable;
  session, media, and collaboration semantics are not.
- Build local durability before peer synchronization: a peer cannot reliably
  import or share a session that the originating host cannot reopen.
- Do not add plugin hosting or an arrange view while the loop recorder, export,
  and hand-off flows are incomplete.

## Workspace Tooling: sem & weave

Two non-authoritative structural tools from Ataraxy Labs are wired into this
repo. Both read code structure via tree-sitter, not program semantics; they
never replace `cargo check` / `cargo test` / compiling.

**weave** (entity-level git merge driver). `.gitattributes` maps ~46 file
types to `merge=weave`; ordinary `git merge` resolves false conflicts where
independent edits touch different functions, structs, or keys in the same
file. A true same-entity conflict still produces markers, tagged with the
entity name and reason (e.g. `function 'foo': both modified`). Preview a
merge before running it with `weave-cli preview <branch>`.

The merge-driver binary path is machine-local, not committed (git can't
version a local binary path). It is wired via `git config --global
merge.weave.driver` on this machine, which covers every repo including
fresh clones, so no per-repo setup is needed here. On a new machine, install
with `cargo install --git https://github.com/Ataraxy-Labs/weave weave-cli
weave-driver`, then either repeat the global `git config --global
merge.weave.*` setup or run `weave setup` in each repo.

**sem** (semantic version control): entity-level diff, context, impact, and
blame queries on top of Git. Installed via `cargo install --git
https://github.com/Ataraxy-Labs/sem sem-cli` and registered as a
user-scoped Claude Code MCP server (`sem_diff`, `sem_context`, `sem_impact`,
`sem_entities`, `sem_blame`, `sem_log`; call these directly as tools). CLI
fallback if the MCP tools are not available:

```bash
sem diff --format plain
sem context <Symbol> --budget 2000 --json
sem impact <Symbol> --file <path> --json
```

Use `sem context` and `sem impact` to brief yourself on a symbol before
editing it, especially across the sibling-repo lattice (Hocket's audio
primitives path-dep into `../woodshed`). Avoid unfiltered scans over large
directories: `sem entities crates --json` on a big tree dumps a lot.

## Documentation

Follow `design_docs/DOC_POLICY.md`. Non-trivial work gets a dated plan in
`design_docs/`, whose progress reflects the live code and verification state.
Do not edit `PROJECT_DESCRIPTION.md` without explicit maintainer direction.
