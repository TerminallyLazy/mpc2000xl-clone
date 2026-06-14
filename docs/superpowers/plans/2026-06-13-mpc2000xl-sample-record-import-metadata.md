# MPC2000XL SAMPLE Record/Import Metadata Foundation

## Source And Evidence Status

This slice is an internal deterministic metadata-only foundation. It does not claim MPC2000XL or JJ-OS parity, and it does not implement real sampling, WAV decode, waveform display, storage of audio bytes, file paths, binary media, manuals, service scans, firmware, factory samples, or proprietary data.

The accepted behavior is repo-owned and fixture-backed:

- SAMPLE F3 creates a recorded sample identity with deterministic metadata length.
- SAMPLE F4 creates an imported sample identity with deterministic metadata length.
- Created metadata is assigned to the selected program pad and selected in the SAMPLE/TRIM catalog.
- Project snapshots persist only sample id, name, source kind, and length metadata through existing pad assignments.
- Old generated sample JSON without source or length fields restores as generated metadata.

## Deterministic Contract

- Source kinds are `generated`, `recorded`, and `imported`.
- Generated sample lengths remain derived from pad position.
- Recorded metadata length is 44,100 frames.
- Imported metadata length is 88,200 frames.
- User sample identifiers are allocated from current program assignments, without a persisted counter:
  - `recorded_001`, `recorded_002`, ...
  - `imported_001`, `imported_002`, ...
- User sample names are `REC-001`, `REC-002`, `IMP-001`, and `IMP-002`.
- Snapshot validation rejects empty id/name, missing recorded/imported length, zero length, and metadata lengths above ten minutes at 48 kHz.

## Verification Targets

- Core flow tests cover creation, assignment replacement, selected catalog metadata, serialization, snapshot round-trip/defaults/validation, and playback/trim windows.
- Conformance fixture `sample_record_import_metadata_round_trip.json` covers record/import metadata creation, selected imported sample state, project round-trip, and restored playback intent using imported metadata.
- Desktop exposes Record meta and Import meta controls only in SAMPLE mode and displays source kind plus effective length.
