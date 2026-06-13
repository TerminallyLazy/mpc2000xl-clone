# MPC2000XL Pad Bank Switching Foundation

## Goal

Make A-D pad bank selection a deterministic core behavior instead of a hard-coded desktop or pad-strike detail. The foundation keeps the rights-safe synthetic program surface and does not claim exact MPC2000XL or JJ-OS behavior yet.

## Behavior Slice

- Add explicit front-panel bank controls for A, B, C, and D.
- Pressing a bank control sets `state.pad_bank`, moves `selected_program_pad.bank` to that bank without changing the selected pad number, refreshes LCD state, and emits `BankChanged` plus `LcdChanged`.
- Physical/simulated pad strikes continue to carry an explicit bank and make `state.pad_bank` match the struck bank.
- PROGRAM mode pad strikes select the struck bank/pad and resolve the visible assignment for that address.
- Generated `Program01` contains 64 metadata-only synthetic assignments for A01-D16.
- Sequence recording, scheduled playback, and host audio routing preserve bank-specific `SamplePlaybackIntent` metadata.
- Restored projects keep exactly the pad assignments stored in the snapshot; A-only snapshots remain valid and missing B/C/D assignments behave as unassigned pads.
- MIDI note-on remains an internal simulation mapping notes 36-51 to bank A pads 1-16. It does not follow the active pad bank in this slice.

## Source And Evidence Status

- `internal_spec`: this plan and conformance fixtures define the current deterministic behavior.
- `manual`: owner-manual pad-bank behavior is still under investigation; no manual text is copied into the repo.
- `firmware` and `jjos`: no verified runtime trace is mapped for bank-switch behavior yet.
- `schematic`: no board-level key matrix evidence is needed for this software event contract.

## Acceptance

- Core unit tests cover bank controls, 64 generated assignments, banked PROGRAM strikes, banked recording metadata, snapshot round-trip, A-only snapshot restore, and MIDI bank-A limitation.
- Conformance includes a bank-B switch/record/project-round-trip fixture.
- Desktop exposes A-D bank controls, labels pads as active-bank addresses, and strikes pads through the active bank.
- Evidence docs record internal-spec status and open reference gaps.
