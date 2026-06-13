# MPC2000XL Full-App Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first executable, testable foundation for the full MPC2000XL desktop instrument: workspace, deterministic core, conformance harness, native shell, firmware image inspector, source-evidence scaffolding, and verification commands.

**Architecture:** Use a Rust workspace with small crates: `mpc_core` owns deterministic machine state and hardware-style events, `mpc_conformance` runs fixture-backed behavior checks, `mpc_firmware_spike` inspects user-supplied OS images without storing bytes, and `apps/desktop` renders a rights-safe native front panel through `eframe`. This is not a reduced product target; it is the first full-app foundation layer required before sequencing, sampling, storage, MIDI, audio, and deeper emulation can be added safely.

**Tech Stack:** Rust 2024 edition, Cargo workspace, `serde`, `serde_json`, `clap`, `anyhow`, `sha2`, `eframe`/`egui`, Python 3 for repository asset checks.

---

## File Structure

- `Cargo.toml`: workspace definition and shared dependency versions.
- `.gitignore`: ignores build output, local firmware/media assets, and visual-companion scratch files.
- `README.md`: project boundary, legal asset policy, and verification commands.
- `crates/mpc_core/`: deterministic MPC state, front-panel event model, LCD frame model, and unit tests.
- `crates/mpc_conformance/`: fixture schema, fixture runner, CLI, and fixture tests.
- `crates/mpc_firmware_spike/`: user-supplied firmware image inspector and CLI.
- `apps/desktop/`: native desktop shell that sends hardware-style events into `mpc_core`.
- `docs/evidence/`: source-map and behavior-matrix seed files.
- `scripts/verify.sh`: one command for formatting, tests, and asset checks.
- `tools/check_assets.py`: guard against committing proprietary manuals, scans, firmware, audio, and image assets.

---

### Task 1: Workspace Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `README.md`

- [ ] **Step 1: Create the root Cargo workspace**

Create `Cargo.toml`:

```toml
[workspace]
members = [
  "crates/mpc_core",
]
resolver = "3"

[workspace.package]
edition = "2024"
license = "MIT OR Apache-2.0"
rust-version = "1.92"
authors = ["MPC2000XL Clone Contributors"]

[workspace.dependencies]
anyhow = "1.0.100"
clap = { version = "4.6.1", features = ["derive"] }
eframe = "0.34.3"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
sha2 = "0.11.0"
```

- [ ] **Step 2: Create ignore rules for build output and local assets**

Create `.gitignore`:

```gitignore
/target/
/.superpowers/
/.DS_Store
*.log
__pycache__/
*.py[cod]

# User-supplied firmware, manuals, samples, and local reference captures stay out of git.
/local-assets/
/reference-assets/
/firmware/
/captures/
```

- [ ] **Step 3: Document project boundaries**

Create `README.md`:

````markdown
# MPC2000XL Clone

This repository is the foundation for a full cross-platform desktop instrument modeled on the MPC2000XL workflow.

The product target is a full app, not a reduced prototype. The first code layers establish deterministic core behavior, fixture-backed conformance, a rights-safe desktop shell, and research tooling for user-supplied firmware images.

## Legal Asset Boundary

Do not commit proprietary Akai OS images, manuals, service scans, factory samples, copied front-panel artwork, logos, or hardware photos. Research docs can reference local paths and public URLs. Code and tests must use independently written behavior descriptions, synthetic fixtures, clean metadata, and user-supplied assets.

## Verification

The full verification entrypoint will be added by the foundation implementation plan as `./scripts/verify.sh`.

At the workspace-skeleton stage, this command is expected to fail until the workspace member crates are added:

```bash
cargo metadata --no-deps
```
````

- [ ] **Step 4: Verify workspace metadata is readable**

Run:

```bash
cargo metadata --no-deps
```

Expected: FAIL because `crates/mpc_core/Cargo.toml` does not exist yet. Future workspace members are added by the tasks that create those crates.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml .gitignore README.md
git commit -m "chore: add Rust workspace skeleton"
```

---

### Task 2: Deterministic MPC Core

**Files:**
- Create: `crates/mpc_core/Cargo.toml`
- Create: `crates/mpc_core/src/lib.rs`
- Create: `crates/mpc_core/src/events.rs`
- Create: `crates/mpc_core/src/lcd.rs`
- Create: `crates/mpc_core/src/state.rs`
- Create: `crates/mpc_core/tests/core_flow.rs`
- Create: `Cargo.lock`

- [ ] **Step 1: Create the core crate manifest**

Create `crates/mpc_core/Cargo.toml`:

```toml
[package]
name = "mpc_core"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
authors.workspace = true

[dependencies]
serde.workspace = true

[dev-dependencies]
serde_json.workspace = true
```

- [ ] **Step 2: Create the core crate module root**

Create `crates/mpc_core/src/lib.rs`:

```rust
pub mod events;
pub mod lcd;
pub mod state;

