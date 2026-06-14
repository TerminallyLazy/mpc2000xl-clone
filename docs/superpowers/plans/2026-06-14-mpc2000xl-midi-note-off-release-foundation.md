# MPC2000XL MIDI Note-Off Release Foundation

## Behavior Slice

This slice replaces the previous deterministic note-off no-op with a typed release path. It is a foundation for release, choke, and envelope work; it is not accepted evidence of exact MPC2000XL or JJ-OS note-off behavior.

Implemented behavior:

- `MidiNoteOff` validates channel, note, and velocity ranges.
- The existing MIDI input channel filter applies to note-off as well as note-on.
- The current MIDI base note maps the active 16-note window to bank A pads `1..=16`.
- Notes outside the mapped window emit `MidiInputIgnored` with a deterministic reason.
- Mapped note-offs for unassigned pads emit `MidiInputIgnored` with the target pad label.
- Mapped note-offs for assigned pads emit `MachineOutput::MidiNoteReleased` followed by `MachineOutput::SampleReleaseIntent`.
- Note-off release does not trigger `PadTriggered`, does not emit `SamplePlaybackIntent`, does not emit `MidiOutputIntent`, does not record sequence events, and does not mutate `last_playback`.
- `HostAudioEngine::release_intent` removes active sample-playback voices whose sample id, bank, and pad match the release intent.
- Count-in click voices, other pads, and other samples are not removed by sample release.
- Disabled host audio records a deterministic ignored release event without mutating active voices.
- Desktop host-audio routing consumes `SampleReleaseIntent`, shows release status text, and reports released voice counts.

## Deterministic Contract

- Release matching is metadata-only: sample id, pad bank, and pad number must all match.
- Release is immediate removal from the active host-audio voice list. There is no generated release envelope, audio tail, mute group, or choke-group model in this slice.
- Release counters are host-engine-local and increment by the number of matching active voices removed.
- The release event path stores no PCM buffers, no proprietary sample data, no firmware bytes, no hardware captures, and no MIDI files.

## Source And Evidence Status

- Status: unit-backed and fixture-backed internal spec.
- Rights boundary: no manuals, service scans, firmware bytes, JJ-OS assets, hardware captures, MIDI files, copied third-party code, audio bytes, factory samples, or native device traces are stored in the repo.
- Evidence gap: exact MPC2000XL/JJ-OS note-off semantics, envelopes, mute/choke groups, all-notes-off behavior, MIDI output note-off policy, hardware timing, and firmware traces remain unmapped.

## Files

Owned files touched by this slice:

- `crates/mpc_core/src/events.rs`
- `crates/mpc_core/src/state.rs`
- `crates/mpc_core/src/lib.rs`
- `crates/mpc_core/tests/core_flow.rs`
- `crates/mpc_audio/src/lib.rs`
- `apps/desktop/src/main.rs`
- `crates/mpc_conformance/tests/fixtures/midi_note_off_release_foundation.json`
- `docs/evidence/source-map.md`
- `docs/evidence/behavior-matrix.json`
- `docs/superpowers/plans/2026-06-14-mpc2000xl-midi-note-off-release-foundation.md`

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_core --test core_flow midi_note -- --test-threads=1`
- `cargo test -p mpc_audio release -- --test-threads=1`
- `cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- behavior matrix JSON validation and duplicate-ID guard
- `git diff --check`

Focused checks added:

- MIDI note-off 36 maps to A01 release metadata.
- Note-off release serializes stably.
- Out-of-range and unassigned note-offs are ignored deterministically.
- Note-off release does not trigger playback, recording, MIDI output echo, or `last_playback` mutation.
- Host audio release removes matching sample voices.
- Host audio release leaves count-in click and other-pad voices intact.
- Disabled host audio ignores release without voice mutation.

## Next Boundaries

Next release slices should map accepted reference evidence before claiming exact sampler behavior. Practical next work includes envelope release curves, mute/choke groups, all-notes-off, outbound MIDI note-off capture, native MIDI device I/O, hardware/firmware trace comparison, and JJ-OS-specific behavior only after a real target is verified.
