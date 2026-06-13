# MPC2000XL Sequence Playback Scheduling Foundation

## Behavior Slice

This slice adds deterministic playback scheduling for recorded sequence events during transport playback. It is internal-spec/manual-investigation foundation behavior for the full app, not a claim of final MPC2000XL reference accuracy.

Implemented behavior:

- `Tick { micros }` keeps using the existing integer tempo and 96 PPQN playhead math.
- While stopped, ticks still produce no outputs and do not schedule recorded events.
- While playing, the scheduler compares the previous and new playhead ticks and schedules stored events where `previous_playhead_ticks < event.tick <= new_playhead_ticks`.
- Events at the current playhead are not retriggered by zero-tick or otherwise non-advancing ticks.
- Recorded events with `playback: Some(intent)` emit the stored `SamplePlaybackIntent`, preserving sample metadata captured at record time.
- Recorded events with `playback: None` do not emit sample playback.
- Multiple scheduled events emit playback intents in recorded event order, including same-tick insertion order.
- `last_playback` reflects the final scheduled playback intent in a tick batch.
- Desktop host audio now processes every `SamplePlaybackIntent` in an output batch instead of only the first.

## Deterministic Assumptions

- The current 96 PPQN timing basis remains an internal foundation pending exact source mapping.
- Sequence playback is append-order deterministic over `recorded_events`.
- This slice does not add looping, locate/rewind controls, timing correction, event editing, mute/solo, MIDI output, voice stealing, audio latency modeling, or sample-accurate scheduling.
- Project snapshots may restore a current playhead before later recorded sequence events so restored playback can cross and schedule those events.

## Source And Evidence Status

This slice is backed by repo-owned plans, unit tests, and JSON conformance fixtures. Manual page mapping, firmware traces, hardware captures, and VMPC comparative evidence are still investigation items.

No proprietary assets, binary media, firmware bytes, screenshots, manual scans, copied sequence-screen text, or samples are included.

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test -p mpc_core sequence_playback -- --nocapture`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `git diff --check`

## Next Boundaries

Future slices should map exact reference behavior before claiming accuracy for bar/beat/tick display, sequence length and loop boundaries, overdub merge policy, event editing, timing correction, metronome/count-in, MIDI routing, voice allocation, or audio scheduling latency.
