use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt;

use crate::events::{
    DiskOperation, HardwareEvent, MachineOutput, MidiSettingsField, Mode, PadAssignment,
    PadAssignmentChange, PadBank, PanelControl, PlaybackMissReason, Program, ProgramEditField,
    ProgramPad, SampleCatalogEntry, SamplePlaybackIntent, SamplePlaybackMiss,
    SamplePlaybackResolution, SampleTrim, SequenceEvent, SetupField, SetupPreferences,
    SongEditField, SongStep, SyntheticSample, TimingCorrectField, TimingCorrectSettings,
    TrimEditField, generated_sample_length_frames, sample_window_length_frames,
};
use crate::lcd::LcdFrame;

/// Internal timing resolution for this foundation slice.
///
/// The exact MPC2000XL timing source mapping is still pending reference
/// evidence, so sequence recording uses a deterministic 96 PPQN basis for now.
pub const INTERNAL_PPQN: u32 = 96;
pub const FOUNDATION_BEATS_PER_BAR: u32 = 4;
pub const PROJECT_SNAPSHOT_VERSION: u16 = 1;

const PROJECT_SNAPSHOT_KIND: &str = "mpc2000xl_clone_project";
const PROJECT_RIGHTS_BOUNDARY: &str = "metadata_only_no_audio_bytes";
const MIN_TEMPO_BPM_X100: u32 = 3000;
const MAX_TEMPO_BPM_X100: u32 = 30000;
const MIN_SEQUENCE_INDEX: u8 = 1;
const MAX_SEQUENCE_INDEX: u8 = 99;
const MIN_SONG_SEQUENCE_INDEX: u8 = 0;
const MAX_SONG_SEQUENCE_INDEX: u8 = 98;
const MIN_SONG_REPEATS: u8 = 1;
const MAX_SONG_REPEATS: u8 = 99;
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
const DEFAULT_PAD_TUNE_CENTS: i16 = 0;
const MAX_PAD_LEVEL: u8 = 127;
const MIN_PAD_PAN: i8 = -50;
const MAX_PAD_PAN: i8 = 50;
const MIN_PAD_TUNE_CENTS: i16 = -1200;
const MAX_PAD_TUNE_CENTS: i16 = 1200;
const MIDI_MIN_CHANNEL: u8 = 1;
const MIDI_MAX_CHANNEL: u8 = 16;
const MIDI_MAX_NOTE: u8 = 127;
const DEFAULT_MIDI_BASE_NOTE: u8 = 36;
const MAX_MIDI_BASE_NOTE: u8 = 112;
const MIDI_MAPPED_PAD_COUNT: u8 = 16;
const MIN_SETUP_COUNT_IN_BARS: u8 = 0;
const MAX_SETUP_COUNT_IN_BARS: u8 = 4;
const MIN_SETUP_LCD_CONTRAST: u8 = 0;
const MAX_SETUP_LCD_CONTRAST: u8 = 10;
const MIN_TIMING_CORRECT_SWING_PERCENT: u8 = 50;
const MAX_TIMING_CORRECT_SWING_PERCENT: u8 = 75;
const PAD_BANKS: [PadBank; 4] = [PadBank::A, PadBank::B, PadBank::C, PadBank::D];

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
/// song-chain metadata, and current UI/playhead position. It does not contain
/// audio bytes, copied assets, firmware data, manuals, service scans, or
/// transport armed/playing state. Restoring a snapshot always leaves transport
/// stopped and disarmed. Version 1 remains additive for foundation metadata:
/// omitted newer objects restore deterministic defaults, while malformed
/// present objects are rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSnapshot {
    pub kind: String,
    pub version: u16,
    pub rights_boundary: String,
    pub machine: ProjectMachineSnapshot,
    pub sequence: ProjectSequenceSnapshot,
    pub program: ProjectProgramSnapshot,
    #[serde(default)]
    pub song: ProjectSongSnapshot,
    #[serde(default)]
    pub setup: ProjectSetupSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectMachineSnapshot {
    pub mode: Mode,
    pub selected_main_field: MainScreenField,
    pub pad_bank: PadBank,
    pub selected_program_pad: ProgramPad,
    #[serde(default)]
    pub selected_program_edit_field: ProgramEditField,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_sample_id: Option<String>,
    #[serde(default)]
    pub selected_trim_edit_field: TrimEditField,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_input_channel: Option<u8>,
    #[serde(default = "default_midi_base_note")]
    pub midi_base_note: u8,
    #[serde(default)]
    pub selected_midi_settings_field: MidiSettingsField,
    #[serde(default)]
    pub selected_disk_operation: DiskOperation,
    #[serde(default)]
    pub selected_timing_correct_field: TimingCorrectField,
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
    #[serde(default)]
    pub muted_tracks: Vec<u8>,
    pub bar_count: u16,
    #[serde(default)]
    pub loop_enabled: bool,
    #[serde(default)]
    pub timing_correct: TimingCorrectSettings,
    pub recorded_events: Vec<SequenceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectProgramSnapshot {
    pub index: u8,
    pub name: String,
    pub pad_assignments: Vec<PadAssignment>,
    #[serde(default)]
    pub sample_trims: Vec<SampleTrim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSongSnapshot {
    pub steps: Vec<SongStep>,
    pub selected_step_index: usize,
    pub selected_field: SongEditField,
}

impl Default for ProjectSongSnapshot {
    fn default() -> Self {
        Self {
            steps: default_song_steps(),
            selected_step_index: 0,
            selected_field: SongEditField::Step,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSetupSnapshot {
    pub preferences: SetupPreferences,
    pub selected_field: SetupField,
}

impl Default for ProjectSetupSnapshot {
    fn default() -> Self {
        Self {
            preferences: SetupPreferences::default(),
            selected_field: SetupField::default(),
        }
    }
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
    pub loop_enabled: bool,
    pub selected_track: u8,
    #[serde(default)]
    pub muted_tracks: Vec<u8>,
    pub bar_count: u16,
    pub selected_main_field: MainScreenField,
    pub pad_bank: PadBank,
    pub current_program: Program,
    pub selected_program_pad: ProgramPad,
    pub selected_program_edit_field: ProgramEditField,
    pub selected_sample_id: Option<String>,
    pub selected_trim_edit_field: TrimEditField,
    pub sample_trims: Vec<SampleTrim>,
    pub midi_input_channel: Option<u8>,
    pub midi_base_note: u8,
    pub selected_midi_settings_field: MidiSettingsField,
    pub selected_disk_operation: DiskOperation,
    pub timing_correct: TimingCorrectSettings,
    pub selected_timing_correct_field: TimingCorrectField,
    pub setup_preferences: SetupPreferences,
    pub selected_setup_field: SetupField,
    pub song_steps: Vec<SongStep>,
    pub selected_song_step_index: usize,
    pub selected_song_edit_field: SongEditField,
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
        let selected_sample_id = normalized_sample_id(&current_program, None);

        Self {
            mode: Mode::Main,
            lcd: LcdFrame::main_screen(
                sequence_index,
                &sequence_name,
                selected_track,
                false,
                0,
                &current_program.name,
                tempo_bpm_x100,
                false,
                false,
                false,
                bar_count,
                sequence_length_ticks_for_bars(bar_count),
                selected_main_field,
                0,
                0,
            ),
            sequence_index,
            sequence_name,
            tempo_bpm_x100,
            playing: false,
            recording: false,
            loop_enabled: false,
            selected_track,
            muted_tracks: Vec::new(),
            bar_count,
            selected_main_field,
            pad_bank: PadBank::A,
            current_program,
            selected_program_pad,
            selected_program_edit_field: ProgramEditField::Pad,
            selected_sample_id,
            selected_trim_edit_field: TrimEditField::Start,
            sample_trims: Vec::new(),
            midi_input_channel: None,
            midi_base_note: DEFAULT_MIDI_BASE_NOTE,
            selected_midi_settings_field: MidiSettingsField::InputChannel,
            selected_disk_operation: DiskOperation::SaveProject,
            timing_correct: TimingCorrectSettings::default(),
            selected_timing_correct_field: TimingCorrectField::Division,
            setup_preferences: SetupPreferences::default(),
            selected_setup_field: SetupField::Metronome,
            song_steps: default_song_steps(),
            selected_song_step_index: 0,
            selected_song_edit_field: SongEditField::Step,
            last_playback: None,
            playhead_ticks: 0,
            playhead_tick_remainder: 0,
            recorded_events: Vec::new(),
            event_count: 0,
        }
    }
}

impl MpcState {
    pub fn sequence_length_ticks(&self) -> u64 {
        sequence_length_ticks_for_bars(self.bar_count)
    }

    pub fn sample_catalog(&self) -> Vec<SampleCatalogEntry> {
        sample_catalog_for_program(&self.current_program, &self.sample_trims)
    }

    pub fn selected_sample(&self) -> Option<SampleCatalogEntry> {
        let catalog = self.sample_catalog();
        selected_sample_entry(&catalog, self.selected_sample_id.as_deref()).cloned()
    }

    pub fn is_track_muted(&self, track: u8) -> bool {
        self.muted_tracks.contains(&track)
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
                selected_program_edit_field: self.state.selected_program_edit_field,
                selected_sample_id: self.state.selected_sample_id.clone(),
                selected_trim_edit_field: self.state.selected_trim_edit_field,
                midi_input_channel: self.state.midi_input_channel,
                midi_base_note: self.state.midi_base_note,
                selected_midi_settings_field: self.state.selected_midi_settings_field,
                selected_disk_operation: self.state.selected_disk_operation,
                selected_timing_correct_field: self.state.selected_timing_correct_field,
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
                muted_tracks: self.state.muted_tracks.clone(),
                bar_count: self.state.bar_count,
                loop_enabled: self.state.loop_enabled,
                timing_correct: self.state.timing_correct,
                recorded_events: self.state.recorded_events.clone(),
            },
            program: ProjectProgramSnapshot {
                index: self.state.current_program.index,
                name: self.state.current_program.name.clone(),
                pad_assignments,
                sample_trims: normalized_sample_trims(
                    &self.state.current_program,
                    &self.state.sample_trims,
                ),
            },
            song: ProjectSongSnapshot {
                steps: self.state.song_steps.clone(),
                selected_step_index: self.state.selected_song_step_index,
                selected_field: self.state.selected_song_edit_field,
            },
            setup: ProjectSetupSnapshot {
                preferences: self.state.setup_preferences,
                selected_field: self.state.selected_setup_field,
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
        self.state.loop_enabled = snapshot.sequence.loop_enabled;
        self.state.selected_track = snapshot.sequence.selected_track;
        self.state.muted_tracks = sorted_tracks(snapshot.sequence.muted_tracks);
        self.state.bar_count = snapshot.sequence.bar_count;
        self.state.selected_main_field = snapshot.machine.selected_main_field;
        self.state.pad_bank = snapshot.machine.pad_bank;
        self.state.current_program = Program {
            index: snapshot.program.index,
            name: snapshot.program.name,
            pad_assignments,
        };
        self.state.selected_program_pad = snapshot.machine.selected_program_pad;
        self.state.selected_program_edit_field = snapshot.machine.selected_program_edit_field;
        self.state.selected_sample_id = normalized_sample_id(
            &self.state.current_program,
            snapshot.machine.selected_sample_id.as_deref(),
        );
        self.state.selected_trim_edit_field = snapshot.machine.selected_trim_edit_field;
        self.state.sample_trims =
            normalized_sample_trims(&self.state.current_program, &snapshot.program.sample_trims);
        self.state.midi_input_channel = snapshot.machine.midi_input_channel;
        self.state.midi_base_note = snapshot.machine.midi_base_note;
        self.state.selected_midi_settings_field = snapshot.machine.selected_midi_settings_field;
        self.state.selected_disk_operation = snapshot.machine.selected_disk_operation;
        self.state.timing_correct = snapshot.sequence.timing_correct;
        self.state.selected_timing_correct_field = snapshot.machine.selected_timing_correct_field;
        self.state.setup_preferences = snapshot.setup.preferences;
        self.state.selected_setup_field = snapshot.setup.selected_field;
        self.state.song_steps = snapshot.song.steps;
        self.state.selected_song_step_index = snapshot.song.selected_step_index;
        self.state.selected_song_edit_field = snapshot.song.selected_field;
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
            HardwareEvent::MidiNoteOn {
                channel,
                note,
                velocity,
            } => self.handle_midi_note_on(channel, note, velocity),
            HardwareEvent::MidiNoteOff {
                channel,
                note,
                velocity,
            } => self.handle_midi_note_off(channel, note, velocity),
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
            PanelControl::TimingCorrect => self.set_mode(Mode::TimingCorrect),
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
            PanelControl::LocateStart => self.locate_start(),
            PanelControl::ToggleLoop => self.toggle_loop(),
            PanelControl::PadBankA => self.select_pad_bank(PadBank::A),
            PanelControl::PadBankB => self.select_pad_bank(PadBank::B),
            PanelControl::PadBankC => self.select_pad_bank(PadBank::C),
            PanelControl::PadBankD => self.select_pad_bank(PadBank::D),
            PanelControl::CursorLeft => self.move_main_field_left(),
            PanelControl::CursorRight => self.move_main_field_right(),
            PanelControl::CursorUp => self.move_program_edit_field_up(),
            PanelControl::CursorDown => self.move_program_edit_field_down(),
            PanelControl::SoftKey(index) => self.handle_soft_key(index),
            PanelControl::Numeric(_) => Self::ignored(format!(
                "{}.{control:?}_unimplemented",
                mode_reason(self.state.mode)
            )),
        }
    }

    fn set_mode(&mut self, mode: Mode) -> Vec<MachineOutput> {
        self.state.mode = mode;
        if matches!(mode, Mode::Sample | Mode::Trim) {
            self.ensure_selected_sample_selection();
        }
        self.refresh_lcd();
        vec![
            MachineOutput::ModeChanged { mode },
            MachineOutput::LcdChanged,
        ]
    }

    fn locate_start(&mut self) -> Vec<MachineOutput> {
        self.state.playhead_ticks = 0;
        self.state.playhead_tick_remainder = 0;
        self.refresh_lcd();
        vec![
            MachineOutput::PlayheadLocated { tick: 0 },
            MachineOutput::LcdChanged,
        ]
    }

    fn toggle_loop(&mut self) -> Vec<MachineOutput> {
        self.state.loop_enabled = !self.state.loop_enabled;
        self.refresh_lcd();
        vec![
            MachineOutput::LoopChanged {
                enabled: self.state.loop_enabled,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn select_pad_bank(&mut self, bank: PadBank) -> Vec<MachineOutput> {
        self.state.pad_bank = bank;
        self.state.selected_program_pad.bank = bank;
        self.refresh_lcd();
        vec![
            MachineOutput::BankChanged { bank },
            MachineOutput::LcdChanged,
        ]
    }

    fn handle_data_wheel(&mut self, delta: i32) -> Vec<MachineOutput> {
        if self.state.mode == Mode::Program {
            return self.adjust_selected_program_field(delta);
        }

        if self.state.mode == Mode::Midi {
            return self.adjust_midi_settings(delta);
        }

        if self.state.mode == Mode::TimingCorrect {
            return self.adjust_timing_correct(delta);
        }

        if self.state.mode == Mode::Disk {
            return self.adjust_disk_operation(delta);
        }

        if self.state.mode == Mode::Setup {
            return self.adjust_setup_preferences(delta);
        }

        if self.state.mode == Mode::Song {
            return self.adjust_song_field(delta);
        }

        if self.state.mode == Mode::Sample {
            return self.adjust_selected_sample(delta);
        }

        if self.state.mode == Mode::Trim {
            return self.adjust_selected_sample_trim(delta);
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

        if self.state.mode == Mode::Midi {
            self.state.selected_midi_settings_field =
                self.state.selected_midi_settings_field.previous();
            self.refresh_lcd();
            return vec![
                MachineOutput::MidiSettingsChanged {
                    input_channel: self.state.midi_input_channel,
                    base_note: self.state.midi_base_note,
                    selected_field: self.state.selected_midi_settings_field,
                },
                MachineOutput::LcdChanged,
            ];
        }

        if self.state.mode == Mode::TimingCorrect {
            self.state.selected_timing_correct_field =
                self.state.selected_timing_correct_field.previous();
            self.refresh_lcd();
            return self.timing_correct_changed_outputs();
        }

        if self.state.mode == Mode::Disk {
            return self.select_disk_operation(self.state.selected_disk_operation.previous());
        }

        if self.state.mode == Mode::Song {
            self.state.selected_song_edit_field = self.state.selected_song_edit_field.previous();
            self.refresh_lcd();
            return vec![MachineOutput::LcdChanged];
        }

        if self.state.mode == Mode::Trim {
            self.state.selected_trim_edit_field = self.state.selected_trim_edit_field.previous();
            self.refresh_lcd();
            return vec![MachineOutput::LcdChanged];
        }

        if self.state.mode == Mode::Setup {
            return self.select_setup_field(self.state.selected_setup_field.previous());
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

        if self.state.mode == Mode::Midi {
            self.state.selected_midi_settings_field =
                self.state.selected_midi_settings_field.next();
            self.refresh_lcd();
            return vec![
                MachineOutput::MidiSettingsChanged {
                    input_channel: self.state.midi_input_channel,
                    base_note: self.state.midi_base_note,
                    selected_field: self.state.selected_midi_settings_field,
                },
                MachineOutput::LcdChanged,
            ];
        }

        if self.state.mode == Mode::TimingCorrect {
            self.state.selected_timing_correct_field =
                self.state.selected_timing_correct_field.next();
            self.refresh_lcd();
            return self.timing_correct_changed_outputs();
        }

        if self.state.mode == Mode::Disk {
            return self.select_disk_operation(self.state.selected_disk_operation.next());
        }

        if self.state.mode == Mode::Song {
            self.state.selected_song_edit_field = self.state.selected_song_edit_field.next();
            self.refresh_lcd();
            return vec![MachineOutput::LcdChanged];
        }

        if self.state.mode == Mode::Trim {
            self.state.selected_trim_edit_field = self.state.selected_trim_edit_field.next();
            self.refresh_lcd();
            return vec![MachineOutput::LcdChanged];
        }

        if self.state.mode == Mode::Setup {
            return self.select_setup_field(self.state.selected_setup_field.next());
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

    fn move_program_edit_field_up(&mut self) -> Vec<MachineOutput> {
        if self.state.mode == Mode::Song {
            return self.select_song_step_by_delta(-1, "song.step.previous_unavailable");
        }

        if self.state.mode != Mode::Program {
            return Self::ignored(format!(
                "{}.{:?}_unimplemented",
                mode_reason(self.state.mode),
                PanelControl::CursorUp
            ));
        }

        self.state.selected_program_edit_field = self.state.selected_program_edit_field.previous();
        self.refresh_lcd();
        vec![MachineOutput::LcdChanged]
    }

    fn move_program_edit_field_down(&mut self) -> Vec<MachineOutput> {
        if self.state.mode == Mode::Song {
            return self.select_song_step_by_delta(1, "song.step.next_unavailable");
        }

        if self.state.mode != Mode::Program {
            return Self::ignored(format!(
                "{}.{:?}_unimplemented",
                mode_reason(self.state.mode),
                PanelControl::CursorDown
            ));
        }

        self.state.selected_program_edit_field = self.state.selected_program_edit_field.next();
        self.refresh_lcd();
        vec![MachineOutput::LcdChanged]
    }

    fn handle_soft_key(&mut self, index: u8) -> Vec<MachineOutput> {
        if self.state.mode == Mode::Program {
            return self.handle_program_soft_key(index);
        }

        if matches!(self.state.mode, Mode::Sample | Mode::Trim) {
            return self.handle_sample_soft_key(index);
        }

        if self.state.mode == Mode::Disk {
            return self.handle_disk_soft_key(index);
        }

        if self.state.mode == Mode::Song {
            return self.handle_song_soft_key(index);
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
            4 => self.toggle_selected_track_mute(),
            5 => self.erase_latest_sequence_event_on_selected_track(),
            _ => Self::ignored(format!("main_screen.soft_key.{index}_unimplemented")),
        }
    }

    fn toggle_selected_track_mute(&mut self) -> Vec<MachineOutput> {
        let track = self.state.selected_track;
        let muted = match self.state.muted_tracks.binary_search(&track) {
            Ok(index) => {
                self.state.muted_tracks.remove(index);
                false
            }
            Err(index) => {
                self.state.muted_tracks.insert(index, track);
                true
            }
        };

        self.refresh_lcd();
        vec![
            MachineOutput::TrackMuteChanged {
                track,
                muted,
                muted_tracks: self.state.muted_tracks.clone(),
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn erase_latest_sequence_event_on_selected_track(&mut self) -> Vec<MachineOutput> {
        let selected_track = self.state.selected_track;
        let Some(index) = self
            .state
            .recorded_events
            .iter()
            .rposition(|event| event.selected_track == selected_track)
        else {
            return Self::ignored(format!("sequence.erase.track_{selected_track}.no_events"));
        };

        let event = self.state.recorded_events.remove(index);
        self.refresh_lcd();
        vec![
            MachineOutput::SequenceEventsErased {
                selected_track,
                count: 1,
                events: vec![event],
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn handle_program_soft_key(&mut self, index: u8) -> Vec<MachineOutput> {
        match index {
            1 => self.clear_selected_pad_assignment(),
            2 => self.restore_selected_pad_assignment(),
            _ => Self::ignored(format!("program.soft_key.{index}_unimplemented")),
        }
    }

    fn handle_sample_soft_key(&mut self, index: u8) -> Vec<MachineOutput> {
        match index {
            1 => self.adjust_selected_sample(-1),
            2 => self.adjust_selected_sample(1),
            6 if self.state.mode == Mode::Sample => self.set_mode(Mode::Trim),
            6 if self.state.mode == Mode::Trim => self.set_mode(Mode::Sample),
            _ => Self::ignored(format!(
                "{}.soft_key.{index}_unmapped",
                mode_reason(self.state.mode)
            )),
        }
    }

    fn handle_disk_soft_key(&mut self, index: u8) -> Vec<MachineOutput> {
        match index {
            2 => vec![MachineOutput::DiskOperationRequested {
                operation: DiskOperation::SaveProject,
            }],
            3 => vec![MachineOutput::DiskOperationRequested {
                operation: DiskOperation::LoadProject,
            }],
            _ => Self::ignored(format!("disk.soft_key.{index}_unmapped")),
        }
    }

    fn handle_song_soft_key(&mut self, index: u8) -> Vec<MachineOutput> {
        match index {
            2 => self.insert_song_step_after_selected(),
            3 => self.delete_selected_song_step(),
            _ => Self::ignored(format!("song.soft_key.{index}_unmapped")),
        }
    }

    fn adjust_disk_operation(&mut self, delta: i32) -> Vec<MachineOutput> {
        match delta.cmp(&0) {
            std::cmp::Ordering::Less => {
                self.select_disk_operation(self.state.selected_disk_operation.previous())
            }
            std::cmp::Ordering::Greater => {
                self.select_disk_operation(self.state.selected_disk_operation.next())
            }
            std::cmp::Ordering::Equal => {
                Self::ignored("disk.data_wheel_zero_delta_ignored".to_string())
            }
        }
    }

    fn select_disk_operation(&mut self, operation: DiskOperation) -> Vec<MachineOutput> {
        self.state.selected_disk_operation = operation;
        self.refresh_lcd();
        vec![
            MachineOutput::DiskOperationSelected { operation },
            MachineOutput::LcdChanged,
        ]
    }

    fn select_setup_field(&mut self, field: SetupField) -> Vec<MachineOutput> {
        self.state.selected_setup_field = field;
        self.refresh_lcd();
        self.setup_preferences_changed_outputs()
    }

    fn adjust_setup_preferences(&mut self, delta: i32) -> Vec<MachineOutput> {
        if delta == 0 {
            return Self::ignored(format!(
                "setup.{}.zero_delta_ignored",
                self.state.selected_setup_field.label()
            ));
        }

        match self.state.selected_setup_field {
            SetupField::Metronome => {
                let next = delta.is_positive();
                if self.state.setup_preferences.metronome_enabled == next {
                    return Self::ignored("setup.metronome.boundary".to_string());
                }
                self.state.setup_preferences.metronome_enabled = next;
            }
            SetupField::CountInBars => {
                let next = clamp_delta_u8(
                    self.state.setup_preferences.count_in_bars,
                    delta,
                    MIN_SETUP_COUNT_IN_BARS,
                    MAX_SETUP_COUNT_IN_BARS,
                );
                if self.state.setup_preferences.count_in_bars == next {
                    return Self::ignored("setup.count_in_bars.boundary".to_string());
                }
                self.state.setup_preferences.count_in_bars = next;
            }
            SetupField::LcdContrast => {
                let next = clamp_delta_u8(
                    self.state.setup_preferences.lcd_contrast,
                    delta,
                    MIN_SETUP_LCD_CONTRAST,
                    MAX_SETUP_LCD_CONTRAST,
                );
                if self.state.setup_preferences.lcd_contrast == next {
                    return Self::ignored("setup.lcd_contrast.boundary".to_string());
                }
                self.state.setup_preferences.lcd_contrast = next;
            }
        }

        self.refresh_lcd();
        self.setup_preferences_changed_outputs()
    }

    fn setup_preferences_changed_outputs(&self) -> Vec<MachineOutput> {
        vec![
            MachineOutput::SetupPreferencesChanged {
                preferences: self.state.setup_preferences,
                selected_field: self.state.selected_setup_field,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn adjust_timing_correct(&mut self, delta: i32) -> Vec<MachineOutput> {
        if delta == 0 {
            return Self::ignored(format!(
                "timing_correct.{}.zero_delta_ignored",
                self.state.selected_timing_correct_field.label()
            ));
        }

        match self.state.selected_timing_correct_field {
            TimingCorrectField::Division => {
                self.state.timing_correct.division = if delta.is_positive() {
                    self.state.timing_correct.division.next()
                } else {
                    self.state.timing_correct.division.previous()
                };
            }
            TimingCorrectField::Swing => {
                let next = clamp_delta_u8(
                    self.state.timing_correct.swing_percent,
                    delta,
                    MIN_TIMING_CORRECT_SWING_PERCENT,
                    MAX_TIMING_CORRECT_SWING_PERCENT,
                );
                if next == self.state.timing_correct.swing_percent {
                    return Self::ignored("timing_correct.swing.boundary".to_string());
                }
                self.state.timing_correct.swing_percent = next;
            }
        }

        self.refresh_lcd();
        self.timing_correct_changed_outputs()
    }

    fn timing_correct_changed_outputs(&self) -> Vec<MachineOutput> {
        vec![
            MachineOutput::TimingCorrectChanged {
                settings: self.state.timing_correct,
                selected_field: self.state.selected_timing_correct_field,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn select_song_step_by_delta(
        &mut self,
        delta: i32,
        no_movement_reason: &str,
    ) -> Vec<MachineOutput> {
        if delta == 0 {
            return Self::ignored("song.step.zero_delta_ignored".to_string());
        }

        let last_index = self.state.song_steps.len().saturating_sub(1);
        let next_index =
            clamp_delta_usize(self.state.selected_song_step_index, delta, 0, last_index);
        if next_index == self.state.selected_song_step_index {
            return Self::ignored(no_movement_reason.to_string());
        }

        self.state.selected_song_step_index = next_index;
        let step = self.state.song_steps[next_index];
        self.refresh_lcd();
        vec![
            MachineOutput::SongStepSelected {
                index: next_index,
                step,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn adjust_song_field(&mut self, delta: i32) -> Vec<MachineOutput> {
        if delta == 0 {
            return Self::ignored(format!(
                "song.{}.zero_delta_ignored",
                self.state.selected_song_edit_field.label()
            ));
        }

        match self.state.selected_song_edit_field {
            SongEditField::Step => self.select_song_step_by_delta(delta, "song.step.boundary"),
            SongEditField::Sequence => self.adjust_selected_song_step_sequence(delta),
            SongEditField::Repeats => self.adjust_selected_song_step_repeats(delta),
        }
    }

    fn adjust_selected_song_step_sequence(&mut self, delta: i32) -> Vec<MachineOutput> {
        let index = self.state.selected_song_step_index;
        let step = &mut self.state.song_steps[index];
        let next = clamp_delta_u8(
            step.sequence_index,
            delta,
            MIN_SONG_SEQUENCE_INDEX,
            MAX_SONG_SEQUENCE_INDEX,
        );
        if next == step.sequence_index {
            return Self::ignored("song.sequence.boundary".to_string());
        }

        step.sequence_index = next;
        let step = *step;
        self.refresh_lcd();
        vec![
            MachineOutput::SongStepChanged {
                index,
                field: SongEditField::Sequence,
                step,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn adjust_selected_song_step_repeats(&mut self, delta: i32) -> Vec<MachineOutput> {
        let index = self.state.selected_song_step_index;
        let step = &mut self.state.song_steps[index];
        let next = clamp_delta_u8(step.repeats, delta, MIN_SONG_REPEATS, MAX_SONG_REPEATS);
        if next == step.repeats {
            return Self::ignored("song.repeats.boundary".to_string());
        }

        step.repeats = next;
        let step = *step;
        self.refresh_lcd();
        vec![
            MachineOutput::SongStepChanged {
                index,
                field: SongEditField::Repeats,
                step,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn insert_song_step_after_selected(&mut self) -> Vec<MachineOutput> {
        let selected_index = self.state.selected_song_step_index;
        let selected_step = self.state.song_steps[selected_index];
        let inserted_index = selected_index + 1;
        let inserted_step = SongStep {
            sequence_index: selected_step.sequence_index,
            repeats: MIN_SONG_REPEATS,
        };

        self.state.song_steps.insert(inserted_index, inserted_step);
        self.state.selected_song_step_index = inserted_index;
        self.refresh_lcd();
        vec![
            MachineOutput::SongStepInserted {
                index: inserted_index,
                step: inserted_step,
            },
            MachineOutput::SongStepSelected {
                index: inserted_index,
                step: inserted_step,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn delete_selected_song_step(&mut self) -> Vec<MachineOutput> {
        if self.state.song_steps.len() == 1 {
            return Self::ignored("song.delete.last_step_ignored".to_string());
        }

        let deleted_index = self.state.selected_song_step_index;
        let deleted_step = self.state.song_steps.remove(deleted_index);
        let selected_index = deleted_index.min(self.state.song_steps.len() - 1);
        self.state.selected_song_step_index = selected_index;
        let selected_step = self.state.song_steps[selected_index];
        self.refresh_lcd();
        vec![
            MachineOutput::SongStepDeleted {
                index: deleted_index,
                step: deleted_step,
            },
            MachineOutput::SongStepSelected {
                index: selected_index,
                step: selected_step,
            },
            MachineOutput::LcdChanged,
        ]
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

    fn adjust_midi_settings(&mut self, delta: i32) -> Vec<MachineOutput> {
        match self.state.selected_midi_settings_field {
            MidiSettingsField::InputChannel => {
                self.state.midi_input_channel =
                    clamp_delta_midi_input_channel(self.state.midi_input_channel, delta);
            }
            MidiSettingsField::BaseNote => {
                self.state.midi_base_note =
                    clamp_delta_u8(self.state.midi_base_note, delta, 0, MAX_MIDI_BASE_NOTE);
            }
        }

        self.refresh_lcd();
        vec![
            MachineOutput::MidiSettingsChanged {
                input_channel: self.state.midi_input_channel,
                base_note: self.state.midi_base_note,
                selected_field: self.state.selected_midi_settings_field,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn refresh_lcd(&mut self) {
        if matches!(self.state.mode, Mode::Sample | Mode::Trim) {
            self.ensure_selected_sample_selection();
        }

        self.state.lcd = match self.state.mode {
            Mode::Main => LcdFrame::main_screen(
                self.state.sequence_index,
                &self.state.sequence_name,
                self.state.selected_track,
                self.state.is_track_muted(self.state.selected_track),
                self.state.muted_tracks.len(),
                &self.state.current_program.name,
                self.state.tempo_bpm_x100,
                self.state.playing,
                self.state.recording,
                self.state.loop_enabled,
                self.state.bar_count,
                self.state.sequence_length_ticks(),
                self.state.selected_main_field,
                self.state.playhead_ticks,
                self.state.recorded_events.len(),
            ),
            Mode::Program => LcdFrame::program_screen(
                &self.state.current_program,
                self.state.selected_program_pad,
                self.state.selected_program_edit_field,
                self.assignment_for(self.state.selected_program_pad),
            ),
            Mode::Sample => {
                let selected_sample = self.state.selected_sample();
                LcdFrame::sample_screen(selected_sample.as_ref())
            }
            Mode::Trim => {
                let selected_sample = self.state.selected_sample();
                LcdFrame::trim_screen(
                    selected_sample.as_ref(),
                    self.state.selected_trim_edit_field,
                )
            }
            Mode::Song => {
                let step = self.state.song_steps[self.state.selected_song_step_index];
                let sequence_name =
                    sequence_name_for(song_sequence_display_index(step.sequence_index));
                LcdFrame::song_screen(
                    &self.state.song_steps,
                    self.state.selected_song_step_index,
                    self.state.selected_song_edit_field,
                    &sequence_name,
                )
            }
            Mode::Midi => LcdFrame::midi_screen(
                self.state.midi_input_channel,
                self.state.midi_base_note,
                self.state.selected_midi_settings_field,
            ),
            Mode::TimingCorrect => LcdFrame::timing_correct_screen(
                self.state.timing_correct,
                self.state.selected_timing_correct_field,
            ),
            Mode::Disk => LcdFrame::disk_screen(self.state.selected_disk_operation),
            Mode::Setup => LcdFrame::setup_screen(
                self.state.setup_preferences,
                self.state.selected_setup_field,
            ),
        };
    }

    fn ignored(reason: String) -> Vec<MachineOutput> {
        vec![MachineOutput::Ignored { reason }]
    }

    fn ensure_selected_sample_selection(&mut self) {
        self.state.selected_sample_id = normalized_sample_id(
            &self.state.current_program,
            self.state.selected_sample_id.as_deref(),
        );
    }

    fn adjust_selected_sample(&mut self, delta: i32) -> Vec<MachineOutput> {
        let catalog = self.state.sample_catalog();
        if catalog.is_empty() {
            self.state.selected_sample_id = None;
            return Self::ignored("sample_catalog.empty".to_string());
        }

        let current_index =
            selected_sample_position(&catalog, self.state.selected_sample_id.as_deref())
                .unwrap_or(0);
        let next_index = clamp_delta_usize(current_index, delta, 0, catalog.len() - 1);
        let entry = catalog[next_index].clone();
        self.state.selected_sample_id = Some(entry.sample.id.clone());
        self.refresh_lcd();
        vec![
            MachineOutput::SampleSelected { entry },
            MachineOutput::LcdChanged,
        ]
    }

    fn adjust_selected_sample_trim(&mut self, delta: i32) -> Vec<MachineOutput> {
        if delta == 0 {
            return Self::ignored(format!(
                "trim.{}.zero_delta_ignored",
                self.state.selected_trim_edit_field.label()
            ));
        }

        let Some(entry) = self.state.selected_sample() else {
            self.state.selected_sample_id = None;
            return Self::ignored("sample_catalog.empty".to_string());
        };

        let mut start_frame = entry.start_frame;
        let mut end_frame = entry.end_frame;
        match self.state.selected_trim_edit_field {
            TrimEditField::Start => {
                start_frame = clamp_delta_u32(start_frame, delta, 0, end_frame);
                if start_frame == entry.start_frame {
                    return Self::ignored("trim.start.boundary".to_string());
                }
            }
            TrimEditField::End => {
                end_frame = clamp_delta_u32(
                    end_frame,
                    delta,
                    start_frame,
                    entry.length_frames.saturating_sub(1),
                );
                if end_frame == entry.end_frame {
                    return Self::ignored("trim.end.boundary".to_string());
                }
            }
        }

        let window_length_frames = sample_window_length_frames(start_frame, end_frame);
        self.set_sample_trim(
            entry.sample.id.clone(),
            start_frame,
            end_frame,
            entry.length_frames,
        );
        self.refresh_lcd();
        vec![
            MachineOutput::SampleTrimChanged {
                sample_id: entry.sample.id,
                start_frame,
                end_frame,
                window_length_frames,
                selected_field: self.state.selected_trim_edit_field,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn set_sample_trim(
        &mut self,
        sample_id: String,
        start_frame: u32,
        end_frame: u32,
        length_frames: u32,
    ) {
        let default_end_frame = length_frames.saturating_sub(1);
        if start_frame == 0 && end_frame == default_end_frame {
            self.state
                .sample_trims
                .retain(|trim| trim.sample_id != sample_id);
            return;
        }

        if let Some(trim) = self
            .state
            .sample_trims
            .iter_mut()
            .find(|trim| trim.sample_id == sample_id)
        {
            trim.start_frame = start_frame;
            trim.end_frame = end_frame;
        } else {
            self.state.sample_trims.push(SampleTrim {
                sample_id,
                start_frame,
                end_frame,
            });
        }
        self.state
            .sample_trims
            .sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
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
            let raw_tick = self.state.playhead_ticks;
            let quantized_tick = quantize_tick(
                raw_tick,
                self.state.sequence_length_ticks(),
                self.state.timing_correct,
            );
            let event = SequenceEvent {
                selected_track: self.state.selected_track,
                pad_bank: bank,
                pad_number: pad,
                velocity,
                tick: quantized_tick,
                playback: match &playback {
                    SamplePlaybackResolution::Intent { intent } => Some(intent.clone()),
                    SamplePlaybackResolution::Miss { .. } => None,
                },
            };
            self.state.recorded_events.push(event.clone());
            if quantized_tick != raw_tick {
                outputs.push(MachineOutput::TimingCorrectApplied {
                    original_tick: raw_tick,
                    quantized_tick,
                    division: self.state.timing_correct.division,
                    swing_percent: self.state.timing_correct.swing_percent,
                });
            }
            outputs.push(MachineOutput::SequenceEventRecorded { event });
        }

        self.refresh_lcd();
        if self.state.lcd != previous_lcd {
            outputs.push(MachineOutput::LcdChanged);
        }

        outputs
    }

    fn handle_midi_note_on(&mut self, channel: u8, note: u8, velocity: u8) -> Vec<MachineOutput> {
        if let Some(reason) = validate_midi_note_on(channel, note, velocity) {
            return Self::midi_ignored(reason);
        }

        if let Some(input_channel) = self.state.midi_input_channel {
            if channel != input_channel {
                return Self::midi_ignored(format!(
                    "midi channel {channel} ignored; input channel is {input_channel}"
                ));
            }
        }

        let Some(pad) = midi_note_to_bank_a_pad(note, self.state.midi_base_note) else {
            let range_end = midi_mapped_range_end(self.state.midi_base_note);
            return Self::midi_ignored(format!(
                "midi note {note} is not mapped in this slice; mapped range is {}..={range_end}",
                self.state.midi_base_note
            ));
        };

        let mut outputs = vec![MachineOutput::MidiNoteMapped {
            channel,
            note,
            bank: PadBank::A,
            pad,
            velocity,
        }];
        outputs.extend(self.handle_pad_strike(PadBank::A, pad, velocity));
        outputs
    }

    fn handle_midi_note_off(&mut self, channel: u8, note: u8, velocity: u8) -> Vec<MachineOutput> {
        if let Some(reason) = validate_midi_note_off(channel, note, velocity) {
            return Self::midi_ignored(reason);
        }

        Self::midi_ignored("midi note-off is a no-op in this slice")
    }

    fn midi_ignored(reason: impl Into<String>) -> Vec<MachineOutput> {
        vec![MachineOutput::MidiInputIgnored {
            reason: reason.into(),
        }]
    }

    fn handle_tick(&mut self, micros: u64) -> Vec<MachineOutput> {
        if !self.state.playing {
            return Vec::new();
        }

        let previous_lcd = self.state.lcd.clone();
        let previous_playhead_ticks = self.state.playhead_ticks;
        let sequence_length_ticks = self.state.sequence_length_ticks();
        let numerator = u128::from(micros)
            .saturating_mul(u128::from(self.state.tempo_bpm_x100))
            .saturating_mul(u128::from(INTERNAL_PPQN))
            .saturating_add(u128::from(self.state.playhead_tick_remainder));
        let tick_delta = numerator / TICK_DENOMINATOR;
        let remainder = numerator % TICK_DENOMINATOR;
        let tick_delta = u64::try_from(tick_delta).unwrap_or(u64::MAX);
        let scheduled_events;
        let mut transport_stopped = false;

        if tick_delta == 0 {
            self.state.playhead_tick_remainder = remainder as u64;
            scheduled_events = Vec::new();
        } else if self.state.loop_enabled {
            let previous_loop_tick = previous_playhead_ticks % sequence_length_ticks;
            let total_loop_ticks =
                u128::from(previous_loop_tick).saturating_add(u128::from(tick_delta));
            let looped = total_loop_ticks >= u128::from(sequence_length_ticks);
            let next_loop_tick = (total_loop_ticks % u128::from(sequence_length_ticks)) as u64;

            self.state.playhead_ticks = next_loop_tick;
            self.state.playhead_tick_remainder = remainder as u64;
            scheduled_events = self.scheduled_loop_events(
                previous_loop_tick,
                next_loop_tick,
                sequence_length_ticks,
                looped,
            );
        } else {
            let target_playhead_ticks = previous_playhead_ticks.saturating_add(tick_delta);
            let next_playhead_ticks = target_playhead_ticks.min(sequence_length_ticks);

            self.state.playhead_ticks = next_playhead_ticks;
            self.state.playhead_tick_remainder = if target_playhead_ticks >= sequence_length_ticks {
                0
            } else {
                remainder as u64
            };
            scheduled_events = self.scheduled_events_between(
                previous_playhead_ticks,
                next_playhead_ticks,
                sequence_length_ticks,
                false,
            );

            if target_playhead_ticks >= sequence_length_ticks {
                self.state.playing = false;
                self.state.recording = false;
                transport_stopped = true;
            }
        };

        let mut outputs = Vec::new();
        for (event, intent) in scheduled_events {
            outputs.push(MachineOutput::SequenceEventPlayed { event });
            outputs.push(MachineOutput::SamplePlaybackIntent {
                intent: intent.clone(),
            });
            self.state.last_playback = Some(SamplePlaybackResolution::Intent { intent });
        }
        if transport_stopped {
            outputs.push(MachineOutput::TransportChanged {
                playing: false,
                recording: false,
            });
        }

        self.refresh_lcd();
        if self.state.lcd != previous_lcd {
            outputs.push(MachineOutput::LcdChanged);
        }

        outputs
    }

    fn scheduled_loop_events(
        &self,
        previous_tick: u64,
        next_tick: u64,
        sequence_length_ticks: u64,
        looped: bool,
    ) -> Vec<(SequenceEvent, SamplePlaybackIntent)> {
        if looped {
            let mut scheduled = self.scheduled_events_between(
                previous_tick,
                sequence_length_ticks,
                sequence_length_ticks,
                false,
            );
            scheduled.extend(self.scheduled_events_between(
                0,
                next_tick,
                sequence_length_ticks,
                true,
            ));
            scheduled
        } else {
            self.scheduled_events_between(previous_tick, next_tick, sequence_length_ticks, false)
        }
    }

    fn scheduled_events_between(
        &self,
        previous_tick: u64,
        next_tick: u64,
        sequence_length_ticks: u64,
        include_tick_zero: bool,
    ) -> Vec<(SequenceEvent, SamplePlaybackIntent)> {
        if next_tick < previous_tick {
            return Vec::new();
        }

        self.state
            .recorded_events
            .iter()
            .filter(|event| event.tick <= sequence_length_ticks)
            .filter(|event| !self.state.is_track_muted(event.selected_track))
            .filter(|event| {
                (include_tick_zero && event.tick == 0)
                    || (previous_tick < event.tick && event.tick <= next_tick)
            })
            .filter_map(|event| {
                event
                    .playback
                    .as_ref()
                    .map(|intent| (event.clone(), intent.clone()))
            })
            .collect()
    }

    fn assignment_for(&self, pad: ProgramPad) -> Option<&PadAssignment> {
        self.state
            .current_program
            .pad_assignments
            .iter()
            .find(|assignment| assignment.pad == pad)
    }

    fn assignment_for_mut(&mut self, pad: ProgramPad) -> Option<&mut PadAssignment> {
        self.state
            .current_program
            .pad_assignments
            .iter_mut()
            .find(|assignment| assignment.pad == pad)
    }

    fn adjust_selected_program_pad(&mut self, delta: i32) {
        let next = clamp_delta_u8(self.state.selected_program_pad.pad_number, delta, 1, 16);
        self.state.selected_program_pad.pad_number = next;
    }

    fn adjust_selected_program_field(&mut self, delta: i32) -> Vec<MachineOutput> {
        match self.state.selected_program_edit_field {
            ProgramEditField::Pad => {
                self.adjust_selected_program_pad(delta);
                self.refresh_lcd();
                vec![MachineOutput::LcdChanged]
            }
            ProgramEditField::Level | ProgramEditField::Pan | ProgramEditField::Tune => {
                self.adjust_selected_pad_parameter(delta)
            }
        }
    }

    fn adjust_selected_pad_parameter(&mut self, delta: i32) -> Vec<MachineOutput> {
        let pad = self.state.selected_program_pad;
        let parameter = self.state.selected_program_edit_field;
        let Some(assignment) = self.assignment_for_mut(pad) else {
            return Self::ignored(format!(
                "program.{}.unassigned_{}{:02}",
                parameter.label(),
                pad.bank.label().to_ascii_lowercase(),
                pad.pad_number
            ));
        };

        let value = match parameter {
            ProgramEditField::Level => {
                assignment.level = clamp_delta_u8(assignment.level, delta, 0, MAX_PAD_LEVEL);
                i16::from(assignment.level)
            }
            ProgramEditField::Pan => {
                assignment.pan = clamp_delta_i8(assignment.pan, delta, MIN_PAD_PAN, MAX_PAD_PAN);
                i16::from(assignment.pan)
            }
            ProgramEditField::Tune => {
                assignment.tune_cents = clamp_delta_i16(
                    assignment.tune_cents,
                    delta.saturating_mul(100),
                    MIN_PAD_TUNE_CENTS,
                    MAX_PAD_TUNE_CENTS,
                );
                assignment.tune_cents
            }
            ProgramEditField::Pad => unreachable!("pad edits are handled before parameter edits"),
        };
        let assignment = assignment.clone();

        self.refresh_lcd();
        vec![
            MachineOutput::PadParameterChanged {
                bank: pad.bank,
                pad: pad.pad_number,
                parameter,
                value,
                assignment,
            },
            MachineOutput::LcdChanged,
        ]
    }

    fn clear_selected_pad_assignment(&mut self) -> Vec<MachineOutput> {
        let pad = self.state.selected_program_pad;
        self.state
            .current_program
            .pad_assignments
            .retain(|assignment| assignment.pad != pad);
        self.state.selected_sample_id = normalized_sample_id(
            &self.state.current_program,
            self.state.selected_sample_id.as_deref(),
        );
        self.prune_sample_trims_for_current_program();
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
        self.state.selected_sample_id = normalized_sample_id(
            &self.state.current_program,
            self.state.selected_sample_id.as_deref(),
        );
        self.prune_sample_trims_for_current_program();
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
            let (start_frame, end_frame, window_length_frames) =
                self.sample_window_for_assignment(assignment);
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
                    tune_cents: assignment.tune_cents,
                    start_frame,
                    end_frame,
                    window_length_frames,
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

    fn sample_window_for_assignment(&self, assignment: &PadAssignment) -> (u32, u32, u32) {
        let length_frames = generated_sample_length_frames(assignment.pad);
        let (start_frame, end_frame) = self
            .state
            .sample_trims
            .iter()
            .find(|trim| trim.sample_id == assignment.sample.id)
            .map(|trim| (trim.start_frame, trim.end_frame))
            .unwrap_or((0, length_frames.saturating_sub(1)));
        (
            start_frame,
            end_frame,
            sample_window_length_frames(start_frame, end_frame),
        )
    }

    fn prune_sample_trims_for_current_program(&mut self) {
        self.state.sample_trims =
            normalized_sample_trims(&self.state.current_program, &self.state.sample_trims);
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
            "song",
            "setup",
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
    if let Some(song) = root.get("song") {
        validate_song_json_fields(song)?;
    }
    if let Some(setup) = root.get("setup") {
        validate_setup_json_fields(setup)?;
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
            "selected_program_edit_field",
            "selected_sample_id",
            "selected_trim_edit_field",
            "midi_input_channel",
            "midi_base_note",
            "selected_midi_settings_field",
            "selected_disk_operation",
            "selected_timing_correct_field",
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
            "muted_tracks",
            "bar_count",
            "loop_enabled",
            "timing_correct",
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
    if let Some(timing_correct) = sequence.get("timing_correct") {
        validate_timing_correct_json_fields("sequence.timing_correct", timing_correct)?;
    }

    Ok(())
}

fn validate_timing_correct_json_fields(
    field: &str,
    value: &Value,
) -> Result<(), ProjectSnapshotError> {
    let Some(settings) = reject_unknown_json_fields(field, value, &["division", "swing_percent"])?
    else {
        return Ok(());
    };
    require_json_fields(field, settings, &["division", "swing_percent"])?;
    Ok(())
}

fn validate_program_json_fields(value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(program) = reject_unknown_json_fields(
        "program",
        value,
        &["index", "name", "pad_assignments", "sample_trims"],
    )?
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
    if let Some(sample_trims) = program.get("sample_trims").and_then(Value::as_array) {
        for (index, sample_trim) in sample_trims.iter().enumerate() {
            validate_sample_trim_json_fields(
                &format!("program.sample_trims[{index}]"),
                sample_trim,
            )?;
        }
    }

    Ok(())
}

fn validate_sample_trim_json_fields(
    field: &str,
    value: &Value,
) -> Result<(), ProjectSnapshotError> {
    reject_unknown_json_fields(field, value, &["sample_id", "start_frame", "end_frame"])?;
    Ok(())
}

fn validate_song_json_fields(value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(song) = reject_unknown_json_fields(
        "song",
        value,
        &["steps", "selected_step_index", "selected_field"],
    )?
    else {
        return Ok(());
    };
    require_json_fields(
        "song",
        song,
        &["steps", "selected_step_index", "selected_field"],
    )?;

    if let Some(steps) = song.get("steps").and_then(Value::as_array) {
        for (index, step) in steps.iter().enumerate() {
            validate_song_step_json_fields(&format!("song.steps[{index}]"), step)?;
        }
    }

    Ok(())
}

fn validate_song_step_json_fields(field: &str, value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(step) = reject_unknown_json_fields(field, value, &["sequence_index", "repeats"])?
    else {
        return Ok(());
    };
    require_json_fields(field, step, &["sequence_index", "repeats"])?;
    Ok(())
}

fn validate_setup_json_fields(value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(setup) =
        reject_unknown_json_fields("setup", value, &["preferences", "selected_field"])?
    else {
        return Ok(());
    };
    require_json_fields("setup", setup, &["preferences", "selected_field"])?;

    if let Some(preferences) = setup.get("preferences") {
        validate_setup_preferences_json_fields("setup.preferences", preferences)?;
    }

    Ok(())
}

fn validate_setup_preferences_json_fields(
    field: &str,
    value: &Value,
) -> Result<(), ProjectSnapshotError> {
    let Some(preferences) = reject_unknown_json_fields(
        field,
        value,
        &["metronome_enabled", "count_in_bars", "lcd_contrast"],
    )?
    else {
        return Ok(());
    };
    require_json_fields(
        field,
        preferences,
        &["metronome_enabled", "count_in_bars", "lcd_contrast"],
    )?;
    Ok(())
}

fn validate_assignment_json_fields(field: &str, value: &Value) -> Result<(), ProjectSnapshotError> {
    let Some(assignment) = reject_unknown_json_fields(
        field,
        value,
        &["pad", "sample", "level", "pan", "tune_cents"],
    )?
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
            "tune_cents",
            "start_frame",
            "end_frame",
            "window_length_frames",
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

fn require_json_fields(
    field: &str,
    object: &Map<String, Value>,
    required: &[&str],
) -> Result<(), ProjectSnapshotError> {
    for required_field in required {
        if !object.contains_key(*required_field) {
            return Err(invalid_value(
                &format!("{field}.{required_field}"),
                "required field is missing",
            ));
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
    validate_muted_tracks(&snapshot.sequence.muted_tracks)?;
    validate_range_u16(
        "sequence.bar_count",
        snapshot.sequence.bar_count,
        MIN_BAR_COUNT,
        MAX_BAR_COUNT,
    )?;
    validate_timing_correct_settings("sequence.timing_correct", snapshot.sequence.timing_correct)?;

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
    if let Some(selected_sample_id) = &snapshot.machine.selected_sample_id {
        validate_non_empty("machine.selected_sample_id", selected_sample_id)?;
    }
    if let Some(midi_input_channel) = snapshot.machine.midi_input_channel {
        validate_range_u8(
            "machine.midi_input_channel",
            midi_input_channel,
            MIDI_MIN_CHANNEL,
            MIDI_MAX_CHANNEL,
        )?;
    }
    validate_range_u8(
        "machine.midi_base_note",
        snapshot.machine.midi_base_note,
        0,
        MAX_MIDI_BASE_NOTE,
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

    let validation_program = Program {
        index: snapshot.program.index,
        name: snapshot.program.name.clone(),
        pad_assignments: snapshot.program.pad_assignments.clone(),
    };
    let default_catalog = sample_catalog_for_program(&validation_program, &[]);
    if let Some(selected_sample_id) = &snapshot.machine.selected_sample_id {
        if catalog_entry_for_sample_id(&default_catalog, selected_sample_id).is_none() {
            return Err(invalid_value(
                "machine.selected_sample_id",
                format!("unknown sample id {selected_sample_id:?}"),
            ));
        }
    }
    validate_sample_trims(&snapshot.program.sample_trims, &default_catalog)?;

    for (index, event) in snapshot.sequence.recorded_events.iter().enumerate() {
        validate_sequence_event(&format!("sequence.recorded_events[{index}]"), event)?;
    }

    validate_song_snapshot(&snapshot.song)?;
    validate_setup_snapshot(&snapshot.setup)?;

    Ok(())
}

fn validate_song_snapshot(song: &ProjectSongSnapshot) -> Result<(), ProjectSnapshotError> {
    if song.steps.is_empty() {
        return Err(invalid_value(
            "song.steps",
            "must contain at least one song step",
        ));
    }

    if song.selected_step_index >= song.steps.len() {
        return Err(invalid_value(
            "song.selected_step_index",
            format!("must be in range 0..={}", song.steps.len() - 1),
        ));
    }

    for (index, step) in song.steps.iter().enumerate() {
        validate_song_step(&format!("song.steps[{index}]"), step)?;
    }

    Ok(())
}

fn validate_muted_tracks(muted_tracks: &[u8]) -> Result<(), ProjectSnapshotError> {
    let mut seen_tracks = BTreeSet::new();
    for (index, track) in muted_tracks.iter().copied().enumerate() {
        validate_range_u8(
            &format!("sequence.muted_tracks[{index}]"),
            track,
            MIN_TRACK_INDEX,
            MAX_TRACK_INDEX,
        )?;
        if !seen_tracks.insert(track) {
            return Err(invalid_value(
                &format!("sequence.muted_tracks[{index}]"),
                "duplicate muted track",
            ));
        }
    }
    Ok(())
}

fn validate_song_step(field: &str, step: &SongStep) -> Result<(), ProjectSnapshotError> {
    validate_range_u8(
        &format!("{field}.sequence_index"),
        step.sequence_index,
        MIN_SONG_SEQUENCE_INDEX,
        MAX_SONG_SEQUENCE_INDEX,
    )?;
    validate_range_u8(
        &format!("{field}.repeats"),
        step.repeats,
        MIN_SONG_REPEATS,
        MAX_SONG_REPEATS,
    )
}

fn validate_setup_snapshot(setup: &ProjectSetupSnapshot) -> Result<(), ProjectSnapshotError> {
    validate_range_u8(
        "setup.preferences.count_in_bars",
        setup.preferences.count_in_bars,
        MIN_SETUP_COUNT_IN_BARS,
        MAX_SETUP_COUNT_IN_BARS,
    )?;
    validate_range_u8(
        "setup.preferences.lcd_contrast",
        setup.preferences.lcd_contrast,
        MIN_SETUP_LCD_CONTRAST,
        MAX_SETUP_LCD_CONTRAST,
    )
}

fn validate_timing_correct_settings(
    field: &str,
    settings: TimingCorrectSettings,
) -> Result<(), ProjectSnapshotError> {
    validate_range_u8(
        &format!("{field}.swing_percent"),
        settings.swing_percent,
        MIN_TIMING_CORRECT_SWING_PERCENT,
        MAX_TIMING_CORRECT_SWING_PERCENT,
    )
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
    )?;
    validate_range_i16(
        &format!("{field}.tune_cents"),
        assignment.tune_cents,
        MIN_PAD_TUNE_CENTS,
        MAX_PAD_TUNE_CENTS,
    )
}

fn validate_sample_trims(
    sample_trims: &[SampleTrim],
    catalog: &[SampleCatalogEntry],
) -> Result<(), ProjectSnapshotError> {
    let mut seen_sample_ids = BTreeSet::new();
    for (index, trim) in sample_trims.iter().enumerate() {
        let field = format!("program.sample_trims[{index}]");
        validate_non_empty(&format!("{field}.sample_id"), &trim.sample_id)?;
        if !seen_sample_ids.insert(trim.sample_id.as_str()) {
            return Err(invalid_value(
                &format!("{field}.sample_id"),
                "duplicate sample trim",
            ));
        }
        let Some(entry) = catalog_entry_for_sample_id(catalog, &trim.sample_id) else {
            return Err(invalid_value(
                &format!("{field}.sample_id"),
                format!("unknown sample id {:?}", trim.sample_id),
            ));
        };
        validate_sample_window(
            &field,
            trim.start_frame,
            trim.end_frame,
            trim.window_length_frames(),
            entry.length_frames,
        )?;
    }
    Ok(())
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
    )?;
    validate_range_i16(
        &format!("{field}.tune_cents"),
        intent.tune_cents,
        MIN_PAD_TUNE_CENTS,
        MAX_PAD_TUNE_CENTS,
    )?;
    let length_frames = generated_sample_length_frames(ProgramPad {
        bank: intent.bank,
        pad_number: intent.pad_number,
    });
    validate_sample_window(
        field,
        intent.start_frame,
        intent.end_frame,
        intent.window_length_frames,
        length_frames,
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

fn validate_sample_window(
    field: &str,
    start_frame: u32,
    end_frame: u32,
    window_length_frames: u32,
    length_frames: u32,
) -> Result<(), ProjectSnapshotError> {
    if start_frame > end_frame {
        return Err(invalid_value(
            &format!("{field}.start_frame"),
            "must be <= end_frame",
        ));
    }
    if end_frame >= length_frames {
        return Err(invalid_value(
            &format!("{field}.end_frame"),
            format!("must be less than generated sample length {length_frames}"),
        ));
    }
    let expected_window_length = sample_window_length_frames(start_frame, end_frame);
    if window_length_frames != expected_window_length {
        return Err(invalid_value(
            &format!("{field}.window_length_frames"),
            format!("must equal end_frame - start_frame + 1 ({expected_window_length})"),
        ));
    }
    Ok(())
}

fn validate_midi_channel(channel: u8) -> Option<String> {
    if !(MIDI_MIN_CHANNEL..=MIDI_MAX_CHANNEL).contains(&channel) {
        return Some("midi channel must be in range 1..=16".to_string());
    }
    None
}

fn validate_midi_note(note: u8) -> Option<String> {
    if note > MIDI_MAX_NOTE {
        return Some("midi note must be in range 0..=127".to_string());
    }
    None
}

fn validate_midi_note_on(channel: u8, note: u8, velocity: u8) -> Option<String> {
    validate_midi_channel(channel)
        .or_else(|| validate_midi_note(note))
        .or_else(|| {
            if velocity == 0 || velocity > 127 {
                Some("midi note-on velocity must be in range 1..=127".to_string())
            } else {
                None
            }
        })
}

fn validate_midi_note_off(channel: u8, note: u8, velocity: u8) -> Option<String> {
    validate_midi_channel(channel)
        .or_else(|| validate_midi_note(note))
        .or_else(|| {
            if velocity > 127 {
                Some("midi note-off velocity must be in range 0..=127".to_string())
            } else {
                None
            }
        })
}

fn midi_note_to_bank_a_pad(note: u8, base_note: u8) -> Option<u8> {
    let range_end = midi_mapped_range_end(base_note);
    if (base_note..=range_end).contains(&note) {
        Some(note.saturating_sub(base_note).saturating_add(1))
    } else {
        None
    }
}

fn midi_mapped_range_end(base_note: u8) -> u8 {
    base_note.saturating_add(MIDI_MAPPED_PAD_COUNT.saturating_sub(1))
}

fn clamp_delta_midi_input_channel(current: Option<u8>, delta: i32) -> Option<u8> {
    let current = current.unwrap_or(0);
    match clamp_delta_u8(current, delta, 0, MIDI_MAX_CHANNEL) {
        0 => None,
        channel => Some(channel),
    }
}

fn default_midi_base_note() -> u8 {
    DEFAULT_MIDI_BASE_NOTE
}

fn default_song_steps() -> Vec<SongStep> {
    vec![SongStep {
        sequence_index: MIN_SONG_SEQUENCE_INDEX,
        repeats: MIN_SONG_REPEATS,
    }]
}

fn sorted_tracks(mut tracks: Vec<u8>) -> Vec<u8> {
    tracks.sort_unstable();
    tracks
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

fn validate_range_i16(
    field: &str,
    value: i16,
    min: i16,
    max: i16,
) -> Result<(), ProjectSnapshotError> {
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

fn song_sequence_display_index(index: u8) -> u8 {
    index + 1
}

pub fn sequence_length_ticks_for_bars(bar_count: u16) -> u64 {
    u64::from(bar_count.max(MIN_BAR_COUNT))
        .saturating_mul(u64::from(INTERNAL_PPQN))
        .saturating_mul(u64::from(FOUNDATION_BEATS_PER_BAR))
}

fn quantize_tick(tick: u64, sequence_length_ticks: u64, settings: TimingCorrectSettings) -> u64 {
    let Some(grid_ticks) = settings.division.grid_ticks() else {
        return tick;
    };
    if tick >= sequence_length_ticks {
        return sequence_length_ticks;
    }

    let rough_index = tick / grid_ticks;
    let max_index = sequence_length_ticks
        .saturating_add(grid_ticks)
        .saturating_sub(1)
        / grid_ticks
        + 2;
    let start_index = rough_index.saturating_sub(3);
    let end_index = rough_index.saturating_add(3).min(max_index);
    let mut best = swung_grid_target(0, grid_ticks, settings).min(sequence_length_ticks);

    for index in start_index..=end_index {
        let candidate = swung_grid_target(index, grid_ticks, settings).min(sequence_length_ticks);
        let best_distance = best.abs_diff(tick);
        let candidate_distance = candidate.abs_diff(tick);
        if candidate_distance < best_distance
            || (candidate_distance == best_distance && candidate > best)
        {
            best = candidate;
        }
    }

    best
}

fn swung_grid_target(grid_index: u64, grid_ticks: u64, settings: TimingCorrectSettings) -> u64 {
    if !settings.division.uses_swing() || settings.swing_percent == 50 || grid_index % 2 == 0 {
        return grid_index.saturating_mul(grid_ticks);
    }

    let pair_base = grid_index.saturating_sub(1).saturating_mul(grid_ticks);
    let pair_ticks = grid_ticks.saturating_mul(2);
    let offset = div_round_u64(
        pair_ticks.saturating_mul(u64::from(settings.swing_percent)),
        100,
    );
    pair_base.saturating_add(offset)
}

fn div_round_u64(numerator: u64, denominator: u64) -> u64 {
    numerator.saturating_add(denominator / 2) / denominator
}

fn sample_catalog_for_program(
    program: &Program,
    sample_trims: &[SampleTrim],
) -> Vec<SampleCatalogEntry> {
    let mut assignments = program.pad_assignments.clone();
    assignments.sort_by_key(|assignment| assignment.pad);

    let mut seen_sample_ids = BTreeSet::new();
    let mut entries = Vec::new();
    for assignment in assignments {
        if !seen_sample_ids.insert(assignment.sample.id.clone()) {
            continue;
        }

        let length_frames = generated_sample_length_frames(assignment.pad);
        let (start_frame, end_frame) = sample_trims
            .iter()
            .find(|trim| trim.sample_id == assignment.sample.id)
            .map(|trim| (trim.start_frame, trim.end_frame))
            .unwrap_or((0, length_frames.saturating_sub(1)));
        entries.push(SampleCatalogEntry {
            index: 0,
            count: 0,
            sample: assignment.sample,
            source_pad: assignment.pad,
            start_frame,
            end_frame,
            window_length_frames: sample_window_length_frames(start_frame, end_frame),
            length_frames,
        });
    }

    let count = entries.len();
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.index = index + 1;
        entry.count = count;
    }

    entries
}

fn normalized_sample_id(program: &Program, requested_id: Option<&str>) -> Option<String> {
    let catalog = sample_catalog_for_program(program, &[]);
    selected_sample_entry(&catalog, requested_id)
        .or_else(|| catalog.first())
        .map(|entry| entry.sample.id.clone())
}

fn normalized_sample_trims(program: &Program, sample_trims: &[SampleTrim]) -> Vec<SampleTrim> {
    let catalog = sample_catalog_for_program(program, &[]);
    let mut normalized = sample_trims
        .iter()
        .filter(|trim| catalog_entry_for_sample_id(&catalog, &trim.sample_id).is_some())
        .cloned()
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
    normalized
}

fn selected_sample_entry<'a>(
    catalog: &'a [SampleCatalogEntry],
    selected_sample_id: Option<&str>,
) -> Option<&'a SampleCatalogEntry> {
    selected_sample_id
        .and_then(|sample_id| {
            catalog
                .iter()
                .find(|entry| entry.sample.id.as_str() == sample_id)
        })
        .or_else(|| catalog.first())
}

fn selected_sample_position(
    catalog: &[SampleCatalogEntry],
    selected_sample_id: Option<&str>,
) -> Option<usize> {
    selected_sample_id.and_then(|sample_id| {
        catalog
            .iter()
            .position(|entry| entry.sample.id.as_str() == sample_id)
    })
}

fn catalog_entry_for_sample_id<'a>(
    catalog: &'a [SampleCatalogEntry],
    sample_id: &str,
) -> Option<&'a SampleCatalogEntry> {
    catalog
        .iter()
        .find(|entry| entry.sample.id.as_str() == sample_id)
}

fn default_program() -> Program {
    let pad_assignments = PAD_BANKS
        .iter()
        .copied()
        .flat_map(|bank| {
            (1..=16).map(move |pad_number| generated_assignment(ProgramPad { bank, pad_number }))
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
        tune_cents: DEFAULT_PAD_TUNE_CENTS,
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

fn clamp_delta_u32(current: u32, delta: i32, min: u32, max: u32) -> u32 {
    (i128::from(current) + i128::from(delta)).clamp(i128::from(min), i128::from(max)) as u32
}

fn clamp_delta_usize(current: usize, delta: i32, min: usize, max: usize) -> usize {
    (current as i128 + i128::from(delta)).clamp(min as i128, max as i128) as usize
}

fn clamp_delta_i8(current: i8, delta: i32, min: i8, max: i8) -> i8 {
    (i64::from(current) + i64::from(delta)).clamp(i64::from(min), i64::from(max)) as i8
}

fn clamp_delta_i16(current: i16, delta: i32, min: i16, max: i16) -> i16 {
    (i64::from(current) + i64::from(delta)).clamp(i64::from(min), i64::from(max)) as i16
}

fn mode_reason(mode: Mode) -> &'static str {
    match mode {
        Mode::Main => "main_screen",
        Mode::Program => "program",
        Mode::Sample => "sample",
        Mode::Trim => "trim",
        Mode::Song => "song",
        Mode::Midi => "midi",
        Mode::TimingCorrect => "timing_correct",
        Mode::Disk => "disk",
        Mode::Setup => "setup",
    }
}
