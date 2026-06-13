use serde::{Deserialize, Serialize};

use crate::events::{
    HardwareEvent, MachineOutput, Mode, PadAssignment, PadAssignmentChange, PadBank, PanelControl,
    PlaybackMissReason, Program, ProgramPad, SamplePlaybackIntent, SamplePlaybackMiss,
    SamplePlaybackResolution, SequenceEvent, SyntheticSample,
};
use crate::lcd::LcdFrame;

/// Internal timing resolution for this foundation slice.
///
/// The exact MPC2000XL timing source mapping is still pending reference
/// evidence, so sequence recording uses a deterministic 96 PPQN basis for now.
pub const INTERNAL_PPQN: u32 = 96;

const MIN_TEMPO_BPM_X100: u32 = 3000;
const MAX_TEMPO_BPM_X100: u32 = 30000;
const MIN_SEQUENCE_INDEX: u8 = 1;
const MAX_SEQUENCE_INDEX: u8 = 99;
const MIN_TRACK_INDEX: u8 = 1;
const MAX_TRACK_INDEX: u8 = 64;
const MIN_BAR_COUNT: u16 = 1;
const MAX_BAR_COUNT: u16 = 999;
const TICK_DENOMINATOR: u128 = 60_000_000_u128 * 100;
const DEFAULT_PROGRAM_INDEX: u8 = 1;
const DEFAULT_PROGRAM_NAME: &str = "Program01";
const DEFAULT_PAD_LEVEL: u8 = 100;
const DEFAULT_PAD_PAN: i8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MainScreenField {
    Sequence,
    Track,
    Tempo,
    Bars,
}

impl MainScreenField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sequence => "sequence",
            Self::Track => "track",
            Self::Tempo => "tempo",
            Self::Bars => "bars",
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Sequence => Self::Bars,
            Self::Track => Self::Sequence,
            Self::Tempo => Self::Track,
            Self::Bars => Self::Tempo,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Sequence => Self::Track,
            Self::Track => Self::Tempo,
            Self::Tempo => Self::Bars,
            Self::Bars => Self::Sequence,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MpcState {
    pub mode: Mode,
    pub sequence_index: u8,
    pub sequence_name: String,
    pub tempo_bpm_x100: u32,
    pub playing: bool,
    pub recording: bool,
    pub selected_track: u8,
    pub bar_count: u16,
    pub selected_main_field: MainScreenField,
    pub pad_bank: PadBank,
    pub current_program: Program,
    pub selected_program_pad: ProgramPad,
    pub last_playback: Option<SamplePlaybackResolution>,
    pub playhead_ticks: u64,
    pub playhead_tick_remainder: u64,
    pub recorded_events: Vec<SequenceEvent>,
    pub lcd: LcdFrame,
    pub event_count: u64,
}

impl Default for MpcState {
    fn default() -> Self {
        let sequence_index = MIN_SEQUENCE_INDEX;
        let sequence_name = sequence_name_for(sequence_index);
        let tempo_bpm_x100 = 12000;
        let selected_track = MIN_TRACK_INDEX;
        let bar_count = MIN_BAR_COUNT;
        let selected_main_field = MainScreenField::Tempo;
        let current_program = default_program();
        let selected_program_pad = ProgramPad {
            bank: PadBank::A,
            pad_number: 1,
        };

        Self {
            mode: Mode::Main,
            lcd: LcdFrame::main_screen(
                sequence_index,
                &sequence_name,
                selected_track,
                &current_program.name,
                tempo_bpm_x100,
                false,
                false,
                bar_count,
                selected_main_field,
                0,
                0,
            ),
            sequence_index,
            sequence_name,
            tempo_bpm_x100,
            playing: false,
            recording: false,
            selected_track,
            bar_count,
            selected_main_field,
            pad_bank: PadBank::A,
            current_program,
            selected_program_pad,
            last_playback: None,
            playhead_ticks: 0,
            playhead_tick_remainder: 0,
            recorded_events: Vec::new(),
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
            HardwareEvent::TurnDataWheel { delta } => self.handle_data_wheel(delta),
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
                    self.handle_pad_strike(bank, pad, velocity)
                }
            }
            HardwareEvent::Tick { micros } => self.handle_tick(micros),
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
                self.refresh_lcd();
                vec![
                    MachineOutput::TransportChanged {
                        playing: self.state.playing,
                        recording: true,
                    },
                    MachineOutput::LcdChanged,
                ]
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
            PanelControl::CursorLeft => self.move_main_field_left(),
            PanelControl::CursorRight => self.move_main_field_right(),
            PanelControl::SoftKey(index) => self.handle_soft_key(index),
            PanelControl::CursorUp | PanelControl::CursorDown | PanelControl::Numeric(_) => {
                Self::ignored(format!(
                    "{}.{control:?}_unimplemented",
                    mode_reason(self.state.mode)
                ))
            }
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

