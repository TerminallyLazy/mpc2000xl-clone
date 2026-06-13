use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt;

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
pub const PROJECT_SNAPSHOT_VERSION: u16 = 1;

const PROJECT_SNAPSHOT_KIND: &str = "mpc2000xl_clone_project";
const PROJECT_RIGHTS_BOUNDARY: &str = "metadata_only_no_audio_bytes";
const MIN_TEMPO_BPM_X100: u32 = 3000;
const MAX_TEMPO_BPM_X100: u32 = 30000;
const MIN_SEQUENCE_INDEX: u8 = 1;
const MAX_SEQUENCE_INDEX: u8 = 99;
const MIN_TRACK_INDEX: u8 = 1;
const MAX_TRACK_INDEX: u8 = 64;
const MIN_PROGRAM_INDEX: u8 = 1;
const MAX_PROGRAM_INDEX: u8 = 128;
const MIN_BAR_COUNT: u16 = 1;
const MAX_BAR_COUNT: u16 = 999;
const TICK_DENOMINATOR: u128 = 60_000_000_u128 * 100;
const DEFAULT_PROGRAM_INDEX: u8 = 1;
const DEFAULT_PROGRAM_NAME: &str = "Program01";
const DEFAULT_PAD_LEVEL: u8 = 100;
const DEFAULT_PAD_PAN: i8 = 0;
const MAX_PAD_LEVEL: u8 = 127;
const MIN_PAD_PAN: i8 = -50;
const MAX_PAD_PAN: i8 = 50;

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

/// Versioned, rights-safe project persistence model.
///
/// The snapshot intentionally contains metadata only: sequence settings,
/// program assignments, recorded event metadata, synthetic sample identifiers,
/// and current UI/playhead position. It does not contain audio bytes, copied
/// assets, firmware data, manuals, service scans, or transport armed/playing
/// state. Restoring a snapshot always leaves transport stopped and disarmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSnapshot {
    pub kind: String,
    pub version: u16,
    pub rights_boundary: String,
    pub machine: ProjectMachineSnapshot,
    pub sequence: ProjectSequenceSnapshot,
    pub program: ProjectProgramSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectMachineSnapshot {
    pub mode: Mode,
    pub selected_main_field: MainScreenField,
    pub pad_bank: PadBank,
    pub selected_program_pad: ProgramPad,
    pub playhead_ticks: u64,
    pub playhead_tick_remainder: u64,
    pub event_count: u64,
    pub last_playback: Option<SamplePlaybackResolution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSequenceSnapshot {
    pub index: u8,
    pub name: String,
    pub tempo_bpm_x100: u32,
    pub selected_track: u8,
    pub bar_count: u16,
    pub recorded_events: Vec<SequenceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectProgramSnapshot {
    pub index: u8,
    pub name: String,
    pub pad_assignments: Vec<PadAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectSnapshotError {
    JsonEncode { message: String },
    JsonDecode { message: String },
    UnsupportedVersion { version: u16, supported: u16 },
    InvalidKind { kind: String },
    InvalidRightsBoundary { rights_boundary: String },
    InvalidValue { field: String, message: String },
    DuplicatePadAssignment { pad: ProgramPad },
}

impl fmt::Display for ProjectSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonEncode { message } => {
                write!(formatter, "project JSON encode failed: {message}")
            }
            Self::JsonDecode { message } => {
                write!(formatter, "project JSON decode failed: {message}")
            }
            Self::UnsupportedVersion { version, supported } => write!(
                formatter,
                "unsupported project snapshot version {version}; supported version is {supported}"
            ),
            Self::InvalidKind { kind } => {
                write!(formatter, "invalid project snapshot kind {kind:?}")
            }
            Self::InvalidRightsBoundary { rights_boundary } => write!(
                formatter,
                "invalid project rights boundary {rights_boundary:?}"
            ),
            Self::InvalidValue { field, message } => {
                write!(
                    formatter,
                    "invalid project snapshot field {field}: {message}"
                )
            }
            Self::DuplicatePadAssignment { pad } => write!(
                formatter,
                "duplicate project pad assignment for {}{:02}",
                pad.bank.label(),
                pad.pad_number
            ),
        }
    }
}

