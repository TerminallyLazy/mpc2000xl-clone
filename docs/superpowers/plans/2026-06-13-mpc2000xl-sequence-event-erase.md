# MPC2000XL Sequence Event Erase Foundation

## Behavior Slice

- MAIN `F5 Erase` dispatches `PanelControl::SoftKey(5)` and removes the most recently recorded `SequenceEvent` on the currently selected track.
- Erase is selected-track scoped. Events on other tracks remain in their original relative order.
- A successful erase emits `MachineOutput::SequenceEventsErased { selected_track, count, events }` followed by `LcdChanged`; the MAIN LCD event count is refreshed after removal.
- If the selected track has no recorded events, erase emits `Ignored { reason: "sequence.erase.track_<n>.no_events" }` and does not refresh or rebuild the LCD.
- Erase preserves playing, recording, playhead ticks, playhead fractional remainder, active pad bank, program assignments, and `last_playback`.
- `last_playback` remains unchanged because this slice treats erase as sequence metadata editing, not a new playback decision or audio event.
- Project snapshots persist only the remaining `recorded_events`; restoring after erase round-trips the retained events and existing machine metadata.
- Scheduled playback uses the remaining event list, so erased events are not scheduled after the erase.

## Source And Evidence Status

- This is an internal-spec foundation behavior.
- Exact MPC2000XL and JJ-OS erase-screen parity is not claimed here.
- Real erase-mode menus, erase ranges, note filters, track/all-track policies, undo behavior, and destructive-confirmation flow remain unmapped.
- No proprietary manuals, service scans, firmware bytes, JJ-OS assets, factory samples, or copied third-party code are stored in the repository.

## Verification Scope

- Core tests cover successful MAIN F5 erase, no-event structured ignore, selected-track scoping, playing/recording preservation, playback after erase, snapshot round-trip after erase, and output serialization shape.
- Conformance fixture `sequence_event_erase_last_banked_round_trip.json` records a bank-B event, records a later bank-A event, erases the latest event, and verifies the retained bank-B event survives project restore.
- Desktop exposes the existing LCD F5 route and a visible `Erase last event` button near sequence status.

## Deferred Work

- Reference-verified MPC2000XL/JJ-OS erase screen mapping.
- Range-based erase by bar/beat/tick.
- Pad/note/track/all-track filters.
- Undo or confirmation semantics, if reference behavior requires them.
