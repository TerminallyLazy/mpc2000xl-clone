# MPC2000XL MIDI Settings Mode Foundation

## Behavior Slice

This slice turns MIDI mode from a placeholder into a deterministic settings screen for the existing simulated MIDI note input path. It does not add host MIDI device discovery, realtime MIDI ports, MIDI files, MIDI clock, MIDI sync, MIDI thru, MIDI output, proprietary firmware behavior, JJ-OS parity, or hardware I/O.

Implemented behavior:

- MIDI settings are machine-level state: `midi_input_channel`, `midi_base_note`, and `selected_midi_settings_field`.
- Input channel defaults to Omni (`None`), preserving previous all-channel note-on behavior.
- Base note defaults to `36`, preserving previous note `36..=51` to `A01..=A16` behavior.
- Base note is clamped to `0..=112`, so `base_note..=base_note+15` always stays inside valid MIDI note numbers.
- MIDI mode LCD shows input channel, base note, mapped note range, selected settings field, and `Host MIDI I/O: off`.
- In MIDI mode, cursor left/right toggles between input channel and base note.
- In MIDI mode, the data wheel edits the selected setting:
  - input channel clamps through Omni, Ch 01, ..., Ch 16;
  - base note clamps through `0..=112`.
- Settings edits emit `MidiSettingsChanged { input_channel, base_note, selected_field }` plus `LcdChanged`.
- `MidiNoteOn` validates channel/note/velocity, then applies the optional input-channel filter, then maps notes in `midi_base_note..=midi_base_note+15` to bank A pads `1..=16`.
- If an input channel is selected, valid note-on events on other channels emit `MidiInputIgnored` and do not trigger playback, recording, last-playback mutation, or host-audio routing.
- `MidiNoteOff` remains a validated no-op for playback and recording. The input-channel filter does not change note-off behavior in this foundation slice.
- Project snapshots persist MIDI settings with serde defaults so older snapshots restore to Omni, base note `36`, and `input_channel` selection.
- Snapshot compatibility policy for this slice is current-reader backward compatibility: missing MIDI settings fields restore to documented defaults. Older-binary forward compatibility for new v1 fields is deferred to the formal snapshot migration/versioning slice.
- The desktop shell shows MIDI settings next to MIDI simulation and provides MIDI-mode setting/value buttons that route through cursor/data-wheel events.

## Source And Evidence Status

- Status: fixture-backed internal spec.
- Rights boundary: no manuals, service scans, firmware bytes, JJ-OS assets, hardware captures, MIDI files, copied third-party code, audio bytes, or factory samples are stored in the repo.
- Reference gap: exact MPC2000XL/JJ-OS MIDI settings screens, channel policy, note mapping policy, MIDI clock/sync behavior, and host MIDI I/O behavior remain unmapped.
- The channel/base-note policy exists to make the current simulated note input configurable and testable, not to claim final sampler parity.

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_core midi_settings -- --nocapture`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `git diff --check`
