# MPC2000XL Track Mute Playback Foundation

## Behavior Slice

This slice adds deterministic track mute state to MAIN and sequence playback. It is an internal foundation for muting selected recorded tracks during playback, not complete MPC2000XL Track Mute mode or JJ-OS parity.

Implemented behavior:

- `MpcState::muted_tracks` stores muted track numbers as a sorted, unique `Vec<u8>`.
- Project sequence snapshots persist `muted_tracks`, with a serde default so older snapshots restore as no muted tracks.
- Snapshot validation rejects muted track numbers outside `1..=64` and duplicate muted tracks; unsorted unique snapshots restore into sorted state.
- MAIN soft key 4 toggles mute for the current `selected_track` and emits `MachineOutput::TrackMuteChanged` followed by `LcdChanged`.
- Non-MAIN modes keep their existing soft-key behavior and do not interpret soft key 4 as track mute.
- Recorded sequence playback skips events whose `selected_track` is muted.
- Skipped muted-track events do not emit `SequenceEventPlayed`, do not emit `SamplePlaybackIntent`, and do not update `last_playback`.
- Recording and MAIN F5 erase remain metadata operations and still work while the selected track is muted.
- Loop playback uses the same mute filter across the loop boundary.
- MAIN LCD and desktop status expose the selected track mute state and total muted-track count.

## Deterministic Assumptions

- Track mute only gates sequence playback scheduling in this slice. Direct pad strikes, MIDI note-on pad triggering, recording metadata capture, and erase behavior are intentionally unchanged.
- Mute state is sequence-level metadata in `ProjectSequenceSnapshot`.
- Track mute is not Track Mute mode, solo, JJ-OS next-sequence behavior, pad mute, MIDI output mute, voice allocation, or audio-engine stop/kill behavior.
- The scheduler remains append-order deterministic for unmuted recorded events.

## Source And Evidence Status

This slice is backed by repo-owned tests, JSON conformance fixtures, and internal behavior documentation. Manual page mapping, firmware traces, hardware captures, VMPC comparative evidence, and JJ-OS-specific behavior are still investigation items.

No proprietary assets, binary media, firmware bytes, screenshots, manual scans, copied sequence-screen text, JJ-OS artifacts, or samples are included.

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test -p mpc_core --test core_flow track_mute -- --test-threads=1`
- `cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-track-mute-workspace-test-target cargo test --workspace -- --test-threads=1`
- `CARGO_TARGET_DIR=/tmp/mpc2000xl-track-mute-desktop-check-target cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `python3 -m json.tool docs/evidence/behavior-matrix.json >/dev/null`
- `git diff --check`

## Next Boundaries

Future slices should map exact reference behavior before claiming accuracy for dedicated Track Mute screens, solo interaction, mute groups, mute automation, JJ-OS extensions, MIDI output muting, voice stealing, audio tail cutoff, next-sequence behavior, and bar/beat/tick display parity.
