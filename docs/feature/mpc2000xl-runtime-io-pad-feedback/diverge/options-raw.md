# MPC2000XL Runtime I/O And Pad Feedback Divergence Options

## HMW Question

How might we make the desktop MPC feel like a dependable live instrument when it talks to real host audio devices, user-owned WAV files, external MIDI synths, and velocity-sensitive pad feedback?

## SCAMPER Options

### Option 1: Unified Runtime Session

**Core idea**: A user sees one live runtime session area that owns audio output, MIDI output, MIDI input, sample relinks, and pad feedback state.
**Key mechanism**: Replace isolated default-device and runtime-only controls with a shared session model that tracks selected devices, relink status, active voices, scheduled note-offs, and pad lights.
**Key assumption**: Musicians trust the app more when all live external dependencies are visible and managed from one runtime surface.
**SCAMPER origin**: Substitute.
**Closest competitor**: Ableton Live preferences plus session status indicators.

### Option 2: Project-Backed Performance State

**Core idea**: A user reloads a project and the same imported WAV pads, host choices, MIDI behavior, and pad memory return as much as the local machine permits.
**Key mechanism**: Combine project metadata with local runtime references: source WAV paths, optional managed-copy paths, selected host device ids, selected MIDI port ids, note-off policy, and pad light memory.
**Key assumption**: A project should reopen into a playable state without embedding user media bytes or claiming native MPC disk compatibility.
**SCAMPER origin**: Combine.
**Closest competitor**: DAW project files with missing-media relink status.

### Option 3: Hardware-Controller Style Feedback

**Core idea**: A user can read the pad grid like a modern MPC controller: assigned pads are dim, recent hits glow, and current pressure is brightest.
**Key mechanism**: Adapt controller LED layering: base assignment memory, decaying hit memory, active pressure intensity, and deterministic release/fade timers.
**Key assumption**: Visual feedback should support finger memory and confidence, not just selection state.
**SCAMPER origin**: Adapt.
**Closest competitor**: Akai MPC Live/X pad LEDs.

### Option 4: Note Lifecycle First

**Core idea**: A user never hears stuck notes on external synths because every outbound note-on has an explicit tracked note-off.
**Key mechanism**: Magnify note lifecycle correctness by adding a pending-note registry, manual release events where available, and synthesized note-offs after fixed or sample-derived duration for one-shot clicks and sequence playback.
**Key assumption**: Reliable stop behavior matters more than exact MPC note-off evidence in this slice.
**SCAMPER origin**: Modify/Magnify.
**Closest competitor**: DAW MIDI panic and note-lifetime management.

### Option 5: Reusable Device And Media Registry

**Core idea**: A user gets better desktop behavior now while future plugin, test, and hardware-in-the-loop builds reuse the same device/media registry.
**Key mechanism**: Put the runtime I/O state to other use by moving audio device descriptors, MIDI port descriptors, WAV source references, and pad-light snapshots into typed library APIs instead of desktop-only fields.
**Key assumption**: Device selection and media relinking will be needed beyond the current egui shell.
**SCAMPER origin**: Put to other use.
**Closest competitor**: JUCE device managers and media pools.

### Option 6: Minimal Explicit Relink

**Core idea**: A user sees missing WAV pads after reload and can relink them manually, while all other runtime behavior stays mostly unchanged.
**Key mechanism**: Eliminate automatic recovery complexity by storing only sample ids and presenting clear missing-payload status with a per-sample relink action.
**Key assumption**: Explicit recovery is safer than silently depending on stale host paths.
**SCAMPER origin**: Eliminate.
**Closest competitor**: Basic sample-player plugins with missing-file prompts.

### Option 7: Playback Drives The UI

**Core idea**: A user does not configure pad lights directly; pad visuals, note-off scheduling, and status panels derive entirely from real playback and release events.
**Key mechanism**: Reverse the workflow so UI feedback is a projection of machine outputs and host receipts rather than independent button state.
**Key assumption**: The cleanest way to avoid mismatches is to let the same events that play audio and MIDI also drive lights and status.
**SCAMPER origin**: Reverse.
**Closest competitor**: Elm/Redux-style event projection in music tools.

## Crazy 8s Supplements

### Option 8: Panic And Recovery Strip

**Core idea**: A user has a small live strip for "all notes off", audio queue clear, missing sample count, and device reconnect.
**Key mechanism**: Add operational recovery commands that are independent of normal pad and transport workflows.
**Key assumption**: Real host devices fail or disconnect, so recovery must be one click.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: MIDI panic controls in DAWs.

### Option 9: MPC Memory Overlay

**Core idea**: A user can glance at pads and see which are assigned, recently hit, currently sounding, and missing media.
**Key mechanism**: Add a pad-state overlay with separate visual states for assignment, last-hit decay, active voice, missing runtime payload, and selected pad.
**Key assumption**: The pad grid should communicate memory and health, not only input.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: Maschine and MPC software pad grids.

### Option 10: Autosaved Local Runtime Profile

