# MPC2000XL MIDI Device I/O Foundation

## Behavior Slice

This slice adds optional native host MIDI device input/output behind the existing host MIDI abstraction. It is a full-app foundation for connecting desktop MIDI ports, not a claim of exact MPC2000XL MIDI hardware timing, sync, thru, electrical behavior, or JJ-OS parity.

Implemented behavior:

- `mpc_midi` depends on `midir` for cross-platform host MIDI port discovery and connection.
- Host MIDI state now distinguishes disabled, capture, and device output modes.
- `CaptureMidiBackend` remains the deterministic default backend for CI and conformance.
- `DeviceMidiOutputBackend` implements `HostMidiBackend` and sends existing `MidiOutputIntent` note-on messages to a selected host MIDI output port.
- Output encoding is the standard three-byte note-on shape: status `0x90 | (channel - 1)`, note, velocity.
- `DeviceMidiInputConnection` opens a selected host MIDI input port and decodes standard three-byte note-on and note-off messages into a bounded queue.
- Note-on messages with velocity zero are decoded as note-off events.
- Unsupported, empty, short, or otherwise unhandled MIDI messages are ignored with counters and bounded recent reasons instead of entering core state.
- The desktop shell can refresh MIDI ports, select input and output ports, connect/disconnect input, switch output between capture and device, and poll decoded input events into the existing `HardwareEvent::MidiNoteOn` / `HardwareEvent::MidiNoteOff` path.
- While native MIDI input is connected, the desktop requests periodic repaint so queued input drains even when the window is otherwise idle.
- Desktop status shows host MIDI mode/backend, output port/sent count, input queue depth, received/decoded/ignored/dropped counts, and last output event.

## Deterministic Contract

- Capture backend remains the deterministic test and fixture path and opens no OS MIDI ports.
- Native MIDI ports are opened only after explicit desktop selection.
- Device input queue capacity is bounded by `DEFAULT_DEVICE_MIDI_INPUT_QUEUE_EVENTS` and `MAX_DEVICE_MIDI_INPUT_QUEUE_EVENTS`.
- Queue overflow drops decoded input events and records counters/reasons without growing memory.
- Output send returns a queued receipt only after the host MIDI backend accepts the byte message.
- Device input is polled by the desktop frame loop and routed through existing core MIDI mapping, release, host-audio, and host-MIDI output handling.
- Desktop polling remains active during a native input connection by scheduling repaint every 10 ms; this is a liveness policy, not a claimed MPC2000XL timing tolerance.
- This slice intentionally supports note-on/note-off only.

## Source And Evidence Status

- Status: unit-backed internal spec plus desktop compile coverage.
- `midir` version: `0.11.0`, used only for host MIDI device discovery and I/O plumbing.
- Rights boundary: no proprietary manuals, service scans, firmware bytes, JJ-OS assets, hardware captures, MIDI files, factory samples, copied third-party code, audio files, or native MIDI traces are stored in the repo.
- Evidence gap: exact MPC2000XL MIDI timing, filtering, port naming, sync, clock, thru, running status, SysEx, CC, program change, pitch bend, note-off output, hotplug behavior, electrical behavior, and JJ-OS behavior remain unmapped.

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_midi -- --test-threads=1`
- `cargo test -p mpc_core --test core_flow midi -- --test-threads=1`
- `cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- behavior matrix JSON validation and duplicate-ID guard
- `git diff --check`

Focused checks added:

- Note-on output messages encode to deterministic MIDI bytes.
- Invalid output channels fail before bytes are sent.
- Input decoder accepts note-on and note-off, including note-on velocity zero as note-off.
- Input decoder ignores unsupported messages and rejects short note messages.
- Device input queue clamps capacity, drains decoded events, counts ignored messages, and drops overflow without unbounded growth.

## Next Boundaries

Future MIDI slices should add explicit device hotplug handling, timing diagnostics, MIDI note-off output policy, all-notes-off, clock/sync/thru, running status, CC/program-change/pitch-bend/SysEx parsing, SMF import/export, MIDI monitor tooling, and manual/hardware evidence mapping before claiming reference accuracy. Exact MPC2000XL MIDI behavior and JJ-OS-specific behavior remain deferred until accepted evidence exists.
