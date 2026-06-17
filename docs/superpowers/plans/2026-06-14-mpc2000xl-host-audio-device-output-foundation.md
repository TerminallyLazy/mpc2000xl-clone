# MPC2000XL Host Audio Device Output Foundation

## Behavior Slice

This slice adds an optional default host output-device backend behind the existing host-audio abstraction. It is a full-app foundation for audible desktop output, not a claim of exact MPC2000XL DAC behavior, analog output routing, scheduler latency, or JJ-OS parity.

Implemented behavior:

- `mpc_audio` depends on CPAL for cross-platform host output-device access.
- `DeviceAudioBackend::open_default` opens the platform default output device, reads its default output config, builds a stream, and starts it.
- The device backend implements the existing `HostAudioBackend` trait, so `HostAudioEngine` validation, counters, events, voice allocation, release, and mute-group choke behavior remain shared with capture/null backends.
- Accepted rendered audio is copied into a bounded device queue and reported as a queued backend receipt.
- Device output accepts rendered buffers only when their sample rate matches the opened device stream; the desktop selector rebuilds host render settings at the device rate when switching to the default device.
- The stream callback drains queued frames, mixes stereo to mono when needed, writes stereo left/right for multi-channel devices, fills extra channels with silence, and outputs silence on underrun.
- Device queue overflow, unavailable default device, unsupported sample format, config failures, stream build failures, stream play failures, and queue-lock poisoning return structured host-audio backend failures without panics.
- Recent stream callback errors are retained in bounded status metadata for desktop display.
- The desktop shell can switch host audio between deterministic capture and the default device backend.
- Desktop status shows device id, sample rate, channel count, sample format, queued queue depth, callback frame count, underruns, and stream error count.

## Deterministic Contract

- Capture and null backends remain the deterministic CI/test surface and never open devices.
- The device backend is optional at runtime and is selected explicitly in the desktop shell.
- Queue capacity is bounded by `DEFAULT_DEVICE_AUDIO_QUEUE_FRAMES` and `MAX_DEVICE_AUDIO_QUEUE_FRAMES`.
- A device enqueue succeeds only when the whole rendered buffer fits; overflow rejects the render without retaining a partial buffer.
- A device enqueue also rejects sample-rate mismatches instead of silently changing pitch or speed. This slice does not include a resampler.
- A successful device enqueue returns queued-but-not-played receipt status because callback playback completion is asynchronous host behavior.
- Stream callback underruns produce silence and increment underrun accounting; they do not mutate `HostAudioEngine` success counters directly.

## Source And Evidence Status

- Status: fixture-backed internal spec plus unit and desktop compile coverage.
- CPAL version: `0.18.1`, used only for host output-device plumbing.
- Rights boundary: no proprietary manuals, service scans, firmware bytes, JJ-OS assets, hardware captures, MIDI files, copied third-party code, audio files, factory samples, or native device traces are stored in the repo.
- Evidence gap: exact MPC2000XL DAC, output levels, analog routing, latency, scheduler timing, interpolation, filters, effects, and JJ-OS behavior remain unmapped.

## Verification Targets

- `cargo fmt --all --check`
- `cargo test -p mpc_audio device_audio_queue -- --test-threads=1`
- `cargo test -p mpc_audio -- --test-threads=1`
- `cargo check -p mpc_desktop`
- `cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1`
- `python3 tools/check_assets.py`
- behavior matrix JSON validation and duplicate-ID guard
- `git diff --check`

Focused checks added:

- Conformance fixture coverage validates deterministic device-output queue semantics without opening CPAL devices.
- Device queue capacity clamps to a bounded maximum.
- Device queue enqueues complete rendered buffers and drains stereo `f32` frames deterministically.
- Mono output mixes left/right and counts underrun silence.
- Overflow rejects the whole render without partial retention.
- Sample-rate mismatches are rejected before enqueue.
- Recent stream callback errors are retained with bounded history.

## Next Boundaries

Future device-output slices should add explicit audio-device selection, resampling or negotiated render-rate selection, latency diagnostics, callback-driven voice advancement, host buffer sizing, optional platform-specific backend features, metering, and manual/hardware evidence mapping before claiming reference accuracy. Exact MPC2000XL DAC/output behavior, native sample playback from user-owned audio bytes, effects, and JJ-OS-specific behavior remain deferred until accepted evidence exists.
