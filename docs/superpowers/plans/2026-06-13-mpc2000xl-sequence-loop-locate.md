# MPC2000XL Sequence Loop / Locate Foundation

## Behavior Slice

This slice adds deterministic locate-to-start and sequence loop boundary behavior on top of the recorded playback scheduling foundation. It is internal-spec foundation behavior for the full desktop app, not a claim of complete MPC2000XL or JJ-OS parity.

Implemented behavior:

- `PanelControl::LocateStart` sets `playhead_ticks` to `0`, clears `playhead_tick_remainder`, refreshes the LCD, and emits `MachineOutput::PlayheadLocated { tick: 0 }`.
- `PanelControl::ToggleLoop` toggles `MpcState.loop_enabled`, refreshes the LCD, and emits `MachineOutput::LoopChanged`.
- Sequence length is computed as `bar_count * INTERNAL_PPQN * 4` using `sequence_length_ticks_for_bars`.
- With loop disabled, playback that reaches or crosses sequence end clamps to the sequence length, clears tick remainder, and stops transport.
- With loop enabled, playback that reaches or crosses sequence end wraps to the start and remains playing.
- Loop wrap scheduling emits stored playback events before the boundary, then events after wrap. A recorded event at tick `0` is scheduled only at a loop boundary, not on ordinary playback from tick `0`.
- `loop_enabled` persists in project snapshots as sequence behavior, with a serde default so older snapshots without the field load as loop disabled.
- The desktop app exposes `LOCATE START`, loop toggle, loop status, sequence length, and playhead ticks near the transport/status area.

## Deterministic Assumptions

- The current 96 PPQN timing basis remains an internal foundation pending exact source mapping.
- Bar length assumes 4/4 for this slice.
- Loop wrap schedules each stored event at most once per tick dispatch, even if a very large host tick could represent multiple loop cycles. Exact multi-cycle catch-up behavior remains future work.
- Non-loop end-of-sequence behaves like an automatic stop and clears both playing and recording flags.
- Recorded events outside the current sequence length are not scheduled by the bounded playback window.

## Source And Evidence Status

This slice is backed by repo-owned tests, JSON conformance fixtures, and internal behavior documentation. Manual page mapping, firmware traces, hardware captures, VMPC comparative evidence, and JJ-OS-specific behavior are still investigation items.

No proprietary assets, binary media, firmware bytes, screenshots, manual scans, copied sequence-screen text, or samples are included.

## Verification

Required verification for this slice:

- `cargo fmt --all --check`
- `cargo test -p mpc_core sequence_loop -- --nocapture`
- `cargo test --workspace`
- `cargo check -p mpc_desktop`
- `python3 tools/check_assets.py`
- `git diff --check`

## Next Boundaries

Future slices should map exact reference behavior before claiming accuracy for bar/beat/tick display, sequence end policy, multi-loop catch-up, count-in/metronome, overdub merge policy at loop wrap, timing correction, event editing, mute/solo, MIDI output, voice allocation, or audio scheduling latency.