    fn handle_data_wheel(&mut self, delta: i32) -> Vec<MachineOutput> {
        if self.state.mode == Mode::Program {
            self.adjust_selected_program_pad(delta);
            self.refresh_lcd();
            return vec![MachineOutput::LcdChanged];
        }

        if self.state.mode != Mode::Main {
            return Self::ignored(format!(
                "{}.data_wheel_unmapped",
                mode_reason(self.state.mode)
            ));
        }

        match self.state.selected_main_field {
            MainScreenField::Sequence => self.adjust_sequence(delta),
            MainScreenField::Track => self.adjust_track(delta),
            MainScreenField::Tempo => self.adjust_tempo(delta),
            MainScreenField::Bars => self.adjust_bars(delta),
        }

        self.refresh_lcd();
        vec![MachineOutput::LcdChanged]
    }

    fn move_main_field_left(&mut self) -> Vec<MachineOutput> {
        if self.state.mode == Mode::Program {
            self.adjust_selected_program_pad(-1);
            self.refresh_lcd();
            return vec![MachineOutput::LcdChanged];
        }

        if self.state.mode != Mode::Main {
            return Self::ignored(format!(
                "{}.cursor_left_unmapped",
                mode_reason(self.state.mode)
            ));
        }

        self.state.selected_main_field = self.state.selected_main_field.previous();
        self.refresh_lcd();
        vec![MachineOutput::LcdChanged]
    }

    fn move_main_field_right(&mut self) -> Vec<MachineOutput> {
        if self.state.mode == Mode::Program {
            self.adjust_selected_program_pad(1);
            self.refresh_lcd();
            return vec![MachineOutput::LcdChanged];
        }

        if self.state.mode != Mode::Main {
            return Self::ignored(format!(
                "{}.cursor_right_unmapped",
                mode_reason(self.state.mode)
            ));
        }

        self.state.selected_main_field = self.state.selected_main_field.next();
        self.refresh_lcd();
        vec![MachineOutput::LcdChanged]
    }

    fn handle_soft_key(&mut self, index: u8) -> Vec<MachineOutput> {
        if self.state.mode == Mode::Program {
            return self.handle_program_soft_key(index);
        }

        if self.state.mode != Mode::Main {
            return Self::ignored(format!(
                "{}.soft_key.{index}_unmapped",
                mode_reason(self.state.mode)
            ));
        }

        match index {
            2 => {
                self.state.selected_main_field = MainScreenField::Track;
                self.adjust_track(1);
                self.refresh_lcd();
                vec![MachineOutput::LcdChanged]
            }
            3 => {
                self.state.selected_main_field = MainScreenField::Track;
                self.adjust_track(-1);
                self.refresh_lcd();
                vec![MachineOutput::LcdChanged]
            }
            _ => Self::ignored(format!("main_screen.soft_key.{index}_unimplemented")),
        }
    }

    fn handle_program_soft_key(&mut self, index: u8) -> Vec<MachineOutput> {
        match index {
            1 => self.clear_selected_pad_assignment(),
            2 => self.restore_selected_pad_assignment(),
            _ => Self::ignored(format!("program.soft_key.{index}_unimplemented")),
        }
    }

    fn adjust_sequence(&mut self, delta: i32) {
        self.state.sequence_index = clamp_delta_u8(
            self.state.sequence_index,
            delta,
            MIN_SEQUENCE_INDEX,
            MAX_SEQUENCE_INDEX,
        );
        self.state.sequence_name = sequence_name_for(self.state.sequence_index);
    }

    fn adjust_track(&mut self, delta: i32) {
        self.state.selected_track = clamp_delta_u8(
            self.state.selected_track,
            delta,
            MIN_TRACK_INDEX,
            MAX_TRACK_INDEX,
        );
    }

    fn adjust_tempo(&mut self, delta: i32) {
        let current = i64::from(self.state.tempo_bpm_x100);
        let delta = i64::from(delta) * 100;
        let next = (current + delta)
            .clamp(i64::from(MIN_TEMPO_BPM_X100), i64::from(MAX_TEMPO_BPM_X100))
            as u32;
        self.state.tempo_bpm_x100 = next;
    }

