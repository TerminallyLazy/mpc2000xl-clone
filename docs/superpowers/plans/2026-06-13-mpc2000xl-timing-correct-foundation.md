# MPC2000XL TIMING CORRECT Foundation

## Behavior Slice

This slice adds a deterministic TIMING CORRECT control surface and recording-time quantization foundation. It does not claim exact MPC2000XL, JJ-OS, firmware, or hardware timing-correct parity.

Implemented behavior:

- `Mode::TimingCorrect` and `PanelControl::TimingCorrect` open a `TIMING` LCD screen.
- TIMING settings are sequence project metadata: `division` and `swing_percent`.
- Defaults are division `off`, swing `50`, and selected field `division`.
- Divisions are `off`, `eighth`, `eighth_triplet`, `sixteenth`, `sixteenth_triplet`, and `thirty_second`.
- Internal deterministic 96 PPQN grids are: eighth `48`, eighth triplet `32`, sixteenth `24`, sixteenth triplet `16`, and thirty-second `12` ticks.
- Cursor left/right toggles the selected TIMING field between `division` and `swing`.
- Data wheel on division cycles one enum step by delta sign; zero delta is ignored.
- Data wheel on swing clamps to `50..=75`; zero delta and boundaries are ignored with deterministic reasons.
- `TimingCorrectChanged { settings, selected_field }` plus `LcdChanged` is emitted for field/value changes.
- When playing and recording, pad strikes and mapped MIDI note-ons record quantized ticks when division is not `off`.
- Quantization chooses the nearest grid target, clamps to sequence length, and rounds exact halfway cases upward.
- Swing offsets odd non-triplet targets later inside their two-step pair. Triplet divisions ignore swing in this foundation.
- `TimingCorrectApplied` is emitted before `SequenceEventRecorded` when the recorded tick changes.
- Recording on a muted selected track still records quantized metadata, matching existing mute behavior.
- Desktop exposes a `TIMING` mode button plus compact TC field/value controls.

## Snapshot Policy

- Project snapshots persist `sequence.timing_correct`.
- Project snapshots persist the current TIMING edit field as machine UI state.
- Missing timing-correct fields restore documented defaults for older snapshots.
- Present incomplete `sequence.timing_correct` objects are rejected during JSON prevalidation.
- Structured restore and JSON restore both validate `swing_percent` is `50..=75`.

## Source And Evidence Status

- Status: fixture-backed internal spec.
- Rights boundary: no manuals, service scans, firmware bytes, JJ-OS assets, hardware captures, copied third-party code, audio bytes, factory samples, or proprietary timing-correct assets are stored in the repo.
- Reference gap: exact MPC2000XL TIMING CORRECT screens, swing law, quantize rounding, real-time recording behavior, firmware traces, and JJ-OS timing-correct behavior remain unmapped.
- This foundation exists to make timing-correct state deterministic, visible, serializable, and testable before external behavior evidence is mapped.

## Verification Targets

- `cargo fmt --all --check`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-tc-core-target cargo test -p mpc_core --test core_flow timing -- --test-threads=1`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-tc-conformance-target cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-tc-desktop-target cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `git diff --check`
