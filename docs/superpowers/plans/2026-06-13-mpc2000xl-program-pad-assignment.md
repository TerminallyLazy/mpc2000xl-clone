# MPC2000XL Program Pad Assignment Foundation

## Behavior Slice

This slice adds deterministic program and pad assignment behavior to the foundation app. It is a behavior slice toward the full MPC2000XL application, not an MVP and not a claim of complete reference accuracy.

Implemented behavior:

- `mpc_core` has typed, rights-safe program/sample metadata.
- The current program tracks index, name, and pad assignments.
- The default program is `Program01` and assigns bank A pads 1 through 16 to generated synthetic sample slots.
- Synthetic assignments store sample id, sample name, level, and pan foundation values only.
- No sample audio bytes, proprietary samples, firmware contents, screenshots, or copied reference media are stored.
- Valid `StrikePad` events still emit `PadTriggered`.
- Assigned pad strikes also emit `SamplePlaybackIntent` with bank, pad, sample id/name, velocity, selected track, current program, level, and pan.
- Unassigned pad strikes emit `SamplePlaybackMiss` with a structured `PadUnassigned` reason while preserving `PadTriggered`.
- `MpcState` stores the selected Program-mode pad and the last playback resolution.
- PROGRAM LCD text shows current program and selected pad assignment using repo-owned text.
- In PROGRAM mode, F1 clears the selected pad assignment and F2 restores the deterministic generated assignment.
- PROGRAM mode pad strikes select the struck pad and resolve its assignment deterministically.
- Cursor left/right and data wheel step the selected PROGRAM pad within the current bank as a small foundation affordance.
- Desktop shows current program, selected pad assignment, and last playback resolution.

## Recording Model

Recorded `SequenceEvent` now has an optional `playback` field.

The field stores a `SamplePlaybackIntent` snapshot when the pad had an assignment at record time. This is the simpler strongly typed model for the foundation because the sequence event remains the durable recorded object and carries the assignment metadata needed by future playback scheduling.

When a recorded pad is unassigned, the sequence event is still recorded with its pad, velocity, track, and tick fields, but `playback` is `None`. The runtime still emits `SamplePlaybackMiss` for the strike so the miss reason is observable without inventing an audio event.

Existing fixture JSON remains backward-compatible because missing `playback` fields default to `None`.

## Deterministic Assumptions

- Default program index is `1`.
- Default program name is `Program01`.
- Default assignments are generated only for bank A pads 1 through 16.
- Generated sample ids use `synthetic_<bank>_<pad>`, for example `synthetic_a_01`.
- Generated sample names use `SYN-<bank><pad>`, for example `SYN-A01`.
- Generated assignment level is `100`.
- Generated assignment pan is center, stored as `0`.
- Clearing an assignment removes only the selected pad assignment.
- Reassigning replaces any selected pad assignment with the deterministic generated assignment.
- Assignment storage is metadata only and intentionally does not model audio buffers, envelopes, filters, voices, polyphony, mute groups, sample start/end, or DSP.

## Source And Evidence Status

This slice is backed by repo-owned internal specifications, core tests, and conformance fixtures. Exact owner-manual page mapping, firmware traces, VMPC comparative behavior, and hardware captures for program assignment behavior are still pending.

Behavior-matrix entries for the new program/pad assignment behaviors are marked as internal-spec/manual-investigation pending exact manual evidence. No proprietary copied content is included.

## Scope Boundary

This slice intentionally does not implement full MPC2000XL program editing, sample loading, sample playback, waveform storage, voice allocation, envelopes, filters, tuning, mute groups, velocity layers, stereo sample behavior, pad-bank switching UI, persistence, disk formats, or reference-accurate PROGRAM screen copy.

The goal is to establish deterministic assignment metadata and observable playback intent so later slices can attach an audio engine and refine behavior against mapped source evidence without changing the event-driven contract.

## Files

Owned files touched by this slice:

- `crates/mpc_core/src/events.rs`
- `crates/mpc_core/src/state.rs`
- `crates/mpc_core/src/lcd.rs`
- `crates/mpc_core/src/lib.rs`
- `crates/mpc_core/tests/core_flow.rs`
- `crates/mpc_conformance/src/lib.rs`
- `crates/mpc_conformance/tests/fixtures.rs`
- `crates/mpc_conformance/tests/fixtures/program_pad_main_strike_assignment.json`
- `crates/mpc_conformance/tests/fixtures/program_pad_clear_reassign.json`
- `crates/mpc_conformance/tests/fixtures/sequence_recording_assigned_pad_metadata.json`
- `apps/desktop/src/main.rs`
- `docs/evidence/behavior-matrix.json`
- `docs/superpowers/plans/2026-06-13-mpc2000xl-program-pad-assignment.md`

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `./scripts/verify.sh`
- `git status --short`

Focused checks added or extended:

- Default program assigns bank A pads to generated synthetic samples.
- Assigned pad strikes emit `SamplePlaybackIntent`.
- Clearing a selected pad assignment causes a later strike to emit `SamplePlaybackMiss`.
- Reassigning restores the generated synthetic assignment.
- PROGRAM pad strikes update the selected pad and LCD.
- Recording an assigned pad captures sample metadata on the recorded sequence event.
- Conformance fixtures cover main-mode assignment resolution, PROGRAM clear/reassign, and recorded assignment metadata.

## Next Boundaries

Next behavior slices should map source evidence before claiming reference accuracy for PROGRAM edit flow or screen text. Practical next boundaries are bank switching, richer program assignment fields, sample-slot persistence, sequence playback scheduling from recorded intents, and a rights-safe synthetic audio engine that renders generated tones rather than bundled proprietary samples.
