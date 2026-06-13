# MPC2000XL Firmware Emulator Spike Design

Date: 2026-06-13
Status: Draft for user review
Related product spec: `docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md`

## Purpose

Investigate whether a user-supplied Akai MPC2000XL OS image can boot and run through an emulated hardware environment. This is a research spike, not the only product path. Its result informs whether firmware-loaded hardware emulation should become a core implementation strategy.

## Scope

The spike focuses on proof of feasibility:

- Identify CPU architecture, memory map, boot flow, ROM/OS image layout, and required peripherals.
- Determine whether Akai MPC2000XL OS 1.14 or 1.20 images can be legally and technically loaded by the user.
- Emulate the minimum hardware needed to reach meaningful boot or screen output.
- Capture findings, blockers, unknowns, and continue/stop criteria.

The spike does not ship firmware, does not commit OS images, and does not claim JJ-OS support unless an actual MPC2000XL JJ-OS target is verified.

## Research Questions

1. What CPU and peripheral set does the MPC2000XL require at boot?
2. What is the OS image format, expected load address, checksum behavior, and storage path?
3. Which devices must exist before the firmware can draw to LCD, accept panel input, access storage, or emit MIDI/audio?
4. Can existing emulator libraries provide CPU/device support without license conflict?
5. What observable boot milestone can be reached first: reset vector, boot loop, LCD write, menu screen, storage access, MIDI output, or audio output?
6. What evidence can be fed back into the conformance lab?

## Legal Boundary

All firmware images are user-supplied. The app or spike tooling may accept a local path and compute hashes for identification, but the repo must not include firmware bytes or download scripts that fetch proprietary OS files. Documentation can explain the boundary and list known public reference pages, but not redistribute assets.

## Technical Components

### Image Loader

Accepts a local OS image path, validates basic size/hash metadata, and exposes bytes to the emulator process. It should keep logs free of firmware contents.

### CPU Core

Executes the target CPU instruction set, exposes registers/memory for debugging, and supports deterministic stepping. Existing emulator cores may be used if license-compatible.

### Memory And Bus Model

Models RAM, ROM/OS image mapping, memory-mapped I/O, interrupts, and reset behavior. The first milestone is accurate enough mapping to reach known firmware execution points.

### Peripheral Stubs

Initial stubs can respond minimally to LCD, panel, timer, storage, MIDI, and audio interactions. Stubs should log accesses so unknown device behavior can be mapped.

### Trace Recorder

Records CPU events, memory-mapped I/O accesses, LCD writes, MIDI events, storage commands, and timing markers. These traces become conformance evidence if useful.

## Milestones

1. Static architecture report: CPU, memory, image format hypotheses, and required peripheral list.
2. Loader report: accepted user-supplied image metadata and validation behavior.
3. Reset-vector milestone: emulator starts execution from the expected boot location.
4. Device-access milestone: logs identify LCD/panel/storage/timer/MIDI/audio access patterns.
5. First-visible-output milestone: LCD or equivalent screen-state output appears, if feasible.
6. Feasibility decision: continue as a product path, keep as reference tooling, or stop.

## Continue Criteria

Continue toward firmware-loaded emulation if:

- The OS image format and boot flow are understood.
- The CPU core can execute enough firmware to reach observable behavior.
- Required peripherals are practical to emulate or stub progressively.
- Licensing of emulator dependencies is compatible with the product.
- The path improves fidelity beyond the clean-room implementation alone.

## Stop Criteria

Stop or downgrade the path if:

- OS image loading cannot be validated without redistributing proprietary assets.
- Required hardware behavior is too undocumented to progress.
- Dependency licensing conflicts with the product.
- Performance or timing constraints make the approach unsuitable for desktop instrument use.
- The spike cannot produce conformance evidence after reasonable investigation.

## Outputs

- Architecture findings.
- Loader behavior notes.
- Memory/peripheral map.
- Trace logs with no proprietary bytes.
- Dependency/license notes.
- Continue/stop recommendation.
- Conformance evidence candidates.

## Risks

- Firmware behavior may depend on hardware details not visible in manuals.
- Booting the OS may be much harder than implementing user-visible behavior natively.
- A partial emulator can consume effort without producing a musician-usable app.
- JJ-OS may not be available for MPC2000XL, so the spike must focus on confirmed MPC2000XL OS images first.