impl std::error::Error for ProjectSnapshotError {}

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

    pub fn export_project_snapshot(&self) -> ProjectSnapshot {
        let mut pad_assignments = self.state.current_program.pad_assignments.clone();
        pad_assignments.sort_by_key(|assignment| assignment.pad);

        ProjectSnapshot {
            kind: PROJECT_SNAPSHOT_KIND.to_string(),
            version: PROJECT_SNAPSHOT_VERSION,
            rights_boundary: PROJECT_RIGHTS_BOUNDARY.to_string(),
            machine: ProjectMachineSnapshot {
                mode: self.state.mode,
                selected_main_field: self.state.selected_main_field,
                pad_bank: self.state.pad_bank,
                selected_program_pad: self.state.selected_program_pad,
                playhead_ticks: self.state.playhead_ticks,
                playhead_tick_remainder: self.state.playhead_tick_remainder,
                event_count: self.state.event_count,
                last_playback: self.state.last_playback.clone(),
            },
            sequence: ProjectSequenceSnapshot {
                index: self.state.sequence_index,
                name: self.state.sequence_name.clone(),
                tempo_bpm_x100: self.state.tempo_bpm_x100,
                selected_track: self.state.selected_track,
                bar_count: self.state.bar_count,
                recorded_events: self.state.recorded_events.clone(),
            },
            program: ProjectProgramSnapshot {
                index: self.state.current_program.index,
                name: self.state.current_program.name.clone(),
                pad_assignments,
            },
        }
    }

    pub fn restore_project_snapshot(
        &mut self,
        snapshot: ProjectSnapshot,
    ) -> Result<(), ProjectSnapshotError> {
        validate_project_snapshot(&snapshot)?;

        let mut pad_assignments = snapshot.program.pad_assignments;
        pad_assignments.sort_by_key(|assignment| assignment.pad);

        self.state.mode = snapshot.machine.mode;
        self.state.sequence_index = snapshot.sequence.index;
        self.state.sequence_name = snapshot.sequence.name;
        self.state.tempo_bpm_x100 = snapshot.sequence.tempo_bpm_x100;
        self.state.playing = false;
        self.state.recording = false;
        self.state.selected_track = snapshot.sequence.selected_track;
        self.state.bar_count = snapshot.sequence.bar_count;
        self.state.selected_main_field = snapshot.machine.selected_main_field;
        self.state.pad_bank = snapshot.machine.pad_bank;
        self.state.current_program = Program {
            index: snapshot.program.index,
            name: snapshot.program.name,
            pad_assignments,
        };
        self.state.selected_program_pad = snapshot.machine.selected_program_pad;
        self.state.last_playback = snapshot.machine.last_playback;
        self.state.playhead_ticks = snapshot.machine.playhead_ticks;
        self.state.playhead_tick_remainder = snapshot.machine.playhead_tick_remainder;
        self.state.recorded_events = snapshot.sequence.recorded_events;
        self.state.event_count = snapshot.machine.event_count;
        self.refresh_lcd();

        Ok(())
    }

    pub fn to_project_json(&self) -> Result<String, ProjectSnapshotError> {
        serde_json::to_string_pretty(&self.export_project_snapshot()).map_err(|error| {
            ProjectSnapshotError::JsonEncode {
                message: error.to_string(),
            }
        })
    }

    pub fn from_project_json(json: &str) -> Result<ProjectSnapshot, ProjectSnapshotError> {
        let value: Value =
            serde_json::from_str(json).map_err(|error| ProjectSnapshotError::JsonDecode {
                message: error.to_string(),
            })?;
        validate_project_snapshot_json_fields(&value)?;
        let snapshot =
            serde_json::from_value(value).map_err(|error| ProjectSnapshotError::JsonDecode {
                message: error.to_string(),
            })?;
        validate_project_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn restore_project_json(&mut self, json: &str) -> Result<(), ProjectSnapshotError> {
        let snapshot = Self::from_project_json(json)?;
        self.restore_project_snapshot(snapshot)
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
        let previous_playhead_ticks = self.state.playhead_ticks;
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

        let scheduled_events = if previous_playhead_ticks < self.state.playhead_ticks {
            self.state
                .recorded_events
                .iter()
                .filter(|event| {
                    previous_playhead_ticks < event.tick && event.tick <= self.state.playhead_ticks
                })
                .filter_map(|event| {
                    event
                        .playback
                        .as_ref()
                        .map(|intent| (event.clone(), intent.clone()))
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let mut outputs = Vec::new();
        for (event, intent) in scheduled_events {
            outputs.push(MachineOutput::SequenceEventPlayed { event });
            outputs.push(MachineOutput::SamplePlaybackIntent {
                intent: intent.clone(),
            });
            self.state.last_playback = Some(SamplePlaybackResolution::Intent { intent });
        }

        self.refresh_lcd();
        if self.state.lcd != previous_lcd {
            outputs.push(MachineOutput::LcdChanged);
        }

        outputs
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

fn validate_project_snapshot_json_fields(value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(root) = reject_unknown_json_fields(
        "",
        value,
        &[
            "kind",
            "version",
            "rights_boundary",
            "machine",
            "sequence",
            "program",
        ],
    )?
    else {
        return Ok(());
    };

    if let Some(machine) = root.get("machine") {
        validate_machine_json_fields(machine)?;
    }
    if let Some(sequence) = root.get("sequence") {
        validate_sequence_json_fields(sequence)?;
    }
    if let Some(program) = root.get("program") {
        validate_program_json_fields(program)?;
    }

    Ok(())
}

fn validate_machine_json_fields(value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(machine) = reject_unknown_json_fields(
        "machine",
        value,
        &[
            "mode",
            "selected_main_field",
            "pad_bank",
            "selected_program_pad",
            "playhead_ticks",
            "playhead_tick_remainder",
            "event_count",
            "last_playback",
        ],
    )?
    else {
        return Ok(());
    };

    if let Some(selected_program_pad) = machine.get("selected_program_pad") {
        validate_program_pad_json_fields("machine.selected_program_pad", selected_program_pad)?;
    }
    if let Some(last_playback) = machine.get("last_playback") {
        validate_playback_resolution_json_fields("machine.last_playback", last_playback)?;
    }

    Ok(())
}

fn validate_sequence_json_fields(value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(sequence) = reject_unknown_json_fields(
        "sequence",
        value,
        &[
            "index",
            "name",
            "tempo_bpm_x100",
            "selected_track",
            "bar_count",
            "recorded_events",
        ],
    )?
    else {
        return Ok(());
    };

    if let Some(recorded_events) = sequence.get("recorded_events").and_then(Value::as_array) {
        for (index, event) in recorded_events.iter().enumerate() {
            validate_sequence_event_json_fields(
                &format!("sequence.recorded_events[{index}]"),
                event,
            )?;
        }
    }

    Ok(())
}

fn validate_program_json_fields(value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(program) =
        reject_unknown_json_fields("program", value, &["index", "name", "pad_assignments"])?
    else {
        return Ok(());
    };

    if let Some(pad_assignments) = program.get("pad_assignments").and_then(Value::as_array) {
        for (index, assignment) in pad_assignments.iter().enumerate() {
            validate_assignment_json_fields(
                &format!("program.pad_assignments[{index}]"),
                assignment,
            )?;
        }
    }

    Ok(())
}

fn validate_assignment_json_fields(field: &str, value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(assignment) =
        reject_unknown_json_fields(field, value, &["pad", "sample", "level", "pan"])?
    else {
        return Ok(());
    };

    if let Some(pad) = assignment.get("pad") {
        validate_program_pad_json_fields(&format!("{field}.pad"), pad)?;
    }
    if let Some(sample) = assignment.get("sample") {
        validate_sample_json_fields(&format!("{field}.sample"), sample)?;
    }

    Ok(())
}

fn validate_sample_json_fields(field: &str, value: &Value) -> Result<(), ProjectSnapshotError> {
    reject_unknown_json_fields(field, value, &["id", "name"])?;
    Ok(())
}

fn validate_sequence_event_json_fields(
    field: &str,
    value: &Value,
) -> Result<(), ProjectSnapshotError> {
    let Some(event) = reject_unknown_json_fields(
        field,
        value,
        &[
            "selected_track",
            "pad_bank",
            "pad_number",
            "velocity",
            "tick",
            "playback",
        ],
    )?
    else {
        return Ok(());
    };

    if let Some(playback) = event.get("playback") {
        validate_playback_intent_json_fields(&format!("{field}.playback"), playback)?;
    }

    Ok(())
}

fn validate_playback_resolution_json_fields(
    field: &str,
    value: &Value,
) -> Result<(), ProjectSnapshotError> {
    if value.is_null() {
        return Ok(());
    }

    let Some(playback) = value.as_object() else {
        return Ok(());
    };
    let allowed_fields = match playback.get("type").and_then(Value::as_str) {
        Some("intent") => &["type", "intent"][..],
        Some("miss") => &["type", "miss"][..],
        _ => &["type", "intent", "miss"][..],
    };
    reject_unknown_keys(field, playback, allowed_fields)?;

    if let Some(intent) = playback.get("intent") {
        validate_playback_intent_json_fields(&format!("{field}.intent"), intent)?;
    }
    if let Some(miss) = playback.get("miss") {
        validate_playback_miss_json_fields(&format!("{field}.miss"), miss)?;
    }

    Ok(())
}

fn validate_playback_intent_json_fields(
    field: &str,
    value: &Value,
) -> Result<(), ProjectSnapshotError> {
    reject_unknown_json_fields(
        field,
        value,
        &[
            "selected_track",
            "program_index",
            "program_name",
            "bank",
            "pad_number",
            "sample_id",
            "sample_name",
            "velocity",
            "level",
            "pan",
        ],
    )?;
    Ok(())
}

fn validate_playback_miss_json_fields(
    field: &str,
    value: &Value,
) -> Result<(), ProjectSnapshotError> {
    reject_unknown_json_fields(
        field,
        value,
        &[
            "selected_track",
            "program_index",
            "program_name",
            "bank",
            "pad_number",
            "velocity",
            "reason",
        ],
    )?;
    Ok(())
}

fn validate_program_pad_json_fields(
    field: &str,
    value: &Value,
) -> Result<(), ProjectSnapshotError> {
    reject_unknown_json_fields(field, value, &["bank", "pad_number"])?;
    Ok(())
}

fn reject_unknown_json_fields<'a>(
    field: &str,
    value: &'a Value,
    allowed: &[&str],
) -> Result<Option<&'a Map<String, Value>>, ProjectSnapshotError> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    reject_unknown_keys(field, object, allowed)?;
    Ok(Some(object))
}

fn reject_unknown_keys(
    field: &str,
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), ProjectSnapshotError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(unknown_json_field(field, key));
        }
    }
    Ok(())
}