**Core idea**: A user reopens the app and previous audio/MIDI device choices return even before opening a project.
**Key mechanism**: Save host-local preferences outside project JSON, keyed by stable device and port ids, with fallback to default/capture when unavailable.
**Key assumption**: Device choices are often machine preferences rather than project data.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: Most desktop DAW audio/MIDI preferences.

## All Generated Options

1. Unified Runtime Session
2. Project-Backed Performance State
3. Hardware-Controller Style Feedback
4. Note Lifecycle First
5. Reusable Device And Media Registry
6. Minimal Explicit Relink
7. Playback Drives The UI
8. Panic And Recovery Strip
9. MPC Memory Overlay
10. Autosaved Local Runtime Profile

## Curated 6

### Curated Option A: Unified Runtime Session

**Core idea**: A user sees one live runtime session area that owns audio output, MIDI output, MIDI input, sample relinks, and pad feedback state.
**Key mechanism**: Shared runtime state and status projection across device selection, media relinks, active voices, MIDI note lifecycle, and pad lights.
**Key assumption**: Live performance confidence comes from seeing all external dependencies in one consistent runtime model.
**SCAMPER origin**: Substitute.
**Closest competitor**: Ableton Live preferences plus session status indicators.
**Diversity test**: Different mechanism: yes. Different user-behavior assumption: yes. Different cost/effort profile: yes.

### Curated Option B: Project-Backed Performance State

**Core idea**: A user reloads a project and imported WAV pads recover from source paths or managed local copies without embedding audio bytes.
**Key mechanism**: Add rights-safe media references to project metadata, relink on load, and keep an optional managed-copy path for future project-local assets.
**Key assumption**: Project reload should favor playable continuity while remaining explicit about missing or moved user media.
**SCAMPER origin**: Combine.
**Closest competitor**: DAW project files with missing-media relink status.
**Diversity test**: Different mechanism: yes. Different user-behavior assumption: yes. Different cost/effort profile: yes.

### Curated Option C: Note Lifecycle First

**Core idea**: A user never hears stuck notes on external synths because every outbound note-on is paired with tracked note-off.
**Key mechanism**: Pending outbound MIDI note registry with release-driven and synthesized note-off paths for pad clicks and sequence playback.
**Key assumption**: Deterministic note lifecycle is the safest near-term behavior even before exact MPC output evidence is mapped.
**SCAMPER origin**: Modify/Magnify.
**Closest competitor**: DAW MIDI note-lifetime management.
**Diversity test**: Different mechanism: yes. Different user-behavior assumption: yes. Different cost/effort profile: yes.

### Curated Option D: Hardware-Controller Style Feedback

**Core idea**: A user can read the pad grid through layered light state: assignment memory, hit memory, active pressure, and missing media.
**Key mechanism**: Pad visual state derived from assignments, recent strikes, active voices, velocity/pressure, and relink health.
**Key assumption**: A modern MPC-style pad grid should help the player remember what is loaded and what was just touched.
**SCAMPER origin**: Adapt.
**Closest competitor**: Akai MPC Live/X pad LEDs.
**Diversity test**: Different mechanism: yes. Different user-behavior assumption: yes. Different cost/effort profile: yes.

### Curated Option E: Playback Drives The UI

**Core idea**: A user sees visual feedback that is always consistent with the events that actually played audio or MIDI.
**Key mechanism**: UI projections consume machine outputs and host receipts rather than maintaining separate click-only state.
**Key assumption**: Event-derived feedback avoids visual lies when playback comes from pads, MIDI input, or sequencer ticks.
**SCAMPER origin**: Reverse.
**Closest competitor**: Event-projection UI architectures.
**Diversity test**: Different mechanism: yes. Different user-behavior assumption: yes. Different cost/effort profile: yes.

### Curated Option F: Autosaved Local Runtime Profile

**Core idea**: A user reopens the app and previous audio/MIDI device choices return even before project reload.
**Key mechanism**: Host-local profile stores selected audio device ids, MIDI port ids, fallback policy, and runtime UI preferences outside project JSON.
**Key assumption**: Device choices are local workstation preferences, while media references belong with the project.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: Desktop DAW audio/MIDI preferences.
**Diversity test**: Different mechanism: yes. Different user-behavior assumption: yes. Different cost/effort profile: yes.

## Eliminated Options

- Reusable Device And Media Registry was merged into Unified Runtime Session because typed APIs are the implementation shape, not a distinct user-facing strategy for this slice.
- Minimal Explicit Relink was merged into Project-Backed Performance State because manual relink remains the fallback, but the selected direction includes automatic source-path recovery and future managed copies.
- Panic And Recovery Strip was merged into Note Lifecycle First because "all notes off" and queue clearing are recovery commands for the same stuck-note and host-runtime reliability problem.
- MPC Memory Overlay was merged into Hardware-Controller Style Feedback because both depend on the same layered pad-state projection.

## Selected User Constraints

- Imported WAV reload should support both source-path relinking and optional managed local copies, while still embedding no audio bytes in project JSON.
- Outbound MIDI note-off should synthesize note-offs automatically after fixed or sample-derived duration for one-shot clicks and sequence playback.
- Pad lighting should include current pressure, last-hit memory, and loaded/assigned memory in one layered model.