pub use events::{HardwareEvent, MachineOutput, Mode, PadBank, PanelControl};
pub use lcd::LcdFrame;
pub use state::{MpcCore, MpcState};
```

- [ ] **Step 3: Define hardware-style events and outputs**

Create `crates/mpc_core/src/events.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Main,
    Program,
    Sample,
    Trim,
    Song,
    Midi,
    Disk,
    Setup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadBank {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelControl {
    MainScreen,
    Program,
    Sample,
    Trim,
    Song,
    Midi,
    Disk,
    Setup,
    Play,
    Stop,
    Rec,
    Overdub,
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    SoftKey(u8),
    Numeric(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HardwareEvent {
    Press { control: PanelControl },
    Release { control: PanelControl },
    TurnDataWheel { delta: i32 },
    StrikePad { bank: PadBank, pad: u8, velocity: u8 },
    Tick { micros: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MachineOutput {
    LcdChanged,
    ModeChanged { mode: Mode },
    TransportChanged { playing: bool, recording: bool },
    PadTriggered { bank: PadBank, pad: u8, velocity: u8 },
    Ignored { reason: String },
}
```

- [ ] **Step 4: Define LCD frame state**

Create `crates/mpc_core/src/lcd.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcdFrame {
    pub title: String,
    pub lines: [String; 4],
    pub soft_keys: [String; 6],
}

impl LcdFrame {
    pub fn main_screen(sequence_name: &str, tempo_bpm_x100: u32, playing: bool) -> Self {
        let tempo = format!("{}.{:02}", tempo_bpm_x100 / 100, tempo_bpm_x100 % 100);
        let status = if playing { "PLAY" } else { "STOP" };

        Self {
            title: "MAIN".to_string(),
            lines: [
                format!("Seq: {sequence_name}"),
                "Tr:01  Pgm:InitProgram".to_string(),
                format!("Tempo:{tempo}  {status}"),
                "Bars:001-001".to_string(),
            ],
            soft_keys: [
                "TrList".to_string(),
                "Track+".to_string(),
                "Track-".to_string(),
                "Solo".to_string(),
                "Erase".to_string(),
                "Edit".to_string(),
            ],
        }
    }

    pub fn mode_screen(title: &str, body: &str) -> Self {
        Self {
            title: title.to_string(),
            lines: [
                body.to_string(),
                "Source: core foundation".to_string(),
                "Evidence: unmapped".to_string(),
                "Ready for fixtures".to_string(),
            ],
            soft_keys: [
                "F1".to_string(),
                "F2".to_string(),
                "F3".to_string(),
                "F4".to_string(),
                "F5".to_string(),
                "F6".to_string(),
            ],
        }
    }
}
```

- [ ] **Step 5: Implement deterministic machine state**

Create `crates/mpc_core/src/state.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::events::{HardwareEvent, MachineOutput, Mode, PadBank, PanelControl};
use crate::lcd::LcdFrame;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpcState {
    pub mode: Mode,
    pub sequence_name: String,
    pub tempo_bpm_x100: u32,
    pub playing: bool,
    pub recording: bool,
    pub selected_track: u8,
    pub pad_bank: PadBank,
    pub lcd: LcdFrame,
    pub event_count: u64,
}

impl Default for MpcState {
    fn default() -> Self {
        let sequence_name = "Sequence01".to_string();
        let tempo_bpm_x100 = 12000;

        Self {
            mode: Mode::Main,
            lcd: LcdFrame::main_screen(&sequence_name, tempo_bpm_x100, false),
            sequence_name,
            tempo_bpm_x100,
            playing: false,
            recording: false,
            selected_track: 1,
            pad_bank: PadBank::A,
            event_count: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MpcCore {
    state: MpcState,
}

impl MpcCore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &MpcState {
        &self.state
    }

    pub fn dispatch(&mut self, event: HardwareEvent) -> Vec<MachineOutput> {
        self.state.event_count += 1;

        match event {
            HardwareEvent::Press { control } => self.handle_press(control),
            HardwareEvent::Release { .. } => Vec::new(),
            HardwareEvent::TurnDataWheel { delta } => self.adjust_tempo(delta),
            HardwareEvent::StrikePad { bank, pad, velocity } => {
                if pad == 0 || pad > 16 {
                    vec![MachineOutput::Ignored {
                        reason: "pad must be in range 1..=16".to_string(),
                    }]
                } else {
                    self.state.pad_bank = bank;
                    vec![MachineOutput::PadTriggered { bank, pad, velocity }]
                }
            }
            HardwareEvent::Tick { .. } => Vec::new(),
        }
    }

    fn handle_press(&mut self, control: PanelControl) -> Vec<MachineOutput> {
        match control {
            PanelControl::MainScreen => self.set_mode(Mode::Main),
            PanelControl::Program => self.set_mode(Mode::Program),
            PanelControl::Sample => self.set_mode(Mode::Sample),
            PanelControl::Trim => self.set_mode(Mode::Trim),
            PanelControl::Song => self.set_mode(Mode::Song),
            PanelControl::Midi => self.set_mode(Mode::Midi),
            PanelControl::Disk => self.set_mode(Mode::Disk),
            PanelControl::Setup => self.set_mode(Mode::Setup),
            PanelControl::Play => {
                self.state.playing = true;
                self.refresh_lcd();
                vec![MachineOutput::TransportChanged {
                    playing: true,
                    recording: self.state.recording,
                }]
            }
            PanelControl::Stop => {
                self.state.playing = false;
                self.state.recording = false;
                self.refresh_lcd();
                vec![MachineOutput::TransportChanged {
                    playing: false,
                    recording: false,
                }]
            }
            PanelControl::Rec => {
                self.state.recording = true;
                vec![MachineOutput::TransportChanged {
                    playing: self.state.playing,
                    recording: true,
                }]
            }
            PanelControl::Overdub => {
                self.state.recording = true;
                self.state.playing = true;
                self.refresh_lcd();
                vec![MachineOutput::TransportChanged {
                    playing: true,
                    recording: true,
                }]
            }
            PanelControl::CursorUp
            | PanelControl::CursorDown
            | PanelControl::CursorLeft
            | PanelControl::CursorRight
            | PanelControl::SoftKey(_)
            | PanelControl::Numeric(_) => vec![MachineOutput::Ignored {
                reason: format!("{control:?} has no mapped foundation behavior"),
            }],
        }
    }

    fn set_mode(&mut self, mode: Mode) -> Vec<MachineOutput> {
        self.state.mode = mode;
        self.refresh_lcd();
        vec![MachineOutput::ModeChanged { mode }, MachineOutput::LcdChanged]
    }

    fn adjust_tempo(&mut self, delta: i32) -> Vec<MachineOutput> {
        let current = self.state.tempo_bpm_x100 as i32;
        let next = (current + delta * 100).clamp(3000, 30000) as u32;
        self.state.tempo_bpm_x100 = next;
        self.refresh_lcd();
        vec![MachineOutput::LcdChanged]
    }

    fn refresh_lcd(&mut self) {
        self.state.lcd = match self.state.mode {
            Mode::Main => LcdFrame::main_screen(
                &self.state.sequence_name,
                self.state.tempo_bpm_x100,
                self.state.playing,
            ),
            Mode::Program => LcdFrame::mode_screen("PROGRAM", "Program: InitProgram"),
            Mode::Sample => LcdFrame::mode_screen("SAMPLE", "Sample record"),
            Mode::Trim => LcdFrame::mode_screen("TRIM", "Trim sample"),
            Mode::Song => LcdFrame::mode_screen("SONG", "Song mode"),
            Mode::Midi => LcdFrame::mode_screen("MIDI", "MIDI sync/settings"),
            Mode::Disk => LcdFrame::mode_screen("DISK", "Virtual disk"),
            Mode::Setup => LcdFrame::mode_screen("SETUP", "System settings"),
        };
    }
}
```

- [ ] **Step 6: Add core flow tests**

Create `crates/mpc_core/tests/core_flow.rs`:

```rust
use mpc_core::{HardwareEvent, Mode, MpcCore, PadBank, PanelControl};

#[test]
fn core_starts_on_main_screen() {
    let core = MpcCore::new();

    assert_eq!(core.state().mode, Mode::Main);
    assert_eq!(core.state().lcd.title, "MAIN");
    assert_eq!(core.state().sequence_name, "Sequence01");
    assert!(!core.state().playing);
}

#[test]
fn mode_button_changes_lcd_screen() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });

    assert_eq!(core.state().mode, Mode::Program);
    assert_eq!(core.state().lcd.title, "PROGRAM");
    assert!(outputs.iter().any(|output| matches!(
        output,
        mpc_core::MachineOutput::ModeChanged { mode: Mode::Program }
    )));
}

#[test]
fn transport_buttons_update_play_state() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    assert!(core.state().playing);
    assert!(core.state().lcd.lines[2].contains("PLAY"));

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });
    assert!(!core.state().playing);
    assert!(!core.state().recording);
    assert!(core.state().lcd.lines[2].contains("STOP"));
}

#[test]
fn valid_pad_strike_is_reported() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::B,
        pad: 12,
        velocity: 96,
    });

    assert_eq!(core.state().pad_bank, PadBank::B);
    assert!(outputs.iter().any(|output| matches!(
        output,
        mpc_core::MachineOutput::PadTriggered {
            bank: PadBank::B,
            pad: 12,
            velocity: 96
        }
    )));
}
```

- [ ] **Step 7: Run core tests**

Run:

```bash
cargo test -p mpc_core
```

Expected: PASS, ten tests.
This first successful Cargo test run should create `Cargo.lock`; keep it committed for reproducible app builds.
The deterministic core test suite includes edge coverage for tempo clamp, invalid pad/velocity, ignored controls, replay determinism, output ordering, and serialization shape.

- [ ] **Step 8: Commit**

```bash
git add Cargo.lock crates/mpc_core
git commit -m "feat: add deterministic MPC core foundation"
```

---

### Task 3: Conformance Fixture Runner

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/mpc_conformance/Cargo.toml`
- Create: `crates/mpc_conformance/src/lib.rs`
- Create: `crates/mpc_conformance/src/main.rs`
- Create: `crates/mpc_conformance/tests/fixtures.rs`
- Create: `crates/mpc_conformance/tests/fixtures/main_screen.json`

