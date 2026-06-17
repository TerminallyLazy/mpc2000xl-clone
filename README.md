# MPC2000XL Clone

This repository is the foundation for a full cross-platform desktop instrument modeled on the MPC2000XL workflow.

The product target is a full app, not a reduced prototype. The first code layers establish deterministic core behavior, fixture-backed conformance, a rights-safe desktop shell, and research tooling for user-supplied firmware images.

## Legal Asset Boundary

Do not commit proprietary Akai OS images, manuals, service scans, factory samples, copied front-panel artwork, logos, or hardware photos. Research docs can reference local paths and public URLs. Code and tests must use independently written behavior descriptions, synthetic fixtures, clean metadata, and user-supplied assets.

## Verification

Run the workspace checks from the repository root:

```bash
cargo fmt --all --check
cargo test --workspace
cargo check -p mpc_desktop
python3 tools/check_assets.py
```

## Project Files

The desktop shell can save and load repo-owned metadata snapshots at paths ending in `.mpc2000xl-project.json`. The default path is ignored by git:

```bash
local-assets/projects/last.mpc2000xl-project.json
```

These files are JSON produced by `mpc_core` project snapshot APIs. They do not embed audio bytes or proprietary Akai assets, and they are not a claim of native MPC2000XL disk-format compatibility.

## Runtime WAV Import

The desktop SAMPLE view can load a user-owned 16-bit PCM mono/stereo WAV from an ignored local path such as:

```bash
local-assets/samples/import.wav
```

The decoded audio stays in memory for playback only. Project files persist imported sample metadata plus rights-safe media references to user-owned local WAV paths. They do not embed WAV bytes. On project load, the desktop app tries to relink those paths and marks missing imported pads without deleting their assignments.

## Runtime Host I/O

The desktop shell can refresh and select real host audio output devices through CPAL. Capture mode remains the deterministic test backend. MIDI output sends note-on and scheduled note-off messages through the selected MIDI backend so external synths do not hang on one-shot pad or sequence playback.

Pad lights are runtime UI state. Assigned pads are dim, recent hits glow briefly, missing imported WAV payloads are marked distinctly, and active strikes use velocity-derived brightness.