fn unknown_json_field(parent: &str, field: &str) -> ProjectSnapshotError {
    let field = if parent.is_empty() {
        field.to_string()
    } else {
        format!("{parent}.{field}")
    };
    invalid_value(&field, "unknown field is not allowed")
}

fn validate_project_snapshot(snapshot: &ProjectSnapshot) -> Result<(), ProjectSnapshotError> {
    if snapshot.kind != PROJECT_SNAPSHOT_KIND {
        return Err(ProjectSnapshotError::InvalidKind {
            kind: snapshot.kind.clone(),
        });
    }
    if snapshot.version != PROJECT_SNAPSHOT_VERSION {
        return Err(ProjectSnapshotError::UnsupportedVersion {
            version: snapshot.version,
            supported: PROJECT_SNAPSHOT_VERSION,
        });
    }
    if snapshot.rights_boundary != PROJECT_RIGHTS_BOUNDARY {
        return Err(ProjectSnapshotError::InvalidRightsBoundary {
            rights_boundary: snapshot.rights_boundary.clone(),
        });
    }

    validate_range_u8(
        "sequence.index",
        snapshot.sequence.index,
        MIN_SEQUENCE_INDEX,
        MAX_SEQUENCE_INDEX,
    )?;
    validate_non_empty("sequence.name", &snapshot.sequence.name)?;
    validate_range_u32(
        "sequence.tempo_bpm_x100",
        snapshot.sequence.tempo_bpm_x100,
        MIN_TEMPO_BPM_X100,
        MAX_TEMPO_BPM_X100,
    )?;
    validate_range_u8(
        "sequence.selected_track",
        snapshot.sequence.selected_track,
        MIN_TRACK_INDEX,
        MAX_TRACK_INDEX,
    )?;
    validate_range_u16(
        "sequence.bar_count",
        snapshot.sequence.bar_count,
        MIN_BAR_COUNT,
        MAX_BAR_COUNT,
    )?;

    validate_range_u8(
        "program.index",
        snapshot.program.index,
        MIN_PROGRAM_INDEX,
        MAX_PROGRAM_INDEX,
    )?;
    validate_non_empty("program.name", &snapshot.program.name)?;
    validate_program_pad(
        "machine.selected_program_pad",
        snapshot.machine.selected_program_pad,
    )?;
    validate_playhead_remainder(snapshot.machine.playhead_tick_remainder)?;
    if snapshot.machine.playhead_ticks == u64::MAX && snapshot.machine.playhead_tick_remainder != 0
    {
        return Err(invalid_value(
            "machine.playhead_tick_remainder",
            "must be 0 when machine.playhead_ticks is u64::MAX",
        ));
    }

    let recorded_event_count =
        u64::try_from(snapshot.sequence.recorded_events.len()).unwrap_or(u64::MAX);
    if snapshot.machine.event_count < recorded_event_count {
        return Err(invalid_value(
            "machine.event_count",
            format!("must be >= sequence.recorded_events.len() {recorded_event_count}"),
        ));
    }
    if snapshot.machine.last_playback.is_some() && snapshot.machine.event_count == 0 {
        return Err(invalid_value(
            "machine.last_playback",
            "requires machine.event_count > 0",
        ));
    }

    validate_playback_resolution(
        "machine.last_playback",
        snapshot.machine.last_playback.as_ref(),
    )?;

    let mut seen_assignments = BTreeSet::new();
    for (index, assignment) in snapshot.program.pad_assignments.iter().enumerate() {
        let field = format!("program.pad_assignments[{index}]");
        validate_assignment(&field, assignment)?;
        if !seen_assignments.insert(assignment.pad) {
            return Err(ProjectSnapshotError::DuplicatePadAssignment {
                pad: assignment.pad,
            });
        }
    }

    for (index, event) in snapshot.sequence.recorded_events.iter().enumerate() {
        validate_sequence_event(&format!("sequence.recorded_events[{index}]"), event)?;
    }

    Ok(())
}