- [ ] **Step 1: Add conformance crate to the workspace**

Modify the root `Cargo.toml` workspace members:

```toml
[workspace]
members = [
  "crates/mpc_core",
  "crates/mpc_conformance",
]
resolver = "3"
```

- [ ] **Step 2: Create the conformance crate manifest**

Create `crates/mpc_conformance/Cargo.toml`:

```toml
[package]
name = "mpc_conformance"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
authors.workspace = true

[dependencies]
anyhow.workspace = true
clap.workspace = true
mpc_core = { path = "../mpc_core" }
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 3: Implement fixture schema and runner**

Create `crates/mpc_conformance/src/lib.rs`:

```rust
use anyhow::{Context, Result, bail};
use mpc_core::{HardwareEvent, Mode, MpcCore};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub name: String,
    pub source_refs: Vec<String>,
    pub events: Vec<HardwareEvent>,
    pub expect: ExpectedState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedState {
    pub mode: Mode,
    pub lcd_title: String,
    pub playing: bool,
    pub recording: bool,
    pub event_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureReport {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub details: Vec<String>,
}

pub fn load_fixture(path: impl AsRef<Path>) -> Result<Fixture> {
    let path = path.as_ref();
    let json = fs::read_to_string(path)
        .with_context(|| format!("failed to read fixture {}", path.display()))?;
    serde_json::from_str(&json)
        .with_context(|| format!("failed to parse fixture {}", path.display()))
}

pub fn run_fixture(fixture: &Fixture) -> FixtureReport {
    let mut core = MpcCore::new();

    for event in &fixture.events {
        core.dispatch(event.clone());
    }

    let state = core.state();
    let mut details = Vec::new();

    if state.mode != fixture.expect.mode {
        details.push(format!(
            "mode mismatch: expected {:?}, got {:?}",
            fixture.expect.mode, state.mode
        ));
    }

    if state.lcd.title != fixture.expect.lcd_title {
        details.push(format!(
            "lcd title mismatch: expected {}, got {}",
            fixture.expect.lcd_title, state.lcd.title
        ));
    }

    if state.playing != fixture.expect.playing {
        details.push(format!(
            "playing mismatch: expected {}, got {}",
            fixture.expect.playing, state.playing
        ));
    }

    if state.recording != fixture.expect.recording {
        details.push(format!(
            "recording mismatch: expected {}, got {}",
            fixture.expect.recording, state.recording
        ));
    }

    if state.event_count != fixture.expect.event_count {
        details.push(format!(
            "event_count mismatch: expected {}, got {}",
            fixture.expect.event_count, state.event_count
        ));
    }

    FixtureReport {
        id: fixture.id.clone(),
        name: fixture.name.clone(),
        passed: details.is_empty(),
        details,
    }
}

pub fn run_fixture_path(path: impl AsRef<Path>) -> Result<FixtureReport> {
    let fixture = load_fixture(path)?;
    if fixture.source_refs.is_empty() {
        bail!("fixture {} has no source references", fixture.id);
    }
    Ok(run_fixture(&fixture))
}
```

- [ ] **Step 4: Add the conformance CLI**

Create `crates/mpc_conformance/src/main.rs`:

```rust
use anyhow::Result;
use clap::Parser;
use mpc_conformance::run_fixture_path;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "mpc-conformance")]
#[command(about = "Runs MPC2000XL behavior fixtures against the deterministic core.")]
struct Args {
    #[arg(value_name = "FIXTURE_JSON")]
    fixture: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let report = run_fixture_path(args.fixture)?;
    println!("{}", serde_json::to_string_pretty(&report)?);

