# MPC2000XL Program Pad Parameter Editing Foundation

## Behavior Slice

This slice extends PROGRAM pad assignment metadata with editable Level, Pan, and Tune values. The later mute-group foundation extends the same edit-field surface with internal mute-group metadata. It is an internal-spec foundation toward full MPC2000XL program editing, not a claim of exact reference PROGRAM-screen behavior.

Implemented behavior:

- `PadAssignment` stores `level`, `pan`, and `tune_cents`.
- `SamplePlaybackIntent` and render summaries carry the current assignment Level, Pan, and Tune values.
- Tune uses a rights-safe foundation range of `-1200..=1200` cents and defaults to `0`.
- PROGRAM mode has a selected edit field: Pad, Level, Pan, or Tune in this original slice; the later mute-group foundation adds Mute Group to the cycle.
- Cursor up/down cycles the selected PROGRAM edit field.
- Cursor left/right still moves the selected PROGRAM pad.
- Data wheel behavior depends on the selected field:
  - Pad moves the selected pad.
  - Level changes the selected assignment level, clamped to `0..=127`.
  - Pan changes the selected assignment pan, clamped to `-50..=50`.
  - Tune changes the selected assignment tune by 100 cents per wheel step, clamped to `-1200..=1200`.
- Editing Level, Pan, or Tune on an unassigned selected pad returns a structured ignored output and leaves the pad unassigned.
- F1 Clear and F2 Assign continue to clear and restore generated assignments, with restored assignments using default Level 100, Pan 0, Tune 0.
- Recorded sequence events snapshot the playback intent, including Tune, so later assignment edits do not rewrite recorded metadata.
- Project snapshots persist assignment tune and selected PROGRAM edit field and reject invalid tune values or unknown JSON fields.
- Desktop PROGRAM status shows the selected edit field plus Level, Pan, and Tune values.

## Render Propagation

The synthetic renderer remains rights-safe and deterministic. It now includes `tune_cents` in `AudioRenderSummary` and uses the tune value to adjust the generated square-wave period and phase. This proves render propagation without loading audio bytes or claiming MPC2000XL pitch accuracy.

## Deterministic Assumptions

- Default generated assignment tune is `0` cents.
- Tune wheel increments are semitone-sized foundation steps of 100 cents.
- Level and pan ranges remain the earlier foundation ranges.
- The selected PROGRAM edit field is UI state and belongs in the project snapshot machine metadata.
- Missing tune or selected edit-field fields in older metadata can default safely to Tune 0 and edit field Pad, while unknown fields remain rejected.

## Source And Evidence Status

This slice is backed by repo-owned tests, fixtures, and implementation contracts. Exact MPC2000XL PROGRAM screen navigation, field naming, pad parameter ranges, pitch law, interpolation, voice behavior, DAC behavior, and audio pitch accuracy remain pending accepted manual, firmware trace, hardware capture, or comparative-source evidence.

No proprietary assets, binary media, firmware bytes, screenshots, manual scans, samples, or real audio files are added.

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test -p mpc_core program_parameter -- --nocapture`
- `cargo test -p mpc_audio tune -- --nocapture`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `git diff --check`

Focused checks added:

- PROGRAM cursor up/down cycles edit fields and LCD marks the selected field.
- Data wheel edits Level, Pan, and Tune with clamping.
- Unassigned pad parameter edits return structured ignored output.
- Pad strikes emit playback intents carrying edited Level, Pan, and Tune.
- Recorded sequence events snapshot edited Level, Pan, and Tune and replay stored metadata after later edits.
- Project snapshot round-trip preserves assignment Tune and selected PROGRAM edit field.
- Project snapshot validation rejects invalid Tune values.
- Audio rendering reports Tune and changes deterministic frames when Tune changes.
- A conformance fixture verifies edited PROGRAM parameters in playback and render metadata.

## Next Boundaries

Next slices should map accepted source evidence before claiming exact MPC2000XL PROGRAM edit behavior. Practical boundaries are reference PROGRAM-screen field flow, pad bank switching in PROGRAM edit contexts, sample start/end, velocity layers, envelopes, filters, evidence-backed mute/choke semantics, voice allocation, and a source-backed pitch/interpolation model.
