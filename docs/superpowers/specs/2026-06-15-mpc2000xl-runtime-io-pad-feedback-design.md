# MPC2000XL Runtime I/O And Pad Feedback Design

Date: 2026-06-15
Status: Draft for user review
Related divergence artifact: `docs/feature/mpc2000xl-runtime-io-pad-feedback/diverge/options-raw.md`
Related prior plans:

- `docs/superpowers/plans/2026-06-14-mpc2000xl-host-audio-device-output-foundation.md`
- `docs/superpowers/plans/2026-06-14-mpc2000xl-runtime-wav-import-foundation.md`
- `docs/superpowers/plans/2026-06-14-mpc2000xl-midi-output-capture-foundation.md`
- `docs/superpowers/plans/2026-06-14-mpc2000xl-midi-note-off-release-foundation.md`

## Purpose

Make the desktop MPC foundation behave more like a dependable live instrument. This slice connects four related runtime gaps: choosing a real host audio output device, keeping imported WAV pads recoverable after project reload, sending outbound MIDI note-offs so external synths do not hang, and lighting pads with pressure, last-hit memory, and loaded-pad memory.

This is still a foundation slice. It must not claim exact MPC2000XL, JJ-OS, DAC, pad LED, MIDI timing, or sample-disk compatibility until those behaviors are source-mapped and verified.

## Approved Decisions

- Imported WAV reload supports both original source-path relinking and optional managed local copies.
- Project files remain rights-safe and must not embed WAV bytes or third-party media.
- Outbound MIDI note-off is synthesized automatically after fixed or sample-derived duration for one-shot pad clicks and sequence playback.
- Pad lighting uses layered state: assigned base light, last-hit memory glow, active pressure brightness, and missing-media status.
- Audio device choices are treated as host runtime/local preference state, not portable MPC project data.

## Architecture

### Deterministic Core

`mpc_core` remains the deterministic owner of project metadata and machine outputs. It should gain only portable data needed for imported media references and, if needed, typed timing hints for note lifecycle. It must not depend on CPAL, midir, egui, file dialogs, or local device state.

The core validates project snapshot structure and keeps old snapshots loadable. Missing files are not a core error because file existence is a host concern. A restored project with imported samples should keep pad assignments and trim metadata even when WAV payloads cannot be loaded.

### Audio Runtime

`mpc_audio` owns WAV decoding and host output-device access. It should expose output-device descriptors and an explicit open-by-device-id path beside the existing default-device path.

The device backend still uses the shared `HostAudioBackend` trait so render validation, counters, voice allocation, release, and mute-group behavior remain common across capture and device output. Device enqueue still rejects sample-rate mismatches instead of silently resampling.

### MIDI Runtime

`mpc_midi` owns host MIDI message encoding, output sends, and outbound note lifecycle support. It should encode note-on and note-off as distinct message kinds and expose deterministic pending-note scheduling that can be unit tested without a physical MIDI port.

The desktop shell drives scheduler advancement from the same runtime loop that polls MIDI input and repaints pad lights.

### Desktop Orchestration

`apps/desktop` owns runtime state that is local to the host app: selected audio output device, selected MIDI ports, relink attempts, visible relink status, pending-note advancement, and pad visual memory.

The pad grid should be a projection of real playback and release events. Pad visual state must update for mouse pad strikes, incoming MIDI note-on events, sequence playback, note release, and project reload relink status.

## Data Model

### Imported Media References

Project metadata should include imported media references keyed by sample id. A reference records:

- `sample_id`
- `source_path`
- optional `managed_copy_path`
- `sample_name`
- `sample_rate_hz`
- `frame_count`
- `byte_count`
- source kind, constrained to imported user media

The project JSON stores paths and metadata only. It does not store audio bytes. On import, the desktop records the source path and loads the runtime payload. If a managed-copy operation is implemented in the same or a later slice, it copies the user-owned file into an ignored local project asset location and records that path as the preferred relink source.

On project load, desktop relink order is:

1. Try `managed_copy_path` when present.
2. Try `source_path`.
3. Keep project metadata and mark the sample as missing runtime payload.

If a file exists but its decoded sample rate or frame count conflicts with the saved reference, the app should refuse automatic relink and show a mismatch status. A manual re-import can intentionally replace metadata.

### Audio Device Descriptors

Audio output descriptors should include:

- list index for display only
- stable device id or host-provided identifier when available
- user-facing device name
- default-device marker when available
- default sample rate, channel count, and sample format when available

The desktop stores the selected descriptor in runtime state. A future local profile may persist the selected device id outside project JSON.

### Outbound MIDI Notes

Outbound note tracking is keyed by:

- channel
- note
- source pad bank and number
- source sample id
- selected track

Each pending note records the note-on time, scheduled note-off time, velocity, and current send status. When a matching early release occurs, the note-off is sent and the pending note is cleared. When the scheduled time expires, the scheduler sends note-off automatically.

Sample-derived duration should use the playback window when available, clamped by a fixed minimum and maximum. The exact clamp values can be implementation constants in the first slice and should be documented in tests.

### Pad Visual State

Visible pad state is computed from layers:

- Assigned base light: dim when a pad has an assignment in the current bank.
- Missing-media state: distinct warning color or treatment when an imported sample is assigned but its WAV payload is missing.
- Hit memory: decaying glow from recent pad, MIDI, or sequence-triggered playback.
- Active pressure: brightest layer while a press/strike is active, with velocity mapped to intensity.
- Selected pad: outline or border that does not hide the light state.

The visual state is runtime-only. It does not change project data.