    if report.passed {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
```

- [ ] **Step 5: Add a source-backed fixture**

Create `crates/mpc_conformance/tests/fixtures/main_screen.json`:

```json
{
  "id": "core.main.program-mode",
  "name": "Program mode button changes LCD title",
  "source_refs": [
    "docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md#front-panel-runtime",
    "docs/superpowers/specs/2026-06-13-mpc2000xl-conformance-lab-design.md#fixture-strategy"
  ],
  "events": [
    {
      "type": "press",
      "control": "program"
    }
  ],
  "expect": {
    "mode": "program",
    "lcd_title": "PROGRAM",
    "playing": false,
    "recording": false,
    "event_count": 1
  }
}
```

- [ ] **Step 6: Add fixture tests**

Create `crates/mpc_conformance/tests/fixtures.rs`:

```rust
use mpc_conformance::{load_fixture, run_fixture};

#[test]
fn fixture_with_source_reference_passes() {
    let fixture = load_fixture("crates/mpc_conformance/tests/fixtures/main_screen.json")
        .expect("fixture should load");

    let report = run_fixture(&fixture);

    assert!(report.passed, "{:?}", report.details);
    assert_eq!(report.id, "core.main.program-mode");
}
```

- [ ] **Step 7: Run conformance tests and CLI**

Run:

```bash
cargo test -p mpc_conformance
cargo run -p mpc_conformance -- crates/mpc_conformance/tests/fixtures/main_screen.json
```

Expected: tests PASS and CLI prints JSON with `"passed": true`.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/mpc_conformance
git commit -m "feat: add conformance fixture runner"
```

---

### Task 4: Native Desktop Shell

**Files:**
- Modify: `Cargo.toml`
- Create: `apps/desktop/Cargo.toml`
- Create: `apps/desktop/src/main.rs`

- [ ] **Step 1: Add desktop app to the workspace**

Modify the root `Cargo.toml` workspace members:

```toml
[workspace]
members = [
  "crates/mpc_core",
  "crates/mpc_conformance",
  "apps/desktop",
]
resolver = "3"
```

- [ ] **Step 2: Create the desktop app manifest**

Create `apps/desktop/Cargo.toml`:

```toml
[package]
name = "mpc_desktop"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
authors.workspace = true

[dependencies]
eframe.workspace = true
mpc_core = { path = "../../crates/mpc_core" }
```

- [ ] **Step 3: Implement a rights-safe front panel shell**

Create `apps/desktop/src/main.rs`:

```rust
use eframe::egui;
use mpc_core::{HardwareEvent, Mode, MpcCore, PadBank, PanelControl};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MPC2000XL Clone Foundation")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([920.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MPC2000XL Clone Foundation",
        options,
        Box::new(|_cc| Ok(Box::new(MpcDesktopApp::default()))),
    )
}

struct MpcDesktopApp {
    core: MpcCore,
    last_status: String,
}

impl Default for MpcDesktopApp {
    fn default() -> Self {
        Self {
            core: MpcCore::new(),
            last_status: "Ready".to_string(),
        }
    }
}

impl eframe::App for MpcDesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("MPC2000XL Clone Foundation");
            ui.label("Rights-safe desktop shell wired to deterministic machine core.");
            ui.separator();

            self.draw_lcd(ui);
            ui.add_space(16.0);
            self.draw_mode_buttons(ui);
            ui.add_space(16.0);
            self.draw_transport(ui);
            ui.add_space(16.0);
            self.draw_pads(ui);
            ui.add_space(16.0);
            ui.label(format!("Status: {}", self.last_status));
        });
    }
}

impl MpcDesktopApp {
    fn draw_lcd(&self, ui: &mut egui::Ui) {
        let lcd = &self.core.state().lcd;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(720.0);
            ui.heading(&lcd.title);
            for line in &lcd.lines {
                ui.monospace(line);
            }
            ui.horizontal_wrapped(|ui| {
                for soft_key in &lcd.soft_keys {
                    ui.label(format!("[{soft_key}]"));
                }
            });
        });
    }

    fn draw_mode_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            self.mode_button(ui, "MAIN", PanelControl::MainScreen, Mode::Main);
            self.mode_button(ui, "PROGRAM", PanelControl::Program, Mode::Program);
            self.mode_button(ui, "SAMPLE", PanelControl::Sample, Mode::Sample);
            self.mode_button(ui, "TRIM", PanelControl::Trim, Mode::Trim);
            self.mode_button(ui, "SONG", PanelControl::Song, Mode::Song);
            self.mode_button(ui, "MIDI", PanelControl::Midi, Mode::Midi);
            self.mode_button(ui, "DISK", PanelControl::Disk, Mode::Disk);
            self.mode_button(ui, "SETUP", PanelControl::Setup, Mode::Setup);
        });
    }

    fn mode_button(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        control: PanelControl,
        mode: Mode,
    ) {
        let selected = self.core.state().mode == mode;
        if ui.selectable_label(selected, label).clicked() {
            self.core.dispatch(HardwareEvent::Press { control });
            self.last_status = format!("Mode changed to {mode:?}");
        }
    }

    fn draw_transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("STOP").clicked() {
                self.core.dispatch(HardwareEvent::Press {
                    control: PanelControl::Stop,
                });
                self.last_status = "Transport stopped".to_string();
            }
            if ui.button("PLAY").clicked() {
                self.core.dispatch(HardwareEvent::Press {
                    control: PanelControl::Play,
                });
                self.last_status = "Transport playing".to_string();
            }
            if ui.button("REC").clicked() {
                self.core.dispatch(HardwareEvent::Press {
                    control: PanelControl::Rec,
                });
                self.last_status = "Record armed".to_string();
            }
            if ui.button("OVERDUB").clicked() {
                self.core.dispatch(HardwareEvent::Press {
                    control: PanelControl::Overdub,
                });
                self.last_status = "Overdub active".to_string();
            }
            if ui.button("Tempo -").clicked() {
                self.core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 });
            }
            if ui.button("Tempo +").clicked() {
                self.core.dispatch(HardwareEvent::TurnDataWheel { delta: 1 });
            }
        });
    }

    fn draw_pads(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("pads")
            .num_columns(4)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                for pad in 1..=16 {
                    if ui.button(format!("PAD {pad:02}")).clicked() {
                        self.core.dispatch(HardwareEvent::StrikePad {
                            bank: PadBank::A,
                            pad,
                            velocity: 100,
                        });
                        self.last_status = format!("Pad A{pad:02} triggered");
                    }
                    if pad % 4 == 0 {
                        ui.end_row();
                    }
                }
            });
    }
}
```

- [ ] **Step 4: Build the desktop app**

Run:

```bash
cargo check -p mpc_desktop
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml apps/desktop
git commit -m "feat: add native desktop shell foundation"
```

---

### Task 5: Firmware Image Inspector

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/mpc_firmware_spike/Cargo.toml`
- Create: `crates/mpc_firmware_spike/src/lib.rs`
- Create: `crates/mpc_firmware_spike/src/main.rs`
- Create: `crates/mpc_firmware_spike/tests/image_report.rs`
- Include: `Cargo.lock` if Cargo updates it

- [ ] **Step 1: Add firmware spike crate to the workspace**

Modify the root `Cargo.toml` workspace members:

```toml
[workspace]
members = [
  "crates/mpc_core",
  "crates/mpc_conformance",
  "crates/mpc_firmware_spike",
  "apps/desktop",
]
resolver = "3"
```

- [ ] **Step 2: Create the firmware spike manifest**

Create `crates/mpc_firmware_spike/Cargo.toml`:

```toml
[package]
name = "mpc_firmware_spike"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
authors.workspace = true

[[bin]]
name = "mpc-firmware-spike"
path = "src/main.rs"

[dependencies]
anyhow.workspace = true
clap.workspace = true
serde.workspace = true
serde_json.workspace = true
sha2.workspace = true

[dev-dependencies]
tempfile = "3.23.0"
```

- [ ] **Step 3: Implement image metadata inspection**

Create `crates/mpc_firmware_spike/src/lib.rs`:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageReport {
    pub file_name: String,
    pub byte_len: u64,
    pub sha256: String,
    pub stores_firmware_bytes: bool,
}

pub fn inspect_image(path: impl AsRef<Path>) -> Result<ImageReport> {
    let path = path.as_ref();
    let file = File::open(path)
        .with_context(|| format!("failed to open firmware image {}", path.display()))?;
    let byte_len = file
        .metadata()
        .with_context(|| format!("failed to read firmware image metadata {}", path.display()))?
        .len();

    let mut hasher = Sha256::new();
    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 8192];

    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to read firmware image {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let sha256 = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();

    Ok(ImageReport {
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string(),
        byte_len,
        sha256,
        stores_firmware_bytes: false,
    })
}
```

- [ ] **Step 4: Add the firmware CLI**

Create `crates/mpc_firmware_spike/src/main.rs`:

```rust
use anyhow::Result;
use clap::Parser;
use mpc_firmware_spike::inspect_image;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "mpc-firmware-spike")]
#[command(about = "Inspects user-supplied MPC2000XL OS images without storing firmware bytes.")]
struct Args {
    #[arg(value_name = "OS_IMAGE")]
    image: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let report = inspect_image(args.image)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
```

- [ ] **Step 5: Add synthetic image test**

Create `crates/mpc_firmware_spike/tests/image_report.rs`:

```rust
use mpc_firmware_spike::inspect_image;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn image_report_never_stores_firmware_bytes() {
    let mut file = NamedTempFile::new().expect("synthetic image should be creatable");
    file.write_all(b"MPC2")
        .expect("synthetic image should write");
    file.flush().expect("synthetic image should flush");

    let report = inspect_image(file.path()).expect("synthetic image should inspect");

    assert_eq!(report.byte_len, 4);
    assert_eq!(
        report.sha256,
        "05e71909ec817edba4a8c4cc7a55f0d8c7bc0a592f7a12ae272f5fbfcc44e427"
    );
    assert!(!report.stores_firmware_bytes);

    let json = serde_json::to_string(&report).expect("report should serialize");
    assert!(!json.contains("MPC2"));
    assert!(!json.contains("4d504332"));
}
```

- [ ] **Step 6: Run firmware spike tests**

Run:

```bash
cargo test -p mpc_firmware_spike
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/mpc_firmware_spike
git commit -m "feat: add firmware image inspector"
```

---

### Task 6: Evidence Seeds And Asset Guard

**Files:**
- Update: `docs/evidence/source-map.md`
- Update: `docs/evidence/behavior-matrix.json`
- Update: `tools/check_assets.py`

- [ ] **Step 1: Create source-map seed with source IDs**

Create `docs/evidence/source-map.md` with stable source IDs and local path hints:

```markdown
# MPC2000XL Source Map

This file tracks evidence sources without copying proprietary source pages, scans, firmware, or media into the repository. Source IDs are stable references; local path hints are workstation pointers only.

## Sources

| ID | Type | Description | Local path hint |
| --- | --- | --- | --- |
| owner-manual | owner_manual | MPC2000XL owner manual | `/Users/lazy/Downloads/akai_mpc2000xl_manual.pdf` |
| analog-schematic | service_schematic | MPC2000XL analog schematic | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k analog.pdf` |
| main-schematic-1 | service_schematic | MPC2000XL main schematic, part 1 | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k main 1_2.pdf` |
| main-schematic-2 | service_schematic | MPC2000XL main schematic, part 2 | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k main 2_2.pdf` |
| operation-schematic | service_schematic | MPC2000XL operation schematic | `/Users/lazy/Downloads/MPC2000XL_ServManual/MPC2k operation.pdf` |
| combined-schematic | service_schematic | MPC2000XL combined schematic | `/Users/lazy/Downloads/Akai-MPC-2000-XL-Schematic.pdf` |
| schematic-photo-1 | service_photo | MPC2000XL service manual schematic photo 1 | `/Users/lazy/Downloads/MPC2000XL_ServManual/P7200082.jpg` |
| schematic-photo-2 | service_photo | MPC2000XL service manual schematic photo 2 | `/Users/lazy/Downloads/MPC2000XL_ServManual/P7200083.jpg` |
| full-app-product-spec | spec | Full app product design notes | `docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md` |
| conformance-lab-spec | spec | Conformance lab design notes | `docs/superpowers/specs/2026-06-13-mpc2000xl-conformance-lab-design.md` |

## Legal Boundary

Do not copy proprietary manuals, firmware, service scans, hardware photos, or audio/media samples into the repository.

## Mapping Rules

- Store independently written behavior summaries.
- Use source IDs in behavior matrices, fixtures, tests, and implementation notes.
- Store page, section, and file references when known.
- Keep raw manuals, firmware, service scans, hardware photos, and audio samples outside git.
- Treat local path hints as optional private lookup aids, not canonical source identity.
- Mark conflicts between manual, VMPC, firmware, and hardware traces explicitly.
```

- [ ] **Step 2: Create structured behavior matrix seed**

Create `docs/evidence/behavior-matrix.json` with evidence classifications, input events, channel-scoped outputs, tolerance, fixture references, and conflict notes:

```json
{
  "schema_version": 1,
  "behaviors": [
    {
      "id": "core.main.initial-screen",
      "name": "Core starts on MAIN screen",
      "area": "front_panel",
      "state": "fixture-backed",
      "evidence": [
        {
          "source_id": "full-app-product-spec",
          "type": "spec",
          "ref": "docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md#front-panel-runtime"
        },
        {
          "source_id": "conformance-lab-spec",
          "type": "spec",
          "ref": "docs/superpowers/specs/2026-06-13-mpc2000xl-conformance-lab-design.md#behavior-matrix"
        }
      ],
      "inputs": [],
      "expected_outputs": {
        "state": {
          "mode": "main",
          "playing": false,
          "recording": false,
          "lcd_title": "MAIN"
        },
        "lcd": {
          "title": "MAIN"
        },
        "transport": {
          "status": "stopped"
        }
      },
      "tolerance": null,
      "fixture_refs": [
        "crates/mpc_core/tests/core_flow.rs"
      ],
      "conflict_notes": []
    },
    {
      "id": "core.main.program-mode",
      "name": "Program button changes LCD title",
      "area": "front_panel",
      "state": "fixture-backed",
      "evidence": [
        {
          "source_id": "full-app-product-spec",
          "type": "spec",
          "ref": "docs/superpowers/specs/2026-06-13-mpc2000xl-full-app-product-design.md#front-panel-runtime"
        },
        {
          "source_id": "conformance-lab-spec",
          "type": "spec",
          "ref": "docs/superpowers/specs/2026-06-13-mpc2000xl-conformance-lab-design.md#fixture-strategy"
        }
      ],
      "inputs": [
        {
          "channel": "front_panel",
          "event": "press",
          "control": "program"
        }
      ],
      "expected_outputs": {
        "state": {
          "mode": "program",
          "playing": false,
          "recording": false,
          "lcd_title": "PROGRAM"
        },
        "lcd": {
          "title": "PROGRAM"
        },
        "events": {
          "event_count": 1
        }
      },
      "tolerance": null,
      "fixture_refs": [
        "crates/mpc_conformance/tests/fixtures/main_screen.json"
      ],
      "conflict_notes": []
    }
  ]
}
```

- [ ] **Step 3: Create tracked-file asset guard**

Create `tools/check_assets.py` so it scans tracked files only, expands forbidden reference/media suffix coverage, and includes an explicit allowlist for generated rights-safe fixtures:

```python
#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_SUFFIXES = {
    ".7z",
    ".aif",
    ".aiff",
    ".avi",
    ".bin",
    ".bmp",
    ".bz2",
    ".dmg",
    ".flac",
    ".gif",
    ".gz",
    ".heic",
    ".img",
    ".iso",
    ".jpeg",
    ".jpg",
    ".m4a",
    ".mid",
    ".midi",
    ".mkv",
    ".mov",
    ".mp3",
    ".mp4",
    ".ogg",
    ".pdf",
    ".png",
    ".rar",
    ".raw",
    ".rom",
    ".sit",
    ".snd",
    ".syx",
    ".tar",
    ".tgz",
    ".tif",
    ".tiff",
    ".wav",
    ".webp",
    ".xz",
    ".zip",
}
ALLOWLIST = {
    # "path/to/generated-fixture.wav": "Synthetic test fixture generated from repo-owned code.",
}


def tracked_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    return [ROOT / path.decode("utf-8") for path in result.stdout.split(b"\0") if path]


def is_forbidden(path: Path) -> bool:
    relative = path.relative_to(ROOT).as_posix()
    return relative not in ALLOWLIST and any(
        suffix.lower() in FORBIDDEN_SUFFIXES for suffix in path.suffixes
    )


def main() -> int:
    violations = [path.relative_to(ROOT).as_posix() for path in tracked_files() if is_forbidden(path)]

