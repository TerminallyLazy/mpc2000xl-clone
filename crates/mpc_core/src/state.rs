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
        self.state.event_count = self.state.event_count.saturating_add(1);

        match event {
            HardwareEvent::Press { control } => self.handle_press(control),
            HardwareEvent::Release { .. } => Vec::new(),
            HardwareEvent::TurnDataWheel { delta } => self.adjust_tempo(delta),
            HardwareEvent::StrikePad {
                bank,
                pad,
                velocity,
            } => {
                if pad == 0 || pad > 16 {
                    vec![MachineOutput::Ignored {
                        reason: "pad must be in range 1..=16".to_string(),
                    }]
                } else if velocity == 0 || velocity > 127 {
                    vec![MachineOutput::Ignored {
                        reason: "velocity must be in range 1..=127".to_string(),
                    }]
                } else {
                    self.state.pad_bank = bank;
                    vec![MachineOutput::PadTriggered {
                        bank,
                        pad,
                        velocity,
                    }]
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
                vec![
                    MachineOutput::TransportChanged {
                        playing: true,
                        recording: self.state.recording,
                    },
                    MachineOutput::LcdChanged,
                ]
            }
            PanelControl::Stop => {
                self.state.playing = false;
                self.state.recording = false;
                self.refresh_lcd();
                vec![
                    MachineOutput::TransportChanged {
                        playing: false,
                        recording: false,
                    },
                    MachineOutput::LcdChanged,
                ]
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
                vec![
                    MachineOutput::TransportChanged {
                        playing: true,
                        recording: true,
                    },
                    MachineOutput::LcdChanged,
                ]
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
        vec![
            MachineOutput::ModeChanged { mode },
            MachineOutput::LcdChanged,
        ]
    }

    fn adjust_tempo(&mut self, delta: i32) -> Vec<MachineOutput> {
        let current = i64::from(self.state.tempo_bpm_x100);
        let delta = i64::from(delta) * 100;
        let next = (current + delta).clamp(3000, 30000) as u32;
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
