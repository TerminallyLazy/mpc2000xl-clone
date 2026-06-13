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
pub struct PadAssignment {
    pub pad: ProgramPad,
    pub sample: SyntheticSample,
    pub level: u8,
    pub pan: i8,
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
    PadAssignmentChanged {
        bank: PadBank,
        pad: u8,
        action: PadAssignmentChange,
        assignment: Option<PadAssignment>,
    },
    SequenceEventRecorded {
        event: SequenceEvent,
    },
    Ignored {
        reason: String,
    },
}