fn validate_assignment(
    field: &str,
    assignment: &PadAssignment,
) -> Result<(), ProjectSnapshotError> {
    validate_program_pad(&format!("{field}.pad"), assignment.pad)?;
    validate_non_empty(&format!("{field}.sample.id"), &assignment.sample.id)?;
    validate_non_empty(&format!("{field}.sample.name"), &assignment.sample.name)?;
    validate_range_u8(
        &format!("{field}.level"),
        assignment.level,
        0,
        MAX_PAD_LEVEL,
    )?;
    validate_range_i8(
        &format!("{field}.pan"),
        assignment.pan,
        MIN_PAD_PAN,
        MAX_PAD_PAN,
    )
}

fn validate_sequence_event(field: &str, event: &SequenceEvent) -> Result<(), ProjectSnapshotError> {
    validate_range_u8(
        &format!("{field}.selected_track"),
        event.selected_track,
        MIN_TRACK_INDEX,
        MAX_TRACK_INDEX,
    )?;
    validate_pad_number(&format!("{field}.pad_number"), event.pad_number)?;
    validate_velocity(&format!("{field}.velocity"), event.velocity)?;

    if let Some(playback) = &event.playback {
        validate_playback_intent(&format!("{field}.playback"), playback)?;
        if playback.selected_track != event.selected_track {
            return Err(invalid_value(
                &format!("{field}.playback.selected_track"),
                "must match recorded event selected_track",
            ));
        }
        if playback.bank != event.pad_bank {
            return Err(invalid_value(
                &format!("{field}.playback.bank"),
                "must match recorded event pad_bank",
            ));
        }
        if playback.pad_number != event.pad_number {
            return Err(invalid_value(
                &format!("{field}.playback.pad_number"),
                "must match recorded event pad_number",
            ));
        }
        if playback.velocity != event.velocity {
            return Err(invalid_value(
                &format!("{field}.playback.velocity"),
                "must match recorded event velocity",
            ));
        }
    }

    Ok(())
}

