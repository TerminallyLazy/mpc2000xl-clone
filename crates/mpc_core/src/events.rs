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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadBank {
    A,
    B,
    C,
    D,
}

impl PadBank {
    pub fn label(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProgramPad {
    pub bank: PadBank,
    pub pad_number: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntheticSample {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleCatalogEntry {
    pub index: usize,
    pub count: usize,
    pub sample: SyntheticSample,
    pub source_pad: ProgramPad,
    pub start_frame: u32,
    pub end_frame: u32,
    pub length_frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadAssignment {
    pub pad: ProgramPad,
    pub sample: SyntheticSample,
    pub level: u8,
    pub pan: i8,
    #[serde(default)]
    pub tune_cents: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub index: u8,
    pub name: String,
    pub pad_assignments: Vec<PadAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SamplePlaybackIntent {
    pub selected_track: u8,
    pub program_index: u8,
    pub program_name: String,
    pub bank: PadBank,
    pub pad_number: u8,
    pub sample_id: String,
    pub sample_name: String,
    pub velocity: u8,
    pub level: u8,
    pub pan: i8,
    #[serde(default)]
    pub tune_cents: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackMissReason {
    PadUnassigned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SamplePlaybackMiss {
    pub selected_track: u8,
    pub program_index: u8,
    pub program_name: String,
    pub bank: PadBank,
    pub pad_number: u8,
    pub velocity: u8,
    pub reason: PlaybackMissReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SamplePlaybackResolution {
    Intent { intent: SamplePlaybackIntent },
    Miss { miss: SamplePlaybackMiss },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadAssignmentChange {
    Cleared,
    Restored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramEditField {
    Pad,
    Level,
    Pan,
    Tune,
}

impl Default for ProgramEditField {
    fn default() -> Self {
        Self::Pad
    }
}

impl ProgramEditField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pad => "pad",
            Self::Level => "level",
            Self::Pan => "pan",
            Self::Tune => "tune",
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Pad => Self::Tune,
            Self::Level => Self::Pad,
            Self::Pan => Self::Level,
            Self::Tune => Self::Pan,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Pad => Self::Level,
            Self::Level => Self::Pan,
            Self::Pan => Self::Tune,
            Self::Tune => Self::Pad,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiSettingsField {
    InputChannel,
    BaseNote,
}

impl Default for MidiSettingsField {
    fn default() -> Self {
        Self::InputChannel
    }
}

impl MidiSettingsField {
    pub fn label(self) -> &'static str {
        match self {
            Self::InputChannel => "input_channel",
            Self::BaseNote => "base_note",
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::InputChannel => Self::BaseNote,
            Self::BaseNote => Self::InputChannel,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::InputChannel => Self::BaseNote,
            Self::BaseNote => Self::InputChannel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupField {
    Metronome,
    CountInBars,
    LcdContrast,
}

impl Default for SetupField {
    fn default() -> Self {
        Self::Metronome
    }
}

impl SetupField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Metronome => "metronome",
            Self::CountInBars => "count_in_bars",
            Self::LcdContrast => "lcd_contrast",
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Metronome => Self::LcdContrast,
            Self::CountInBars => Self::Metronome,
            Self::LcdContrast => Self::CountInBars,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Metronome => Self::CountInBars,
            Self::CountInBars => Self::LcdContrast,
            Self::LcdContrast => Self::Metronome,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupPreferences {
    pub metronome_enabled: bool,
    pub count_in_bars: u8,
    pub lcd_contrast: u8,
}

impl Default for SetupPreferences {
    fn default() -> Self {
        Self {
            metronome_enabled: true,
            count_in_bars: 0,
            lcd_contrast: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SongEditField {
    Step,
    Sequence,
    Repeats,
}

impl Default for SongEditField {
    fn default() -> Self {
        Self::Step
    }
}

impl SongEditField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Sequence => "sequence",
            Self::Repeats => "repeats",
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Step => Self::Repeats,
            Self::Sequence => Self::Step,
            Self::Repeats => Self::Sequence,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Step => Self::Sequence,
            Self::Sequence => Self::Repeats,
            Self::Repeats => Self::Step,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskOperation {
    SaveProject,
    LoadProject,
}

impl Default for DiskOperation {
    fn default() -> Self {
        Self::SaveProject
    }
}

impl DiskOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::SaveProject => "save_project",
            Self::LoadProject => "load_project",
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            Self::SaveProject => "Save Project",
            Self::LoadProject => "Load Project",
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::SaveProject => Self::LoadProject,
            Self::LoadProject => Self::SaveProject,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::SaveProject => Self::LoadProject,
            Self::LoadProject => Self::SaveProject,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceEvent {
    pub selected_track: u8,
    pub pad_bank: PadBank,
    pub pad_number: u8,
    pub velocity: u8,
    pub tick: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback: Option<SamplePlaybackIntent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SongStep {
    pub sequence_index: u8,
    pub repeats: u8,
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
    LocateStart,
    ToggleLoop,
    PadBankA,
    PadBankB,
    PadBankC,
    PadBankD,
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
    Press {
        control: PanelControl,
    },
    Release {
        control: PanelControl,
    },
    TurnDataWheel {
        delta: i32,
    },
    StrikePad {
        bank: PadBank,
        pad: u8,
        velocity: u8,
    },
    MidiNoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    MidiNoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    Tick {
        micros: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MachineOutput {
    LcdChanged,
    ModeChanged {
        mode: Mode,
    },
    TransportChanged {
        playing: bool,
        recording: bool,
    },
    PlayheadLocated {
        tick: u64,
    },
    LoopChanged {
        enabled: bool,
    },
    BankChanged {
        bank: PadBank,
    },
    PadTriggered {
        bank: PadBank,
        pad: u8,
        velocity: u8,
    },
    SamplePlaybackIntent {
        intent: SamplePlaybackIntent,
    },
    SamplePlaybackMiss {
        miss: SamplePlaybackMiss,
    },
    SampleSelected {
        entry: SampleCatalogEntry,
    },
    MidiNoteMapped {
        channel: u8,
        note: u8,
        bank: PadBank,
        pad: u8,
        velocity: u8,
    },
    MidiInputIgnored {
        reason: String,
    },
    MidiSettingsChanged {
        input_channel: Option<u8>,
        base_note: u8,
        selected_field: MidiSettingsField,
    },
    DiskOperationSelected {
        operation: DiskOperation,
    },
    DiskOperationRequested {
        operation: DiskOperation,
    },
    SetupPreferencesChanged {
        preferences: SetupPreferences,
        selected_field: SetupField,
    },
    SongStepSelected {
        index: usize,
        step: SongStep,
    },
    SongStepChanged {
        index: usize,
        field: SongEditField,
        step: SongStep,
    },
    SongStepInserted {
        index: usize,
        step: SongStep,
    },
    SongStepDeleted {
        index: usize,
        step: SongStep,
    },
    PadAssignmentChanged {
        bank: PadBank,
        pad: u8,
        action: PadAssignmentChange,
        assignment: Option<PadAssignment>,
    },
    PadParameterChanged {
        bank: PadBank,
        pad: u8,
        parameter: ProgramEditField,
        value: i16,
        assignment: PadAssignment,
    },
    SequenceEventRecorded {
        event: SequenceEvent,
    },
    SequenceEventsErased {
        selected_track: u8,
        count: u64,
        events: Vec<SequenceEvent>,
    },
    SequenceEventPlayed {
        event: SequenceEvent,
    },
    Ignored {
        reason: String,
    },
}
