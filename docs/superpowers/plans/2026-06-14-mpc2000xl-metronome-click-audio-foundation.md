# MPC2000XL Metronome Click Audio Foundation

## Behavior Slice

This slice renders existing count-in metronome click intents as deterministic, rights-safe audio and routes them through the existing host-audio abstraction. It does not add a real audio-device backend, external media, sample files, proprietary click assets, or MPC2000XL hardware-output parity claims.

Implemented behavior:

- `mpc_audio` exposes `AudioRenderKind`, defaulting legacy summaries to `sample_playback`.
- `AudioRenderSummary` can identify `count_in_click` renders and carries optional `count_in_tick`, `bar_index`, `beat_index`, and `accent` metadata.
- `render_count_in_click` consumes `mpc_core::CountInClickIntent` and returns `RenderedAudio`.
- Count-in click frames are generated stereo PCM using deterministic integer synthesis, bounded by `AudioRenderSettings`, with `source_kind = rights_safe_generated` and `loaded_audio_byte_count = 0`.
- Accent clicks render with a higher peak amplitude than non-accent clicks.
- `HostAudioEngine::play_count_in_click_with_render_summary` feeds generated click audio through the existing `play_rendered` path.
- The desktop app routes `MachineOutput::MetronomeClick` through host audio while preserving visible count-in bar/beat/tick status text.
- Enabled host audio updates the same host-audio counters/events for count-in clicks; disabled host audio reports ignored and does not enqueue.

## Deterministic Contract

- Count-in timing remains owned by `mpc_core`.
- This slice does not change the existing `CountInClickIntent` fields or when core emits `MachineOutput::MetronomeClick`.
- Click rendering is a renderer-level contract: same intent and settings produce the same frames and summary.
- The generated click is intentionally synthetic and is not a sampled MPC2000XL metronome sound.

## Source And Evidence Status

This behavior is a fixture-backed internal-spec/manual-investigation foundation. Exact MPC2000XL metronome timbre, DAC behavior, output routing, latency, click volume, hardware mixer path, and device timing tolerance remain pending accepted manual, firmware, service, or hardware evidence.

No proprietary samples, firmware bytes, recordings, binary audio assets, copied media, screenshots, or external click files are added.

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `CARGO_TARGET_DIR=/private/tmp/mpc2000xl-metronome-click-worker-audio-target cargo test -p mpc_audio -- --test-threads=1`
- `CARGO_TARGET_DIR=/private/tmp/mpc2000xl-metronome-click-worker-desktop-target cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `git diff --check`

Focused tests added:

- Deterministic count-in click rendering.
- Accent peak greater than non-accent click peak.
- Legacy sample render summary JSON defaults to `sample_playback` with absent count-in metadata.
- Disabled host audio ignores count-in clicks without backend enqueue.
- Enabled host audio enqueues and captures count-in click render summaries.
- Conformance fixture `crates/mpc_conformance/tests/fixtures/count_in_click_transport.json` verifies count-in click intents and pad playback route through enabled deterministic capture host audio, with queued/played counters and active voice counts asserted.

## Next Boundaries

Next slices can layer sequence-time render windows, voice lifecycle, metronome preference controls, mixer/metering, and optional real device output behind `HostAudioBackend`. Those slices should keep deterministic CI backends and avoid bundling proprietary audio/media.
