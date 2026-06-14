# MPC2000XL MIDI Output Capture Foundation

## Behavior Slice

This slice adds deterministic MIDI output intents and a capture-only host MIDI backend. It is a foundation for output verification and desktop routing, not native OS MIDI port I/O, MIDI clock, MIDI sync, MIDI thru, full MPC2000XL MIDI mode parity, or JJ-OS parity.

Implemented behavior:

- Successful physical pad playback emits `MachineOutput::MidiOutputIntent` immediately after `SamplePlaybackIntent`.
- Successful recorded sequence playback emits the same `MidiOutputIntent` after the scheduled `SamplePlaybackIntent`.
- Incoming `MidiNoteOn` events still map through the pad path, but do not echo a `MidiOutputIntent`.
- Playback misses, ignored inputs, muted-track skipped events, and invalid MIDI input do not emit MIDI output.
- The output intent snapshots track, program, pad, source sample identity, channel, note, and velocity.
- Internal output channel policy maps selected tracks to channels `1..=16` by wrapping track numbers.
- Internal output note policy maps A01-D16 from the current MIDI base note across the 64-pad linear surface when the resulting note is within `0..=127`.
- `mpc_midi` adds `HostMidiEngine`, `CaptureMidiBackend`, typed note messages, receipts, state, ignored events, and failed validation events.
- The desktop shell routes `MidiOutputIntent` through an enabled capture backend and shows queued/ignored/failed status plus the last host MIDI event.

## Deterministic Contract

- Core MIDI output is metadata-only and serialized as a `MachineOutput`; project snapshots remain unchanged.
- The host backend is capture-only. It retains a bounded recent message list for inspection and never opens OS MIDI devices.
- Disabled host MIDI records deterministic ignored events without mutating the capture backend.
- Invalid host MIDI intents fail before backend send.
- Native device discovery, device selection, port lifecycle, MIDI clock, MIDI start/stop/continue, MIDI thru, raw MIDI event persistence, note-off output, voice choking, and reference-accurate MIDI mode behavior remain future slices.

## Source And Evidence Status

- Status: unit-backed and fixture-backed internal spec.
- Rights boundary: no manuals, service scans, firmware bytes, JJ-OS assets, hardware captures, MIDI files, copied third-party code, audio bytes, factory samples, or native device traces are stored in the repo.
- Evidence gap: exact MPC2000XL MIDI output channel/note policies, MIDI mode screen copy, firmware traces, hardware MIDI captures, VMPC comparison, and JJ-OS behavior remain unmapped.

## Files

Owned files touched by this slice:

- `crates/mpc_core/src/events.rs`
- `crates/mpc_core/src/state.rs`
- `crates/mpc_core/src/lib.rs`
- `crates/mpc_core/src/lcd.rs`
- `crates/mpc_core/tests/core_flow.rs`
- `Cargo.toml`
- `Cargo.lock`
- `crates/mpc_midi/Cargo.toml`
- `crates/mpc_midi/src/lib.rs`
- `crates/mpc_conformance/src/lib.rs`
- `crates/mpc_conformance/tests/fixtures/count_in_click_transport.json`
- `crates/mpc_conformance/tests/fixtures/midi_settings_base_note_40_round_trip.json`
- `crates/mpc_conformance/tests/fixtures/midi_output_capture_foundation.json`
- `crates/mpc_conformance/tests/fixtures/sequence_track_mute_playback.json`
- `crates/mpc_conformance/tests/fixtures/timing_correct_quantized_record_restore_playback.json`
- `apps/desktop/Cargo.toml`
- `apps/desktop/src/main.rs`
- `docs/evidence/source-map.md`
- `docs/evidence/behavior-matrix.json`
- `docs/superpowers/plans/2026-06-14-mpc2000xl-midi-output-capture-foundation.md`

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_core --test core_flow midi -- --test-threads=1`
- `cargo test -p mpc_midi -- --test-threads=1`
- `cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `git diff --check`

Focused checks added:

- Physical pad playback emits a MIDI output intent from playback metadata.
- Incoming MIDI note-on mapped playback does not echo a MIDI output intent.
- Sequence playback emits MIDI output for scheduled unmuted events.
- MIDI output intent serializes stably.
- Capture backend queues valid note messages, ignores disabled sends, rejects invalid intents, and retains a bounded recent message list.
- Conformance fixture covers B04 physical pad playback producing note 55 into deterministic capture output.

## Next Boundaries

Next MIDI slices should map reference evidence before claiming accuracy. Likely next work includes native MIDI device discovery/selection, OS port send, MIDI clock transport, note-off output, MIDI thru, raw MIDI event persistence, MIDI channel/program assignment screens, and hardware/firmware trace comparison.
