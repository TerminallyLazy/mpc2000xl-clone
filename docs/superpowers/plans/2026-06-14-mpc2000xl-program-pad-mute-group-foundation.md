# MPC2000XL Program Pad Mute Group Foundation

## Behavior Slice

This slice adds deterministic PROGRAM pad mute-group metadata and host-audio choke accounting. It is an internal foundation for future evidence-backed mute/choke behavior, not a claim of exact MPC2000XL or JJ-OS mute-group semantics.

Implemented behavior:

- `PadAssignment` stores `mute_group`, defaulting to `0` for off.
- `SamplePlaybackIntent` and `AudioRenderSummary` carry `mute_group`.
- PROGRAM mode edit fields now cycle Pad, Level, Pan, Tune, and Mute Group.
- Data wheel edits assigned-pad mute group in the internal range `0..=16`.
- Unassigned mute-group edits return a structured ignored output and leave the pad unassigned.
- Generated, recorded, and imported metadata assignment replacement preserves the previous selected pad mute group.
- Project snapshots default missing mute groups to `0`, serialize nonzero groups, and reject values outside `0..=16`.
- Recorded sequence events snapshot mute group in playback metadata so later assignment edits do not rewrite recorded playback.
- Host audio chokes already-active sample-playback voices in the same nonzero mute group before allocating the new voice.
- Host audio does not choke group `0`, count-in click voices, disabled playback, failed renders, invalid backend receipts, zero-frame receipts, or non-queued receipts.
- Desktop status shows selected assignment mute group, playback/render mute group metadata, and host-audio choked voice counts.

## Deterministic Contract

- Mute group `0` means off and never chokes.
- Choke matching is host-audio metadata only: active sample-playback voices with the same nonzero `mute_group` are removed before the new voice is allocated.
- Choke accounting is separate from release, completion, and oldest-voice stealing.
- Choke runs only after render validation, backend success, receipt validation, queued receipt, and positive frame count.
- If choking reduces active voices below the voice limit, oldest-voice stealing does not run for that allocation.

## Source And Evidence Status

- Status: unit-backed and fixture-backed internal spec.
- Rights boundary: no manuals, service scans, firmware bytes, JJ-OS assets, hardware captures, MIDI files, copied third-party code, audio bytes, factory samples, or native device traces are stored in the repo.
- Evidence gap: exact MPC2000XL PROGRAM mute-group field names, group count, interaction with envelopes, note-off, pad mute mode, MIDI output, voice stealing, audio tails, and JJ-OS behavior remain unmapped.

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_core --test core_flow program_parameter -- --test-threads=1`
- `cargo test -p mpc_audio mute_group -- --test-threads=1`
- `cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- behavior matrix JSON validation and duplicate-ID guard
- `git diff --check`

Focused checks added:

- PROGRAM cursor cycling includes Mute Group.
- PROGRAM data wheel clamps mute group to `0..=16`.
- Pad strike playback intent carries mute group.
- Recorded/replayed sequence metadata preserves mute group.
- Project snapshots round-trip nonzero mute group and reject invalid group `17`.
- Conformance fixture verifies nonzero mute group in playback and render metadata.
- Host audio chokes prior sample voices in the same nonzero mute group.
- Host audio leaves group `0` and count-in click voices active.

## Next Boundaries

Next mute/choke slices should map accepted reference evidence before claiming sampler parity. Practical follow-ups include exact PROGRAM screen copy and ranges, pad mute mode, all-notes-off, envelope release tails, MIDI output note-off policy, per-output routing, hardware/firmware trace comparison, and JJ-OS-specific behavior only after a real target is verified.
