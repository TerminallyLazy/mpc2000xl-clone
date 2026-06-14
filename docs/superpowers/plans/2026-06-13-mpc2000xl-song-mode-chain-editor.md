# MPC2000XL SONG Mode Chain Editor Foundation

## Behavior Slice

This slice turns SONG mode from a placeholder into a deterministic metadata editor for a song chain over existing sequence identifiers. It is an internal foundation for future song behavior, not playback scheduling or MPC/JJ-OS parity.

Implemented behavior:

- `SongStep` stores zero-based `sequence_index` metadata clamped to `0..=98` and `repeats` clamped to `1..=99`; sequence display remains one-based as `Seq 01..99`.
- `SongEditField` stores the selected field as `step`, `sequence`, or `repeats`; it defaults to `step` and cycles with cursor left/right in SONG mode.
- Machine state owns `song_steps`, `selected_song_step_index`, and `selected_song_edit_field`.
- A song always has at least one step. The default chain is one step: stored sequence `0` displayed as `Seq 01`, repeats `1`.
- SONG LCD title is `SONG` and shows selected step index/count, selected sequence number/name, repeats, selected field marker, and Insert/Delete soft keys.
- Cursor up/down in SONG selects previous/next step when possible and emits `SongStepSelected` plus `LcdChanged`.
- Cursor up/down at the first/last step emits `Ignored` without an LCD refresh.
- Data wheel on the `step` field selects a step by delta within existing bounds.
- Data wheel on the `sequence` and `repeats` fields updates the selected step and emits `SongStepChanged` plus `LcdChanged`.
- Zero data-wheel deltas and boundary no-ops emit deterministic `Ignored` outputs.
- SONG F2 inserts a step after the selected step, copying the selected sequence and using repeats `1`; the inserted step becomes selected.
- SONG F3 deletes the selected step when more than one step exists and selects the nearest remaining step.
- Deleting the only step is ignored; non-SONG modes do not interpret SONG Insert/Delete soft keys.

## Boundaries

- This does not implement song playback, song-to-sequence conversion, native MPC song files, or JJ-OS behavior.
- This does not claim exact MPC2000XL or JJ-OS screen text, workflows, limits beyond stored sequence `0..=98` and repeats `1..=99`, or manual parity.
- This does not add manuals, scans, firmware, media, samples, images, binary fixtures, or third-party code/assets.
- Storage remains project JSON metadata only; audio and host storage logic are unchanged.

## Snapshot Policy

- `PROJECT_SNAPSHOT_VERSION` remains `1`.
- SONG metadata is persisted as a top-level `song` object with `steps`, `selected_step_index`, and `selected_field`.
- Current-reader backward compatibility is supported with serde defaults: older snapshots missing `song` restore the default one-step chain.
- Present `song` objects are validated: steps must be non-empty, stored sequence must be `0..=98`, repeats must be `1..=99`, and selected index must be in range.
- Foundation v1 remains an additive internal schema line. Older-binary forward compatibility is deferred until a formal project snapshot migration/versioning slice exists.

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_core song -- --nocapture`
- `cargo test -p mpc_conformance -- --nocapture`
- `cargo test --workspace`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-song-desktop-check-target cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `python3 -m json.tool docs/evidence/behavior-matrix.json >/dev/null`
- `git diff --check`

## Source And Evidence Status

- Status: fixture-backed internal spec.
- Source ID: `song-mode-chain-editor-slice-plan`.
- Reference gap: exact MPC2000XL SONG mode UI, song playback, conversion behavior, native file persistence, and JJ-OS differences remain unmapped.
