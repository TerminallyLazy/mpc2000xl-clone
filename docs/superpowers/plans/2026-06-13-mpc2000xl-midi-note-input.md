# MPC2000XL MIDI Note Input Foundation

## Behavior Slice

This slice adds deterministic MIDI note input handling to the foundation app. It models incoming note-on and note-off events in `mpc_core` without adding host MIDI device I/O, native MIDI dependencies, external MIDI files, proprietary assets, firmware, manual scans, screenshots, or samples.

Implemented behavior:

- `HardwareEvent::MidiNoteOn` accepts channel, note, and velocity fields.
- `HardwareEvent::MidiNoteOff` accepts channel, note, and velocity fields.
- MIDI channels are valid only in `1..=16`.
- MIDI note numbers are valid only in `0..=127`.
- MIDI note-on velocity is valid only in `1..=127`; velocity `0` is ignored deterministically in this slice instead of being treated as note-off.
- MIDI note-off velocity is valid in `0..=127`. This original slice treated note-off as a playback and recording no-op; the later 2026-06-14 release foundation supersedes that with a typed `SampleReleaseIntent` path.
- Note-on events for notes `36..=51` map to bank A pads `1..=16` with `pad = note - 35`.
- Mapped note-on events emit `MidiNoteMapped` and then reuse the existing pad strike path, including `PadTriggered`, playback intent or miss, optional sequence recording, LCD refresh, last playback, and desktop host-audio routing.
- Notes outside `36..=51`, invalid fields, and unmapped note-offs emit `MidiInputIgnored` with a deterministic reason.
- Recorded `SequenceEvent` remains pad-based for this slice; raw MIDI channel and note metadata are not persisted yet.
- The desktop shell includes modest MIDI simulation controls for channel, note, velocity, note-on, note-off, and quick note buttons.

## Deterministic Assumptions

- Default MIDI note mapping is intentionally simple: note `36` maps to `A01`, note `51` maps to `A16`, and no other banks or programs are mapped.
- Note-on velocity `0` is treated as invalid input and ignored so the contract remains explicit and testable.
- Note-off does not trigger sample playback, change `last_playback`, or append recorded sequence events.
- Validation and ignored-output reasons are stable test contract strings until reference evidence requires a different model.
- Sequence persistence remains pad-centric to avoid expanding the project snapshot schema before full MIDI event requirements are known.

## Source And Evidence Status

This slice is backed by repo-owned internal specifications, core tests, and one conformance fixture. Exact MPC2000XL owner-manual page mapping, firmware traces, hardware MIDI input captures, timing behavior, multi-mode MIDI settings, and JJ-OS differences are still pending.

No proprietary manuals, firmware contents, hardware captures, scans, screenshots, MIDI files, audio media, or samples are stored in the repository. Behavior-matrix coverage for MIDI note input mapping is marked internal-spec/manual-investigation with firmware and hardware evidence gaps.

## Scope Boundary

This slice intentionally does not implement host MIDI device discovery, realtime MIDI ports, MIDI clock, MIDI thru, MIDI output, channel/program assignment modes, note repeat, note-off voice choking, aftertouch, pitch bend, controller messages, multi-bank mappings, JJ-OS behavior, timing correction, or exact reference screen copy.

The goal is to establish a deterministic typed event contract that later manual, firmware, hardware, or VMPC comparative evidence can refine without changing the core dispatch architecture.

## Files

Owned files touched by this slice:

- `crates/mpc_core/src/events.rs`
- `crates/mpc_core/src/state.rs`
- `crates/mpc_core/tests/core_flow.rs`
- `crates/mpc_conformance/tests/fixtures/midi_note_on_a01_synthetic_audio_render.json`
- `apps/desktop/src/main.rs`
- `docs/evidence/behavior-matrix.json`
- `docs/evidence/source-map.md`
- `docs/superpowers/plans/2026-06-13-mpc2000xl-midi-note-input.md`

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test -p mpc_core midi -- --nocapture`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `git diff --check`

Focused checks added:

- MIDI note 36 maps to bank A pad 1 and emits the same playback intent as physical pad A01.
- MIDI note 51 maps to bank A pad 16.
- Out-of-range mapped notes are ignored without changing last playback or recorded events.
- MIDI note-off has no playback or sequence recording side effect; the later release foundation emits `MidiNoteReleased` and `SampleReleaseIntent` for mapped assigned pads.
- MIDI note-on while overdubbing records a pad-based sequence event with mapped sample metadata.
- Invalid MIDI channel, note, and velocity inputs emit deterministic ignored outputs.
- A conformance fixture verifies MIDI note-on 36 produces A01 synthetic playback metadata and render summary.

## Next Boundaries

Next MIDI slices should map source evidence before claiming MPC2000XL-accurate MIDI behavior. Likely next boundaries are channel/mode settings, host MIDI I/O abstraction, note-off envelope/choke semantics, MIDI clock transport, input quantization, raw MIDI event persistence, and evidence-backed screen behavior.
