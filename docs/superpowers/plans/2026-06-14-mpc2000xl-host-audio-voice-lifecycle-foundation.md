# MPC2000XL Host Audio Voice Lifecycle Foundation

## Behavior Slice

This slice adds deterministic host-audio voice lifecycle accounting behind the existing `HostAudioEngine` render enqueue path. It is an internal foundation for future polyphony, voice stealing, envelopes, mute groups, and device-backed audio. It is not accepted evidence of exact MPC2000XL voice behavior.

Implemented behavior:

- `HostAudioEngine` keeps a deterministic active voice list for accepted positive-frame queued renders.
- The default host-audio voice limit is 32 as internal foundation policy only, with configurable constructor limits clamped to `1..=128`.
- Each successful positive-frame queued render allocates one monotonic voice id with render kind, source label, total frame count, and remaining frame count.
- Disabled host audio, render failures, invalid rendered audio, backend failures, zero-frame queued receipts, and non-queued backend receipts do not allocate voices or increment voice lifecycle counters.
- `advance_voice_frames` deterministically subtracts frames from active voices, completing and removing voices whose remaining frame count reaches zero. Zero-frame advances are no-ops.
- When an accepted queued render would exceed the configured limit, the oldest active voice is stolen before the new voice is allocated. Enqueue receipts report the allocated voice id and any stolen voice id.
- `HostAudioState` reports the voice limit, active voice count, completed voice count, stolen voice count, and active voice summaries without exposing PCM buffers.
- Count-in click renders allocate voices through the same `play_rendered` path as sample playback.
- Desktop host-audio status includes compact voice counts while preserving existing sample playback enqueue/failure event text.

## Deterministic Contract

- Voice ids are host-engine-local, stable, and monotonic for successful positive-frame queued allocations.
- Active voice ordering is allocation order, so voice stealing removes the oldest active voice.
- Completion is driven only by explicit deterministic frame advancement.
- Rendered PCM remains owned by existing render/backend flow. This slice adds no real audio device backend, sample files, proprietary assets, external media, or parallel audio path.

## Source And Evidence Status

This behavior is internal-spec/manual-investigation foundation. The 32-voice default and stealing policy are repo-owned scaffolding only. Exact MPC2000XL polyphony, release envelopes, mute/choke groups, output routing, DAC behavior, scheduler latency, and hardware timing tolerance remain pending accepted manual, firmware, service, or hardware evidence.

No proprietary manuals, service scans, firmware bytes, recordings, factory samples, binary audio fixtures, copied media, or external sample files are added.

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `CARGO_TARGET_DIR=/private/tmp/mpc2000xl-voice-worker-audio-target cargo test -p mpc_audio -- --test-threads=1`
- `CARGO_TARGET_DIR=/private/tmp/mpc2000xl-voice-worker-desktop-target cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- behavior matrix JSON validation and duplicate-ID guard
- `git diff --check`

Focused tests added:

- Enabled sample playback allocates one default-limit voice.
- Disabled sample playback does not allocate a voice.
- Backend failure does not allocate a voice.
- Deterministic frame advancement completes/removes a voice.
- Small configured voice limits steal the oldest active voice and report the stolen id.
- Count-in click audio allocates through the same host path.
- Zero-frame queued receipts and non-queued backend receipts do not allocate a voice.
- Configurable voice limits clamp to foundation bounds.

## Next Boundaries

Future slices can map evidence-backed polyphony, exact sample envelopes, mute/choke groups, mixer routing, metering, and optional real device output behind `HostAudioBackend`. Those slices should preserve deterministic tests and rights-safe fixtures until accepted reference evidence is available.
