# MPC2000XL Main Screen Editing Foundation

## Behavior Slice

This slice moves the foundation app beyond placeholder mode switching by adding deterministic MAIN-screen editing behavior that the desktop shell and conformance fixtures can exercise. It is a behavior slice toward the full MPC2000XL app, not an MVP and not a claim of complete reference accuracy.

Implemented behavior:

- MAIN has a typed selected-field model: sequence, track, tempo, and bars.
- Cursor left/right moves focus through those fields and emits `LcdChanged`.
- Data wheel edits the selected MAIN field.
- Tempo remains clamped to 30.00..300.00 BPM.
- Track uses the internal-spec range 1..=64 until stronger mapped evidence is available.
- Sequence uses deterministic indexes/names in the range 1..=99, formatted as `SequenceNN`.
- Bars use a deterministic count in the range 1..=999.
- MAIN soft keys 2 and 3 implement the existing `Track+` and `Track-` labels.
- Unimplemented soft keys return structured `Ignored` reasons.
- LCD text visibly reflects focus, sequence, track, tempo, play/stop state, and bar count without copying proprietary screen art.

## Scope Boundary

This slice intentionally does not implement full sequence editing, program assignment, event recording, timing correction, song mode, sample workflows, storage, audio, MIDI, or exact MPC2000XL screen art. It creates deterministic state transitions and fixture coverage that later reference-backed behavior can replace or refine.

## Files

Owned files touched by this slice:

- `crates/mpc_core/src/state.rs`
- `crates/mpc_core/src/lcd.rs`
- `crates/mpc_core/src/lib.rs`
- `crates/mpc_core/tests/core_flow.rs`
- `crates/mpc_conformance/src/lib.rs`
- `crates/mpc_conformance/tests/fixtures.rs`
- `crates/mpc_conformance/tests/fixtures/main_screen_cursor_tempo.json`
- `crates/mpc_conformance/tests/fixtures/main_screen_soft_key_track.json`
- `apps/desktop/src/main.rs`
- `docs/evidence/behavior-matrix.json`
- `docs/superpowers/plans/2026-06-13-mpc2000xl-main-screen-editing.md`

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `./scripts/verify.sh`
- `git status --short`

The conformance test runner now executes all JSON fixtures in `crates/mpc_conformance/tests/fixtures`.

## Source And Evidence Status

The behavior IDs added in `docs/evidence/behavior-matrix.json` are internal-spec backed and marked with owner-manual investigation notes. No manual pages, proprietary screenshots, firmware content, or copied screen art are included in the repository. Exact manual page mapping remains future work before claiming reference-level behavior accuracy.
