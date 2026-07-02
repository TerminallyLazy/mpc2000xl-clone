# Sample Flip Workflow

## Goal

Make it easy to turn an authorized source track into an MPC-style bank of playable chops while preserving the repository's metadata-only and user-supplied-asset boundary.

The first implementation layer is `mpc_core::sample_flip`. It is intentionally UI- and downloader-agnostic so the desktop app, a future playlist scanner, or an LLM-assisted chooser can all use the same deterministic project mutation path.

## Rights-safe input contract

The sample flip flow expects a local WAV or a local manifest entry that the user is allowed to process. Examples include:

- the user's own recordings,
- public-domain recordings,
- Creative Commons recordings compatible with the user's intended use,
- licensed material the user has separately cleared.

The app should not ship a raw YouTube downloader. A future provider can accept playlist URLs only as discovery metadata, then resolve eligible tracks into local files with explicit user authorization and provenance before calling the core planner.

## Planner flow

1. Resolve an authorized source to local metadata: `source_id`, display title, local source path, optional managed-copy path, sample rate, frame count, and byte count.
2. Pick a source region. This can be the full file, a deterministic energy scan, or an LLM/scanner-selected `SampleFlipRegion`.
3. Call `build_pad_bank_sample_flip_plan(source, bank, region)`.
4. Call `apply_sample_flip_plan_to_project_snapshot(snapshot, plan)`.
5. Restore the snapshot into `MpcCore` and relink runtime WAV payloads using the existing desktop runtime-sample path.

## Resulting project metadata

For a 16-pad bank, the planner creates:

- 16 imported `PadAssignment`s,
- 16 `SampleTrim`s that slice the chosen source region,
- 16 `ProjectImportedMediaReference`s pointing back to the local source or managed local copy,
- selected-pad/sample focus on the first slice,
- rewritten playback intents for already-recorded events on the flipped bank.

No WAV bytes, YouTube audio, proprietary Akai assets, or copied media are written into `.mpc2000xl-project.json` snapshots.

## Desktop UI

The desktop SAMPLE panel now exposes `Flip WAV to active bank` beside the existing `Load WAV to pad` control. The button uses the current WAV path field and the active pad bank, builds a 16-slice plan across the full authorized WAV, applies that metadata to the current project snapshot, restores the project state, and loads runtime-only WAV payloads for immediate pad playback.

This first UI pass keeps the safe path as the default: local authorized audio first, metadata saved in project files, runtime WAV payloads held outside git. A later playlist/LLM scanner can add candidate discovery and smarter region picking before calling the same core planner.
