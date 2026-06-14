# MPC2000XL Host Audio Plumbing Foundation

## Behavior Slice

This slice adds host-audio plumbing for the full MPC2000XL clone. It is toward the full app and later real output work; it is not an MVP and not a fragile real-time device integration.

Implemented behavior:

- `mpc_audio` exposes typed host-audio API values: `HostAudioEngine`, `HostAudioBackend`, `HostAudioState`, `HostAudioEvent`, `HostAudioError`, deterministic backends, and receipt/error structs.
- `HostAudioEngine` can consume either a `SamplePlaybackIntent` or an already-rendered `RenderedAudio`.
- Playback intent handling is fed by the existing deterministic `mpc_audio::render_intent` renderer through the host engine and reuses engine-owned `AudioRenderSettings` validation.
- `HostAudioEngine::play_intent_with_render_summary` returns a typed playback report so UI callers can show render summaries without selecting render settings or calling the renderer directly.
- Host audio has an explicit enabled/disabled mode.
- Disabled host audio returns a structured `HostAudioEvent::Ignored` result and does not enqueue to the backend.
- Enabled host audio validates render settings and render/frame-count consistency before calling the backend.
- Backend receipts are constructor-driven and validated before success counters increment, including played-without-queued and frame-count mismatch rejection.
- Host state reports backend name, render settings, queued render count, played render count, and the last typed event, including success or failure status.
- `NullAudioBackend` and `CaptureAudioBackend` are deterministic and do not open real audio hardware.
- `CaptureAudioBackend` clamps requested history to `MAX_CAPTURE_AUDIO_BACKEND_CAPTURES` and stores bounded latest render summaries and frame counts, not full audio frames.
- Backend failures propagate through `HostAudioEvent::Failed` without panicking or incrementing success counters.
- The desktop app includes a host-audio enable checkbox, visible backend/mode/counter/event summary, and routes sample playback intents through the host engine while keeping the offline synthetic render summary visible.

## Device Abstraction

The host layer is intentionally behind `HostAudioBackend`. The backend receives a validated `RenderedAudio` reference and returns a typed receipt with queued/played status. The engine validates backend receipt invariants, owns state counters and event recording, and records invalid receipts as host failures before incrementing counters; the backend owns only the output/capture behavior.

Current backends:

- `NullAudioBackend`: accepts renders and reports deterministic queued/played receipts without storing history.
- `CaptureAudioBackend`: accepts renders and stores a bounded latest-history list containing `AudioRenderSummary` plus frame count.

No CPAL, CoreAudio, ALSA, JACK, or other real-time device backend is included in this slice. A future real device backend must remain optional, must be testable behind the same trait without opening devices in CI, and must preserve structured error propagation.

## Deterministic Test Backend

The deterministic backends are the CI/default test surface:

- They require no audio device, host permission, sample file, or runtime audio thread.
- They are fed by the rights-safe synthetic renderer.
- They keep memory bounded by existing render settings plus capture history capacity.
- They store summaries/frame counts only, so tests can assert behavior without retaining unbounded PCM buffers.

Focused unit tests cover:

- Enabled playback enqueue through capture backend.
- Disabled playback ignore without backend enqueue.
- Backend error propagation.
- State counters across multiple playbacks.
- Null backend playback without device setup.
- Invalid rendered audio rejection before backend enqueue.
- Oversized capture-history requests clamp before allocation.
- Invalid backend receipts fail without incrementing success counters.
- Engine-owned intent playback exposes render summaries for desktop UI without direct desktop render orchestration.

## Source And Evidence Status

This behavior is a fixture-backed internal-spec/manual-investigation foundation. Exact MPC2000XL audio output behavior, device routing, output latency, DAC characteristics, voice scheduling, envelopes, filters, interpolation, effects, and electrical output path evidence remain pending owner-manual mapping, firmware traces, service evidence, or hardware capture.

No proprietary samples, firmware bytes, recordings, binary audio assets, screenshots, or copied reference media are added. The host plumbing continues to use `source_kind = rights_safe_generated` from the deterministic renderer.

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `./scripts/verify.sh`
- `git status --short`

Additional focused check run during development:

- `cargo test -p mpc_audio`
- `crates/mpc_conformance/tests/fixtures/host_audio_disabled_ignore.json` verifies disabled host audio ignores playback without capture enqueue or voice allocation.
- `crates/mpc_conformance/tests/fixtures/host_audio_capture_voice_lifecycle.json` verifies enabled capture enqueue, success counters, bounded voice allocation, and oldest-voice stealing through the fixture harness.
- `crates/mpc_conformance/tests/fixtures/count_in_click_transport.json` verifies count-in metronome clicks and pad playback share the deterministic host-audio capture path.

## Next Boundaries

Next slices should keep the device abstraction intact while moving toward full app behavior. Practical boundaries include sequence playback scheduling into render windows, voice lifecycle and polyphony policy, program/sample parameter expansion, envelopes, filters, mute groups, output routing, metering, and an optional real host-device backend with deterministic tests and no CI device requirement.