    fn adjust_bars(&mut self, delta: i32) {
        self.state.bar_count =
            clamp_delta_u16(self.state.bar_count, delta, MIN_BAR_COUNT, MAX_BAR_COUNT);
    }

    fn refresh_lcd(&mut self) {
        self.state.lcd = match self.state.mode {
            Mode::Main => LcdFrame::main_screen(
                self.state.sequence_index,
                &self.state.sequence_name,
                self.state.selected_track,
                &self.state.current_program.name,
                self.state.tempo_bpm_x100,
                self.state.playing,
                self.state.recording,
                self.state.bar_count,
                self.state.selected_main_field,
                self.state.playhead_ticks,
                self.state.recorded_events.len(),
            ),
            Mode::Program => LcdFrame::program_screen(
                &self.state.current_program,
                self.state.selected_program_pad,
                self.assignment_for(self.state.selected_program_pad),
            ),
            Mode::Sample => LcdFrame::mode_screen("SAMPLE", "Sample record"),
            Mode::Trim => LcdFrame::mode_screen("TRIM", "Trim sample"),
            Mode::Song => LcdFrame::mode_screen("SONG", "Song mode"),
            Mode::Midi => LcdFrame::mode_screen("MIDI", "MIDI sync/settings"),
            Mode::Disk => LcdFrame::mode_screen("DISK", "Virtual disk"),
            Mode::Setup => LcdFrame::mode_screen("SETUP", "System settings"),
        };
    }

    fn ignored(reason: String) -> Vec<MachineOutput> {
        vec![MachineOutput::Ignored { reason }]
    }

    fn handle_pad_strike(&mut self, bank: PadBank, pad: u8, velocity: u8) -> Vec<MachineOutput> {
        let previous_lcd = self.state.lcd.clone();
        self.state.pad_bank = bank;
        if self.state.mode == Mode::Program {
            self.state.selected_program_pad = ProgramPad {
                bank,
                pad_number: pad,
            };
        }

        let mut outputs = vec![MachineOutput::PadTriggered {
            bank,
            pad,
            velocity,
        }];
        let playback = self.resolve_playback(bank, pad, velocity);
        match &playback {
            SamplePlaybackResolution::Intent { intent } => {
                outputs.push(MachineOutput::SamplePlaybackIntent {
                    intent: intent.clone(),
                });
            }
            SamplePlaybackResolution::Miss { miss } => {
                outputs.push(MachineOutput::SamplePlaybackMiss { miss: miss.clone() });
            }
        }
        self.state.last_playback = Some(playback.clone());

        if self.state.playing && self.state.recording {
            let event = SequenceEvent {
                selected_track: self.state.selected_track,
                pad_bank: bank,
                pad_number: pad,
                velocity,
                tick: self.state.playhead_ticks,
                playback: match &playback {
                    SamplePlaybackResolution::Intent { intent } => Some(intent.clone()),
                    SamplePlaybackResolution::Miss { .. } => None,
                },
            };
            self.state.recorded_events.push(event.clone());
            outputs.push(MachineOutput::SequenceEventRecorded { event });
        }

        self.refresh_lcd();
        if self.state.lcd != previous_lcd {
            outputs.push(MachineOutput::LcdChanged);
        }

        outputs
    }

    fn handle_tick(&mut self, micros: u64) -> Vec<MachineOutput> {
        if !self.state.playing {
            return Vec::new();
        }

        let previous_lcd = self.state.lcd.clone();
        let numerator = u128::from(micros)
            .saturating_mul(u128::from(self.state.tempo_bpm_x100))
            .saturating_mul(u128::from(INTERNAL_PPQN))
            .saturating_add(u128::from(self.state.playhead_tick_remainder));
        let tick_delta = numerator / TICK_DENOMINATOR;
        let remainder = numerator % TICK_DENOMINATOR;
        let tick_delta = u64::try_from(tick_delta).unwrap_or(u64::MAX);
        self.state.playhead_ticks = self.state.playhead_ticks.saturating_add(tick_delta);
        self.state.playhead_tick_remainder = if self.state.playhead_ticks == u64::MAX {
            0
        } else {
            remainder as u64
        };

        self.refresh_lcd();
        if self.state.lcd != previous_lcd {
            vec![MachineOutput::LcdChanged]
        } else {
            Vec::new()
        }
    }