fn validate_playback_resolution(
    field: &str,
    playback: Option<&SamplePlaybackResolution>,
) -> Result<(), ProjectSnapshotError> {
    match playback {
        Some(SamplePlaybackResolution::Intent { intent }) => {
            validate_playback_intent(&format!("{field}.intent"), intent)
        }
        Some(SamplePlaybackResolution::Miss { miss }) => {
            validate_playback_miss(&format!("{field}.miss"), miss)
        }
        None => Ok(()),
    }
}

fn validate_playback_intent(
    field: &str,
    intent: &SamplePlaybackIntent,
) -> Result<(), ProjectSnapshotError> {
    validate_range_u8(
        &format!("{field}.selected_track"),
        intent.selected_track,
        MIN_TRACK_INDEX,
        MAX_TRACK_INDEX,
    )?;
    validate_range_u8(
        &format!("{field}.program_index"),
        intent.program_index,
        MIN_PROGRAM_INDEX,
        MAX_PROGRAM_INDEX,
    )?;
    validate_non_empty(&format!("{field}.program_name"), &intent.program_name)?;
    validate_pad_number(&format!("{field}.pad_number"), intent.pad_number)?;
    validate_non_empty(&format!("{field}.sample_id"), &intent.sample_id)?;
    validate_non_empty(&format!("{field}.sample_name"), &intent.sample_name)?;
    validate_velocity(&format!("{field}.velocity"), intent.velocity)?;
    validate_range_u8(&format!("{field}.level"), intent.level, 0, MAX_PAD_LEVEL)?;
    validate_range_i8(
        &format!("{field}.pan"),
        intent.pan,
        MIN_PAD_PAN,
        MAX_PAD_PAN,
    )
}

