# Signed Hand-Off Envelope Plan

## Goal

Define the complete, authentic payload that one Strophe peer hands to another,
without tying it to a premature network carrier or pretending divergent edits
have already been reconciled.

## Design

- `strophe-engine::handoff` builds a versioned envelope containing a project
  bundle plus every media blob referenced by its phrases. It refuses incomplete
  snapshots and verifies each media hash on receipt.
- The sender derives a session-scoped signing key through `personae`; the
  envelope binds both that public key and the intended recipient's public key.
- The signed bytes cover the format, session id, sender, recipient, manifest,
  and media. A recipient must match and the signature must verify before a
  `ReceivedHandoff` is materialized.
- The encoded bytes are carrier-neutral. A Murm attachment, Iroh transfer, or
  file exchange can move them without reinterpreting session or media state.
- Receipt produces a staged snapshot. `History` can retain and integrate a
  same-root branch, but incoming conflict reconciliation is not claimed here.
- `ReceivedHandoff::accept_branch` transactionally integrates the incoming
  graph, checks out its head, verifies its manifest projection, and imports
  missing referenced media. It preserves the previous local branch rather than
  fabricating a merged head.

## Done Conditions

- A complete project and all referenced media serialize into a signed envelope.
- The intended recipient verifies and reconstructs the same bundle/media.
- Wrong-recipient, altered, malformed-signature, and missing-media envelopes
  fail before a host can accept them.
- The protocol does not add identity, device, or transport state to a project.
- Same-root acceptance either updates bundle/media together or leaves both
  untouched.

## Progress

- 2026-07-10: **PARTIAL.** The engine protocol and focused tests are landed.
  A later host slice must provide a durable local persona, a carrier, incoming
  staging/review, and a user-facing acceptance action. Core same-root branch
  acceptance is landed; branch merge still waits for a conflict-reconciliation
  policy.
