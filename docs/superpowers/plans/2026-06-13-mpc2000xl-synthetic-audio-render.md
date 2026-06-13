# MPC2000XL Synthetic Audio Render Foundation

## Behavior Slice

This slice adds a deterministic, rights-safe offline audio renderer for the model/render layer of the full MPC2000XL clone. It is toward the full app and later host audio work; it is not an MVP and not a real-time audio-device integration.

Implemented behavior:

- A new workspace crate, `mpc_audio`, consumes `mpc_core::SamplePlaybackIntent`.
- The renderer returns typed `AudioRenderSettings`, `AudioFrame`, `RenderedAudio`, and `AudioRenderSummary` values.
- Output is stereo PCM frame data generated entirely from repo-owned deterministic synthesis.
- Summary metadata includes sample rate, frame count, source sample id/name, selected track, program, pad, velocity, level, pan, peak amplitude, channel balance, source kind, and loaded audio byte count.
- `mpc_conformance` can optionally render the last playback intent when a fixture includes `expect.last_audio_render`.
- Existing conformance fixtures remain backward compatible because the audio render expectation is optional.
- The desktop shell renders a small preview buffer when a sample playback intent occurs and displays the last synthetic render summary.

## Deterministic Assumptions

- Rendering uses integer math for amplitude, channel gain, frame generation, and peak analysis.
- Sample identity is hashed with a fixed FNV-1a style seed derived from `sample_id` and `sample_name`.
- The generated waveform is a seeded square-like waveform. It is intentionally synthetic and does not model MPC2000XL DAC behavior, filters, envelopes, voice allocation, sample start/end, interpolation, or effects.
- Velocity and level scale mono peak amplitude against a 16-bit PCM range using a deterministic `127 x 127` denominator.
- Pan is clamped to `-100..=100` and maps to deterministic left/right channel gains.
- Center pan emits equal left and right peaks. Negative pan reports left balance when left peak is larger; positive pan reports right balance when right peak is larger.
- Render length and sample rate are settings metadata and buffer-size controls only. This slice does not claim source-accurate timing or pitch behavior.

## Source And Evidence Status

This behavior is an internal-spec/manual-investigation foundation. Exact MPC2000XL audio output, timing, pan law, voice allocation, DAC behavior, and sample playback mapping remain pending owner-manual, firmware trace, hardware capture, or other accepted source evidence.

No audio files, sample bytes, factory samples, firmware bytes, copied media, screenshots, or reference recordings are added. The renderer reports `source_kind = rights_safe_generated` and `loaded_audio_byte_count = 0`.

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `./scripts/verify.sh`
- `git status --short`

Focused checks added:

- Same intent and settings render exact same frames and summary.
- Velocity and level change peak amplitude.
- Pan changes left/right channel peaks and balance direction.
- Render frame count and sample rate are preserved.
- Render summaries report rights-safe generated source and zero loaded audio bytes.
- A conformance fixture strikes an assigned pad and checks deterministic render metadata.

## Next Boundaries

Next slices should keep this renderer as a model-layer contract while source evidence is mapped. Practical next boundaries are sequence playback scheduling from recorded intents, render windows that map sequence ticks to frames, richer program/sample parameters, voice lifecycle modeling, envelopes, filters, mute groups, and eventually a host audio integration that consumes this typed render layer without bundling proprietary samples.