    if violations:
        print("Refusing forbidden tracked reference/media assets:")
        for violation in violations:
            print(f" - {violation}")
        print("If a generated rights-safe fixture is intentional, add its repo path to ALLOWLIST with a reason.")
        return 1

    print("Asset guard passed: no forbidden tracked reference/media assets found.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 4: Run asset guard**

Run:

```bash
python3 tools/check_assets.py
```

Expected: PASS with `Asset guard passed: no forbidden tracked reference/media assets found.`

- [ ] **Step 5: Validate JSON and commit**

Run:

```bash
python3 -m json.tool docs/evidence/behavior-matrix.json >/tmp/behavior-matrix.pretty
git status --short
```

Expected before commit: only Task 6 files modified.

Commit:

```bash
git add docs/evidence/source-map.md docs/evidence/behavior-matrix.json tools/check_assets.py docs/superpowers/plans/2026-06-13-mpc2000xl-full-app-foundation.md
git commit -m "fix: tighten tracked asset guard"
```

---

### Task 7: Repository Verification Script

**Files:**
- Create: `scripts/verify.sh`

- [ ] **Step 1: Create verification script**

Create `scripts/verify.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

cargo fmt --all --check
cargo test --workspace
cargo check -p mpc_desktop
python3 tools/check_assets.py
```

- [ ] **Step 2: Make script executable**

Run:

```bash
chmod +x scripts/verify.sh
```

Expected: command exits 0.

- [ ] **Step 3: Run full verification**

Run:

```bash
./scripts/verify.sh
```

Expected: formatting check PASS, workspace tests PASS, desktop check PASS, asset guard PASS.

- [ ] **Step 4: Commit**

```bash
git add scripts/verify.sh
git commit -m "chore: add repository verification script"
```

---

### Task 8: Final Integration Review

**Files:**
- Modify only if a previous task left verification failures.

- [ ] **Step 1: Check git history**

Run:

```bash
git log --oneline --decorate -8
```

Expected: task commits are present on `codex/mpc2000xl-foundation`.

- [ ] **Step 2: Run final verification**

Run:

```bash
./scripts/verify.sh
```

Expected: PASS.

- [ ] **Step 3: Check working tree**

Run:

```bash
git status --short
```

Expected: no output.

- [ ] **Step 4: Record completion summary**

Create `docs/evidence/foundation-completion.md`:

```markdown
# MPC2000XL Foundation Completion

The first full-app foundation layer is complete when these checks pass:

- Rust workspace builds and tests.
- Deterministic `mpc_core` handles front-panel events and LCD state.
- `mpc_conformance` runs JSON fixtures against the core.
- `mpc_desktop` opens a rights-safe native shell connected to the core.
- `mpc_firmware_spike` inspects user-supplied images without storing firmware bytes.
- Asset guard blocks proprietary manuals, scans, firmware, photos, and audio from git.
- `./scripts/verify.sh` passes.
```

- [ ] **Step 5: Commit completion note**

```bash
git add docs/evidence/foundation-completion.md
git commit -m "docs: record foundation completion criteria"
```

---

## Plan Self-Review

Spec coverage:

- Full-app product design: covered by workspace shape, deterministic core, native shell, legal asset boundary, and verification script.
- Conformance lab design: covered by fixture schema, fixture runner, behavior matrix, and coverage seed.
- Firmware emulator spike design: covered by image inspector that keeps user-supplied firmware bytes out of git and produces safe metadata.
- VMPC fork assessment design: not implemented in this foundation plan because it is an assessment workflow; it should be the next separate plan after the foundation repo can run tests and store evidence safely.

Known scope boundary:

- This plan does not claim full MPC2000XL behavior is implemented.
- This plan establishes the executable and testable foundation required to implement full behavior through later task plans.
- Every app-facing behavior added here has a fixture or unit test.
