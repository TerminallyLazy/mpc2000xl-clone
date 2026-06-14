# MPC2000XL Sample Trim Window Foundation

## Behavior Slice

This slice adds deterministic TRIM editing for generated sample playback windows. It is a metadata-only foundation and does not import, decode, store, destructively crop, or waveform-edit real audio.

Implemented behavior:

- `SampleTrim` stores a trim window keyed by sample id: `sample_id`, `start_frame`, and `end_frame`.
- Project snapshots persist sample trims in program metadata and the selected TRIM edit field in machine UI metadata.
- Missing `program.sample_trims` and `machine.selected_trim_edit_field` fields restore deterministic defaults for older current-reader snapshots.
- Generated samples default to `start_frame = 0`, `end_frame = length_frames - 1`, and `window_length_frames = length_frames`.
- Sample trim validation rejects empty sample ids, duplicate sample ids, unknown sample ids, out-of-range windows, and inverted windows.
- Snapshot validation rejects unknown JSON fields at the nested trim boundary.
- TRIM cursor left/right selects Start or End. SAMPLE mode cursor behavior is unchanged.
- SAMPLE data wheel still navigates the generated catalog. TRIM data wheel edits the selected sample window.
- TRIM Start edits clamp to `0..=end_frame`; End edits clamp to `start_frame..=length_frames - 1`.
- F1/F2 continue to navigate previous/next sample in SAMPLE and TRIM.
- `SampleTrimChanged` emits start, end, window length, sample id, and selected field after real trim edits.
- `SamplePlaybackIntent` and synthetic render summaries carry `start_frame`, `end_frame`, and `window_length_frames`.
- Recorded sequence events snapshot playback trim metadata. Later TRIM edits do not mutate recorded event playback; restored sequence playback uses the recorded window.
- Desktop SAMPLE/TRIM status shows the selected trim window and TRIM selected field.

## Deterministic Assumptions

- Generated sample length remains derived from the source pad address for testability.
- Trim metadata is keyed by sample id; duplicate sample ids collapse in the catalog before trim resolution.
- The renderer remains rights-safe synthesis. It reports trim metadata and uses `start_frame` as a deterministic generated source-frame offset, but it does not slice real audio bytes.
- Persisted trim entries are project metadata, not asset references. No file paths or audio bytes are added.

## Source And Evidence Status

- Status: fixture-backed internal spec.
- Rights boundary: metadata only; no WAV data, factory samples, firmware bytes, manuals, service scans, JJ-OS assets, or third-party media are stored in the repo.
- Reference gaps: no real audio import, no waveform editing, no destructive crop, no MPC file-format claims, and no JJ-OS parity.
- Exact MPC2000XL TRIM screen behavior, waveform UI, native file format semantics, and destructive/non-destructive trim behavior remain unmapped pending accepted manual, firmware trace, hardware capture, or approved comparative-source evidence.

## Verification Targets

- `cargo fmt --all --check`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-sample-trim-focused-test-target cargo test -p mpc_core --test core_flow trim -- --test-threads=1`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-sample-trim-conformance-test-target cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-sample-trim-workspace-test-target cargo test --workspace -- --test-threads=1`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-sample-trim-desktop-check-target cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `python3 -m json.tool docs/evidence/behavior-matrix.json >/dev/null`
- `git diff --check`