## Behavior

### Host Audio Device Picker

The desktop audio area should include refresh, output-device selection, and connect/open actions. Capture mode remains available for deterministic operation. Default-device mode remains available as a fallback, but the user can select a real device rather than only "Default device".

If a selected output device is unavailable, opening it fails visibly and leaves the previous backend unchanged when possible. If no previous backend exists, capture remains the safe fallback. Stream build, config, play, queue, and sample-rate mismatch errors should appear in status text without panics.

### WAV Reload

Saving a project after importing WAV samples writes imported media references but no audio bytes. Loading a project attempts relink immediately. Loaded payloads are inserted into `RuntimeSampleLibrary` by sample id. Missing payloads are shown in status and pad lighting, and assignments remain intact.

The current generated-audio fallback for missing runtime samples may remain for deterministic compatibility, but the UI must not imply the imported WAV is loaded when it is missing.

### Outbound MIDI Note-Off

Every outbound note-on generated from a pad strike or sequence playback should create a pending outbound note. A note-off is sent automatically when the pending note expires. The scheduler must also support explicit early release when a release path exists.

Device and capture backends should both record/send note-off messages so tests can assert the full note lifecycle without hardware. MIDI send failures must update status and clear or mark pending state so the scheduler does not loop forever on the same failed note.

An all-notes-off or panic action should be available as a recovery affordance. It may send note-offs for all pending notes first, then clear pending state.

### Pad Lighting

Pad lights update from real runtime events rather than button-click UI state alone. Mouse strikes, incoming MIDI notes, and sequence playback all refresh hit memory. Velocity maps to brightness. Assigned pads remain dimly lit. Missing imported WAVs are visually distinct from loaded pads. Current pressure overrides memory and assignment brightness while active.

The desktop should request repaint while any hit-memory fade or active pending note remains. When no visual state is changing, repaint can return to normal egui behavior.

## Error Handling

- Audio device refresh failure: keep the old list if useful, show a refresh error, and keep current backend unchanged.
- Audio device open failure: keep capture or previous backend active, show the device name and error.
- WAV missing on reload: preserve assignment, mark missing, and report the path attempted.
- WAV metadata mismatch: preserve assignment, do not relink automatically, and report the expected and actual metadata.
- MIDI note-off send failure: report the failed channel/note/backend and prevent infinite retry.
- MIDI device disconnect or unavailable output: keep pending state bounded and expose panic/clear behavior.
- Project snapshots without media references: load with defaults and no relink attempts.

## Testing

Required automated checks:

- `cargo fmt --all --check`
- `cargo test -p mpc_audio -- --test-threads=1`
- `cargo test -p mpc_midi -- --test-threads=1`
- `cargo test -p mpc_core --test core_flow project -- --test-threads=1`
- `cargo test -p mpc_storage -- --test-threads=1`
- `cargo check -p mpc_desktop`
- `cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `python3 tools/check_assets.py`
- `git diff --check`

Focused test coverage:

- Audio output descriptor listing and selected-device open error handling, with hardware-free coverage where possible.
- Project JSON round-trips imported media references and contains no audio byte payloads.
- Old project snapshots without media references still restore.
- Relink chooses managed copy before source path.
- Relink rejects metadata mismatches.
- MIDI note-on and note-off encode to expected bytes.
- Pending outbound notes expire into note-off messages.
- Early release clears matching pending notes.
- Capture MIDI backend records note-on and note-off lifecycle.
- Pad visual-state computation produces assignment base light, missing-media state, hit memory, active pressure brightness, and selected outline without mutating project state.

Manual smoke checks:

- Refresh and select a real host output device on macOS.
- Import a local WAV, save project, reload project, and confirm the pad still plays the WAV when the source exists.
- Move the WAV, reload, and confirm the pad is marked missing without losing assignment metadata.
- Send pad/sequence MIDI to an external synth or monitor and confirm note-off messages stop sustained notes.
- Strike pads at different velocities and confirm the grid shows base, memory, and active-pressure layers.

## Non-Goals

- No audio bytes or WAV fixtures are committed to git.
- No sample-rate conversion or time-stretching is included in this slice.
- No exact MPC2000XL DAC, pad LED, voice envelope, MIDI timing, or JJ-OS behavior is claimed.
- No native MPC disk/project/sample format compatibility is claimed.
- No platform-specific persistent preferences are required for the first implementation, though the design leaves room for a local profile.

## Risks

- CPAL may not expose stable device ids consistently across platforms, so the first implementation may need a best-effort descriptor and clear unavailable-device fallback.
- Storing local source paths improves reload continuity but can expose machine-local paths in project JSON. The app should make this visible and may later offer managed-copy-only projects.
- Sample-derived MIDI note duration is pragmatic but not reference-backed. It should be labeled as internal policy until hardware/manual evidence is mapped.
- Pad lighting can mislead if it is not driven by the same events that trigger audio and MIDI. The implementation should avoid independent visual-only click state.
- Existing runtime WAV fallback to generated audio is useful for deterministic tests but can hide missing media unless status and pad warning state are clear.

## Acceptance Criteria

- The desktop app can list host audio output devices and open a selected device, not only the default device.
- Imported WAV pads survive project save/load by relinking from saved media references when files are available.
- Missing imported WAVs preserve pad assignments and show visible missing-media status.
- Outbound MIDI note-ons from pad strikes and sequence playback are paired with note-offs through the scheduler.
- Pad lights show assigned memory, recent-hit memory, current pressure brightness, and missing imported media state.
- The rights-safe project boundary remains enforced by tests and asset checks.