    fn assignment_for(&self, pad: ProgramPad) -> Option<&PadAssignment> {
        self.state
            .current_program
            .pad_assignments
            .iter()
            .find(|assignment| assignment.pad == pad)
    }

    fn adjust_selected_program_pad(&mut self, delta: i32) {
        let next = clamp_delta_u8(self.state.selected_program_pad.pad_number, delta, 1, 16);
        self.state.selected_program_pad.pad_number = next;
    }

    fn clear_selected_pad_assignment(&mut self) -> Vec<MachineOutput> {
        let pad = self.state.selected_program_pad;
        self.state
            .current_program
            .pad_assignments
            .retain(|assignment| assignment.pad != pad);
        self.refresh_lcd();
        vec![
            MachineOutput::PadAssignmentChanged {
                bank: pad.bank,
                pad: pad.pad_number,
                action: PadAssignmentChange::Cleared,
                assignment: None,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn restore_selected_pad_assignment(&mut self) -> Vec<MachineOutput> {
        let pad = self.state.selected_program_pad;
        let assignment = generated_assignment(pad);
        self.state
            .current_program
            .pad_assignments
            .retain(|existing| existing.pad != pad);
        self.state
            .current_program
            .pad_assignments
            .push(assignment.clone());
        self.state
            .current_program
            .pad_assignments
            .sort_by_key(|existing| existing.pad);
        self.refresh_lcd();
        vec![
            MachineOutput::PadAssignmentChanged {
                bank: pad.bank,
                pad: pad.pad_number,
                action: PadAssignmentChange::Restored,
                assignment: Some(assignment),
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn resolve_playback(&self, bank: PadBank, pad: u8, velocity: u8) -> SamplePlaybackResolution {
        let program = &self.state.current_program;
        let program_pad = ProgramPad {
            bank,
            pad_number: pad,
        };

        if let Some(assignment) = self.assignment_for(program_pad) {
            return SamplePlaybackResolution::Intent {
                intent: SamplePlaybackIntent {
                    selected_track: self.state.selected_track,
                    program_index: program.index,
                    program_name: program.name.clone(),
                    bank,
                    pad_number: pad,
                    sample_id: assignment.sample.id.clone(),
                    sample_name: assignment.sample.name.clone(),
                    velocity,
                    level: assignment.level,
                    pan: assignment.pan,
                },
            };
        }

        SamplePlaybackResolution::Miss {
            miss: SamplePlaybackMiss {
                selected_track: self.state.selected_track,
                program_index: program.index,
                program_name: program.name.clone(),
                bank,
                pad_number: pad,
                velocity,
                reason: PlaybackMissReason::PadUnassigned,
            },
        }
    }
}

fn sequence_name_for(index: u8) -> String {
    format!("Sequence{index:02}")
}

fn default_program() -> Program {
    let pad_assignments = (1..=16)
        .map(|pad_number| {
            generated_assignment(ProgramPad {
                bank: PadBank::A,
                pad_number,
            })
        })
        .collect();

    Program {
        index: DEFAULT_PROGRAM_INDEX,
        name: DEFAULT_PROGRAM_NAME.to_string(),
        pad_assignments,
    }
}

fn generated_assignment(pad: ProgramPad) -> PadAssignment {
    PadAssignment {
        pad,
        sample: generated_sample(pad),
        level: DEFAULT_PAD_LEVEL,
        pan: DEFAULT_PAD_PAN,
    }
}

fn generated_sample(pad: ProgramPad) -> SyntheticSample {
    SyntheticSample {
        id: format!(
            "synthetic_{}_{:02}",
            pad.bank.label().to_ascii_lowercase(),
            pad.pad_number
        ),
        name: format!("SYN-{}{:02}", pad.bank.label(), pad.pad_number),
    }
}

fn clamp_delta_u8(current: u8, delta: i32, min: u8, max: u8) -> u8 {
    (i64::from(current) + i64::from(delta)).clamp(i64::from(min), i64::from(max)) as u8
}

fn clamp_delta_u16(current: u16, delta: i32, min: u16, max: u16) -> u16 {
    (i64::from(current) + i64::from(delta)).clamp(i64::from(min), i64::from(max)) as u16
}

fn mode_reason(mode: Mode) -> &'static str {
    match mode {
        Mode::Main => "main_screen",
        Mode::Program => "program",
        Mode::Sample => "sample",
        Mode::Trim => "trim",
        Mode::Song => "song",
        Mode::Midi => "midi",
        Mode::Disk => "disk",
        Mode::Setup => "setup",
    }
}
