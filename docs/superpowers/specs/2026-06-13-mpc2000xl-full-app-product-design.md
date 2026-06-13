# MPC2000XL Full-App Product Design

Date: 2026-06-13
Status: Draft for user review
Related divergence artifact: `docs/feature/mpc2000xl-clone/diverge/options-raw.md`

## Purpose

Build a full cross-platform desktop instrument that recreates the Akai MPC2000XL working experience with evidence-backed fidelity. The product target is a complete app for macOS, Windows, and Linux, not a reduced prototype. Implementation milestones are proof gates toward the full instrument and do not redefine the end state.

## Product Target

The app must let a musician work through the MPC2000XL mental model: front-panel controls, LCD screens, pads, transport, sequence creation, sample recording and editing, program editing, MIDI, file/storage workflows, and audio output. The app should feel like operating the hardware while using modern desktop devices for audio, MIDI, windows, file selection, and packaging.

The full release target includes:

- Authentic front-panel workflow: LCD, soft keys, cursor keys, data wheel, mode keys, transport, numeric keys, pad banks, pads, shift behavior, and secondary functions.
- Sequencer behavior: songs, sequences, tracks, bars, timing correction, swing, tempo, metronome, recording, overdub, play, stop, locate, erase, copy, edit, and event-level operations.
- Sampler behavior: recording, trimming, editing, assigning samples to programs, playback, voice allocation, envelopes, pitch, filters where applicable, and resampling/export paths if supported by the reference behavior.
- Program behavior: pads, layers, tuning, levels, pan, mute groups, velocity behavior, pad banks, and program save/load.
- MIDI behavior: input, output, sync, channel routing, notes, controllers, program changes, timing, and external device use.
- Storage behavior: virtual disks, sample/program/sequence/project files, import/export, removable-media workflow, and clear host file access.
- Audio/MIDI settings: desktop audio device selection, buffer configuration, MIDI port selection, clock/sync settings, and diagnostics.
- Cross-platform packaging: signed/notarized macOS builds when possible, Windows installer/package, Linux package or AppImage-style deliverable, and reproducible build notes.

## Non-Negotiable Fidelity Bar

"Exact clone" means evidence-backed behavior, not visual similarity alone. A behavior is complete only when it has:

1. A mapped source or accepted reference path.
2. An implementation in the machine core or host shell.
3. A repeatable verification fixture, automated test, or documented manual verification procedure.
4. A known tolerance model for timing, audio, or analog-modeled behavior where bit-identical behavior is unrealistic.

## Architecture

### Machine Core

Owns MPC state and command semantics. It must be deterministic and runnable without a UI so conformance tests can drive it directly.

Responsibilities:

- Sequences, tracks, songs, events, timing, tempo, quantization, swing, and transport state.
- Programs, pad banks, pad assignments, sample references, mixer settings, and edit state.
- Screen/mode state, command dispatch, menus, soft key behavior, error states, and undo/redo if supported by the reference behavior.
- File-system-facing state as logical operations, not direct host file dialogs.

### Front Panel Runtime

Owns hardware-style input and display behavior.

Responsibilities:

- Render an original, rights-safe MPC-inspired front panel with LCD, controls, and pads.
- Map mouse, keyboard, MIDI controller, and optional gamepad events to hardware-style control events.
- Preserve stable control geometry and prevent layout shifts across desktop sizes.
- Support configurable mappings without hiding the default MPC workflow.

### Audio/MIDI Engine

Owns real-time sound and device integration.

Responsibilities:

- Audio scheduling, sample playback, voice allocation, metronome, recording input, and output routing.
- MIDI input/output, clock/sync, external device timing, and event timestamping.
- Host buffer handling, underrun reporting, and latency diagnostics.
- Timing tests with tolerance windows.

### Storage/File Engine

Owns MPC media semantics and host file integration.

Responsibilities:

- Virtual disks, removable-media metaphors, disk image support, file browser behavior, save/load flows, and project import/export.
- Parsing and writing known MPC2000XL-related file formats where supported by reference evidence.
- Host file access through platform shell adapters, not direct machine-core dependencies.

### Conformance Layer

Owns proof that the app behaves like the reference instrument.

Responsibilities:

- Source maps from manuals, service schematics, VMPC assessment, firmware spike, and hardware traces.
- Behavior matrix, fixture catalog, golden tests, coverage reports, and regression gates.
- Verification state tracking for each behavior.

### Desktop Shell

Owns cross-platform packaging and host integration.

Responsibilities:

- Window management, preferences, menus, installers, app updates if needed, crash/error reporting hooks, logs, and device settings UI.
- Platform-specific audio/MIDI/file adapters behind stable interfaces.

## Data Flow

User input enters through the front panel runtime, becomes a hardware-style event, and is sent to the machine core. The machine core updates deterministic MPC state and emits screen/audio/MIDI/storage intents. The audio/MIDI engine handles real-time output and device input. The storage engine resolves disk/file operations. The front panel runtime renders the resulting LCD and control state. The conformance layer can drive the same machine core with scripted events and compare the emitted state against fixtures.

## Error Handling

Errors should be represented at the correct layer:

- Reference-machine errors, such as invalid operations or missing media, appear as MPC-style LCD messages.
- Host integration errors, such as missing audio devices or denied file permissions, appear in desktop shell dialogs or settings diagnostics.
- Conformance failures appear in test output and coverage reports, not in the musician-facing runtime.
- Unknown source behavior is marked as unmapped rather than guessed silently.

## Legal And Asset Boundary

The repo must not commit or redistribute proprietary Akai OS images, copyrighted manual pages, service-manual scans, original MPC artwork, logos, factory samples, or third-party media unless a license is confirmed. Research docs may reference local source paths and public URLs. Product code and tests should use independently written behavior descriptions, synthetic fixtures, clean metadata, user-supplied files, or documented hashes.

Firmware emulation, if pursued, requires user-supplied legally obtained OS images. The app can provide an importer and validation workflow, but must not bundle firmware. Product branding must avoid implying Akai endorsement. References to Akai and MPC2000XL are nominative and documentation-focused.

## JJ-OS Evidence Boundary

JJ-OS compatibility for MPC2000XL is not accepted as a confirmed requirement until a real MPC2000XL JJ-OS target is verified. The supplied OS page lists Akai MPC2000XL OS 1.14 and 1.20 entries and lists JJ OS for MPC1000/MPC2500. The desired JJ-OS-like target can remain an investigation item, but the product spec must not build on an unverified firmware premise.

## Release Definition

A full release is ready only when:

- Core MPC workflows are implemented across front panel, machine core, audio/MIDI, and storage.
- Behavior coverage reports identify implemented, fixture-backed, and remaining unmapped behavior.
- Cross-platform packages are built and smoke-tested on macOS, Windows, and Linux.
- Audio/MIDI latency and timing are measured with documented tolerance windows.
- Legal asset handling is documented and enforced by repo checks.
- The app can be used as an instrument, not just as a visual simulator.

## Non-Goals

- Do not ship proprietary firmware, manuals, logos, factory samples, or copied industrial artwork.
- Do not present incomplete milestones as the full product.
- Do not make DAW-first convenience override reference MPC workflow.
- Do not treat VMPC, firmware output, or hardware traces as unquestioned truth when they conflict; conflicts become investigation items.

## Risks

- Firmware emulation may be blocked by incomplete CPU/peripheral knowledge or OS image format issues.
- VMPC reuse may be blocked by license, architecture, build health, or behavior gaps.
- "Exact" audio and timing behavior requires tolerance models and hardware evidence.
- The full app is broad enough that conformance discipline is required to avoid drifting into a lookalike.
