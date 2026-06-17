# MPC2000XL Runtime WAV Import Foundation

## Behavior Slice

This slice adds runtime import and playback for user-owned WAV files. It is a full-app sampler foundation for loading local audio into a pad, not a claim of exact MPC2000XL/JJ-OS sample import screens, disk formats, waveform editing, interpolation, resampling, DAC behavior, or native project compatibility.

Implemented behavior:

- `mpc_audio` depends on `hound` for WAV decoding.
- `load_wav_sample_payload` accepts 16-bit PCM mono or stereo WAV files, validates sample rate and size guardrails, and converts samples into the existing stereo `AudioFrame` format.
- Runtime sample payloads are kept in a `RuntimeSampleLibrary` keyed by core sample id.
- `render_intent_with_runtime_samples` renders a loaded user WAV payload when the playback intent's sample id exists in the runtime library.
- Missing runtime payloads fall back to the existing rights-safe generated renderer so deterministic conformance fixtures remain stable.
- Runtime WAV playback applies existing pad velocity, level, pan, trim window, host-audio validation, voice allocation, release, and mute-group behavior.
- Runtime WAV playback rejects render/sample-rate mismatches instead of silently resampling or changing speed.
- `mpc_core` exposes `import_sample_metadata_for_selected_pad`, which assigns imported sample metadata to the selected pad using the decoded WAV frame length and a caller-provided sample name.
- Project snapshots remain metadata-only: they persist ids, names, source kind, lengths, assignments, trims, and playback metadata, but not audio bytes or local file paths.
- The desktop SAMPLE view accepts a local WAV path, loads it into the selected pad, registers the runtime payload in memory, and routes pad strikes through the shared host-audio path.
- Desktop status distinguishes generated renders from runtime WAV renders and reports loaded byte counts.

## Deterministic Contract

- No WAV or other audio media fixtures are committed to the repository.
- Tests generate temporary WAV files at runtime and remove them afterward.
- `CaptureAudioBackend` and generated rendering remain deterministic for CI and conformance.
- Runtime user WAV payloads are memory-only and are lost when the app restarts or a project is reloaded without re-importing the user file.
- This slice intentionally supports 16-bit PCM mono/stereo WAV only.
- Runtime imports are capped by `MAX_RUNTIME_SAMPLE_FRAMES`.
- No resampler is included; the host render rate must match the WAV sample rate.

## Source And Evidence Status

- Status: fixture-backed internal spec plus desktop compile coverage.
- `hound` version: `3.5.1`, used only for decoding user-supplied WAV files at runtime.
- Rights boundary: no proprietary manuals, service scans, firmware bytes, JJ-OS assets, hardware captures, factory samples, WAV files, other audio media, copied third-party code, or native project/sample files are stored in the repo.
- Evidence gap: exact MPC2000XL and JJ-OS sample import UI, native disk formats, sample headers, truncation rules, name normalization, resampling, tuning/interpolation, envelopes, filters, effects, ADC/DAC behavior, and storage relinking remain unmapped.

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_audio -- --test-threads=1`
- `cargo test -p mpc_core --test core_flow sample_metadata -- --test-threads=1`
- `cargo test -p mpc_storage -- --test-threads=1`
- `cargo check -p mpc_desktop`
- `cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `python3 tools/check_assets.py`
- `git ls-files '*.wav' '*.aif' '*.aiff' '*.flac' '*.mp3' '*.ogg' '*.m4a' '*.snd'`
- behavior matrix JSON validation and duplicate-ID guard
- `git diff --check`

Focused checks added:

- Temporary mono WAV decode produces stereo runtime frames.
- Runtime renderer uses loaded WAV payloads by sample id and reports `RuntimeUserWav`.
- Runtime renderer falls back to generated audio when no payload is loaded for a sample id.
- Runtime renderer rejects sample-rate mismatches explicitly.
- Core imported metadata assignment uses decoded name/length and rejects zero-length imports.
- Storage tests keep project JSON metadata-only.
- Conformance fixture `crates/mpc_conformance/tests/fixtures/runtime_wav_import_foundation.json` generates a temporary mono WAV at runtime, imports it through the same runtime loader, strikes A01, and validates `runtime_user_wav` render metadata without committing audio media.

## Next Boundaries

Future sample slices should add file picker ergonomics, durable relink/copy policy, waveform display, destructive/non-destructive trimming, chopping/zones, recording from audio input, AIFF support if source evidence requires it, resampling or render-rate negotiation, envelopes, filters, effects, native MPC sample/program/project formats, and hardware/manual evidence mapping before claiming reference accuracy.
