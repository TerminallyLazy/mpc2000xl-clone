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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceEvent {
    pub selected_track: u8,
    pub pad_bank: PadBank,
    pub pad_number: u8,
    pub velocity: u8,
    pub tick: u64,
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
    SequenceEventRecorded {
        event: SequenceEvent,
    },
    Ignored {
        reason: String,
    },
}
