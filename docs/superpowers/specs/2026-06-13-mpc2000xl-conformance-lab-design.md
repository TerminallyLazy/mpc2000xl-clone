# MPC2000XL Conformance Lab Design

Date: 2026-06-13
Status: Draft for user review
Related product spec: `docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md`

## Purpose

Define the proof system for the MPC2000XL desktop app. The conformance lab makes exactness inspectable by mapping behaviors to sources, turning those behaviors into fixtures, and preventing regressions as the app grows.

## Evidence Sources

### Owner Manual Source Map

The 208-page local manual at `/Users/lazy/Downloads/akai_mpc2000xl_manual.pdf` is the primary user-visible behavior source. The source map should index:

- Screens, modes, menu labels, soft key behavior, and error messages.
- Sequencer operations, recording flows, event editing, songs, tracks, timing correction, swing, tempo, and transport.
- Sampling, trimming, program assignment, program editing, pad banks, mixer behavior, and save/load workflows.
- MIDI, sync, file/storage behavior, and utility/settings workflows.

The source map must store independently written summaries and references to page/section identifiers, not copied manual pages.

### Service And Schematic Source Map

The local service/schematic inputs are:

- `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k analog.pdf`
- `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k main 1_2.pdf`
- `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k main 2_2.pdf`
- `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k operation.pdf`
- `/Users/lazy/Downloads/Akai-MPC-2000-XL-Schematic.pdf`
- `/Users/lazy/Downloads/MPC2000XL_ServManual/P7200082.jpg`
- `/Users/lazy/Downloads/MPC2000XL_ServManual/P7200083.jpg`

The schematic source map should capture hardware modules that influence software behavior: LCD/control-panel wiring, button and pad scanning, MIDI, internal storage/SCSI, memory, CPU/peripheral boundaries, audio input/output, and analog signal path. It should not store raw scans in the repo.

### VMPC Comparative Evidence

VMPC2000XL can be used as comparative evidence after its license, buildability, architecture, and behavior coverage are assessed. VMPC behavior is useful for gap discovery and smoke comparisons. It is not automatically authoritative when it conflicts with manual, firmware, or hardware evidence.

### Firmware Evidence

If a user-supplied Akai MPC2000XL OS image can boot in a firmware spike, captured screen states, storage operations, MIDI behavior, and timing outputs become reference evidence. Firmware assets stay outside the repo.

### Hardware Trace Evidence

If a physical MPC2000XL is available, the lab should support:

- MIDI event captures.
- Audio captures for timing, level, envelope, and signal-path comparisons.
- Disk image before/after diffs for file workflows.
- LCD image/OCR traces for screen flows.
- Button/pad event scripts where hardware actuation is available.

## Behavior Matrix

Each behavior gets a stable ID and tracked state:

- `unmapped`: known area exists, but source mapping is not complete.
- `specified`: source-backed expected behavior is written.
- `implemented`: app behavior exists.
- `fixture-backed`: at least one repeatable test or fixture exists.
- `verified`: fixture passes within the defined tolerance.
- `conflict`: two evidence streams disagree and need investigation.

Minimum fields:

- Behavior ID.
- User-facing name.
- Product area.
- Source references.
- Expected behavior summary.
- Inputs/events.
- Expected screen/audio/MIDI/storage outputs.
- Tolerance model, if relevant.
- Current state.
- Fixture path, if available.
- Notes and conflicts.

## Fixture Strategy

Fixtures should be small and explicit. Preferred fixture types:

- Screen-flow fixtures: sequence of panel events and expected LCD state.
- State fixtures: serialized machine-core state after commands.
- MIDI fixtures: input event stream and expected output events with timing tolerance.
- Audio fixtures: deterministic sample playback or capture comparison with tolerance windows.
- Storage fixtures: virtual disk/file input and expected resulting files or metadata.
- Regression fixtures: previously fixed bugs and source conflicts.

Fixtures must avoid proprietary assets. Use synthetic samples, synthetic disk images, generated MIDI files, and clean behavior descriptions.

## Test Harness

The conformance harness drives the machine core without the desktop UI. It should support:

- Loading fixture files.
- Dispatching hardware-style events.
- Advancing deterministic time.
- Capturing screen state, state snapshots, MIDI events, audio summaries, and storage effects.
- Reporting pass/fail, tolerance deltas, unmapped behavior, and coverage.

The desktop UI should have smoke tests, but core behavior verification belongs in the conformance harness.

## Conflict Handling

When sources disagree:

1. Mark the behavior as `conflict`.
2. Record the conflicting evidence streams.
3. Prefer direct MPC2000XL manual/hardware evidence for user-visible behavior.
4. Prefer firmware/hardware traces for hidden timing or device behavior when available.
5. Keep the decision and rationale in the behavior matrix.

## Coverage Gates

Coverage gates should exist before major releases:

- Source-map coverage: major manual sections have behavior IDs.
- Core workflow coverage: create sequence, record notes, assign sample, save/load, MIDI input/output, and playback have fixture-backed tests.
- Regression coverage: each fixed behavioral bug adds a fixture.
- Platform smoke coverage: packaged app opens, audio/MIDI settings load, and front-panel input reaches the machine core on macOS, Windows, and Linux.

## Outputs

The conformance lab should produce:

- Source maps.
- Behavior matrix.
- Fixture catalog.
- Test runner output.
- Coverage summary.
- Conflict report.
- Release conformance notes.

## Risks

- Manual mapping can become too broad without stable behavior IDs.
- Audio and timing tests can become flaky unless tolerance windows are explicit.
- Hardware evidence may be unavailable, so the lab must still add value with manual and synthetic fixtures.
- VMPC comparison can mislead if treated as a substitute for source-backed behavior.
