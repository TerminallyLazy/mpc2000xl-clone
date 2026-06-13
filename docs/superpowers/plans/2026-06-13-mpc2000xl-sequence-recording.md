# MPC2000XL Sequence Recording Foundation

## Behavior Slice

This slice adds deterministic sequence recording behavior to the foundation app. It is a behavior slice toward the full MPC2000XL app, not an MVP and not a claim of complete reference accuracy.

Implemented behavior:

- `mpc_core` has a typed `SequenceEvent` model for recorded pad strikes.
- Recorded pad events capture selected track, pad bank, pad number, velocity, and playhead tick.
- `MpcState` retains recorded sequence events in order.
- `MpcState` tracks a deterministic integer playhead in ticks.
- `Press Rec` arms recording without starting playback.
- `Press Play` starts playback and preserves an armed recording state.
- `Press Overdub` starts playback with recording enabled.
- `Press Stop` stops playback and disarms recording.
- Valid `StrikePad` events still emit `PadTriggered`.
- Valid `StrikePad` events also append and emit `SequenceEventRecorded` only when both playback and recording are active.
- `Tick { micros }` advances the playhead only while playing.
- MAIN LCD text shows playhead ticks and recorded-event count through original rights-safe app text.
- Desktop status prioritizes recorded-event output for recorded pad strikes and shows playhead/event count.

## Deterministic Assumptions

- The internal timing basis is 96 PPQN until exact MPC2000XL source mapping is available.
- Tick conversion uses integer arithmetic: `micros * tempo_bpm_x100 * 96 / (60_000_000 * 100)`.
- Fractional tick remainder is retained in state so repeated smaller ticks accumulate deterministically.
- Arithmetic uses widened integer math and saturating playhead storage to avoid overflow or wrapping with very large `micros` values.
- The current playhead does not reset on Stop in this slice; Stop only stops playback and disarms recording.
- Recording stores pad strikes at the current playhead tick before any later clock events.

## Source And Evidence Status

This slice is backed by repo-owned internal specifications and tests. Exact owner-manual page mapping, firmware timing traces, and hardware behavior captures are still pending.

No manual pages, proprietary screenshots, firmware contents, copied LCD art, or copied sequence screen text are included in the repository. Behavior-matrix entries for sequence recording and playhead ticks are marked as internal-spec/manual-investigation pending exact source mapping.

## Scope Boundary

This slice intentionally does not implement full MPC2000XL sequence editing, bar/beat/tick display, timing correction, metronome/count-in, loop boundaries, event erase, overdub merge policy beyond append-only events, MIDI output, audio scheduling, program assignment, persistence, or exact reference UI copy.

The goal is to establish deterministic state and conformance fixtures that later manual, firmware, hardware, or VMPC comparative evidence can refine without changing the basic event-driven architecture.

## Files

Owned files touched by this slice:

- `crates/mpc_core/src/events.rs`
- `crates/mpc_core/src/state.rs`
- `crates/mpc_core/src/lcd.rs`
- `crates/mpc_core/src/lib.rs`
- `crates/mpc_core/tests/core_flow.rs`
- `crates/mpc_conformance/src/lib.rs`
- `crates/mpc_conformance/tests/fixtures.rs`
- `crates/mpc_conformance/tests/fixtures/sequence_recording_rec_play.json`
- `crates/mpc_conformance/tests/fixtures/sequence_recording_overdub_tick_pad.json`
- `apps/desktop/src/main.rs`
- `docs/evidence/behavior-matrix.json`
- `docs/superpowers/plans/2026-06-13-mpc2000xl-sequence-recording.md`

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `./scripts/verify.sh`
- `git status --short`

Focused checks added or extended:

- Stopped or Rec-armed-only pad strikes do not record sequence events.
- Rec then Play records a valid pad strike.
- Overdub records a valid pad strike.
- Tick advances the playhead only while playing.
- Stop disarms recording.
- Replay determinism includes recorded sequence events.
- Conformance fixtures cover Rec+Play recording and Overdub+Tick recording.

## Next Boundaries

Next behavior slices should map exact reference evidence before claiming MPC2000XL-accurate timing or screen behavior. Likely next boundaries are bar/beat/tick position modeling, sequence length and loop behavior, event list editing, timing correction, and persistence/import-export of recorded events.
