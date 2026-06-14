# MPC2000XL Count-In Click Foundation

## Source And Evidence Status

This slice is an internal deterministic transport foundation. It does not claim MPC2000XL or JJ-OS parity, and it does not implement real metronome audio, audio-device routing, hardware timing tolerance, MIDI clock, SMPTE/MTC, or external sync.

The accepted behavior is repo-owned and fixture-backed:

- SETUP `count_in_bars` already controls the number of pre-roll bars, bounded to 0..4.
- SETUP `metronome_enabled` controls whether count-in click intents are emitted during pre-roll.
- Armed recording plus PLAY, and OVERDUB, start a deterministic count-in when `count_in_bars > 0`.
- During count-in, sequence playhead does not advance and pad/MIDI strikes do not record.
- Tick events consume the count-in pre-roll using the same tempo-to-tick math as sequence playback.
- Count-in completion starts normal sequence advancement on the next tick batch.

## Deterministic Contract

- One bar is four quarter-note beats at the existing 96 PPQN internal grid.
- Count-in total ticks are `count_in_bars * 4 * INTERNAL_PPQN`.
- Count-in click intents include count-in tick, bar index, beat index, and accent flag.
- Beat 1 is accented; beats 2-4 are unaccented.
- If metronome is disabled, count-in still gates recording/playhead advancement but emits no click intents.
- STOP and LOCATE START clear any active count-in.
- Project snapshots do not persist active count-in transport state.

## Verification Targets

- Core flow tests cover REC+PLAY count-in, OVERDUB count-in, metronome-disabled silent count-in, STOP/LOCATE clearing, and no event recording during pre-roll.
- Conformance fixture `count_in_click_transport.json` covers persisted setup preferences, count-in start, click sequence, count-in completion, and post-count-in recording/playhead behavior.
- Desktop status summarizes count-in started/completed and click intents.