fn validate_playback_miss(
    field: &str,
    miss: &SamplePlaybackMiss,
) -> Result<(), ProjectSnapshotError> {
    validate_range_u8(
        &format!("{field}.selected_track"),
        miss.selected_track,
        MIN_TRACK_INDEX,
        MAX_TRACK_INDEX,
    )?;
    validate_range_u8(
        &format!("{field}.program_index"),
        miss.program_index,
        MIN_PROGRAM_INDEX,
        MAX_PROGRAM_INDEX,
    )?;
    validate_non_empty(&format!("{field}.program_name"), &miss.program_name)?;
    validate_pad_number(&format!("{field}.pad_number"), miss.pad_number)?;
    validate_velocity(&format!("{field}.velocity"), miss.velocity)
}

fn validate_program_pad(field: &str, pad: ProgramPad) -> Result<(), ProjectSnapshotError> {
    validate_pad_number(&format!("{field}.pad_number"), pad.pad_number)
}

fn validate_pad_number(field: &str, pad_number: u8) -> Result<(), ProjectSnapshotError> {
    validate_range_u8(field, pad_number, 1, 16)
}

fn validate_velocity(field: &str, velocity: u8) -> Result<(), ProjectSnapshotError> {
    validate_range_u8(field, velocity, 1, 127)
}

fn validate_playhead_remainder(remainder: u64) -> Result<(), ProjectSnapshotError> {
    if u128::from(remainder) >= TICK_DENOMINATOR {
        return Err(invalid_value(
            "machine.playhead_tick_remainder",
            format!("must be less than {TICK_DENOMINATOR}"),
        ));
    }
    Ok(())
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), ProjectSnapshotError> {
    if value.trim().is_empty() {
        return Err(invalid_value(field, "must not be empty"));
    }
    Ok(())
}

fn validate_range_u8(field: &str, value: u8, min: u8, max: u8) -> Result<(), ProjectSnapshotError> {
    if value < min || value > max {
        return Err(invalid_value(
            field,
            format!("must be in range {min}..={max}"),
        ));
    }
    Ok(())
}

fn validate_range_u16(
    field: &str,
    value: u16,
    min: u16,
    max: u16,
) -> Result<(), ProjectSnapshotError> {
    if value < min || value > max {
        return Err(invalid_value(
            field,
            format!("must be in range {min}..={max}"),
        ));
    }
    Ok(())
}

fn validate_range_u32(
    field: &str,
    value: u32,
    min: u32,
    max: u32,
) -> Result<(), ProjectSnapshotError> {
    if value < min || value > max {
        return Err(invalid_value(
            field,
            format!("must be in range {min}..={max}"),
        ));
    }
    Ok(())
}

fn validate_range_i8(field: &str, value: i8, min: i8, max: i8) -> Result<(), ProjectSnapshotError> {
    if value < min || value > max {
        return Err(invalid_value(
            field,
            format!("must be in range {min}..={max}"),
        ));
    }
    Ok(())
}

fn invalid_value(field: &str, message: impl Into<String>) -> ProjectSnapshotError {
    ProjectSnapshotError::InvalidValue {
        field: field.to_string(),
        message: message.into(),
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
