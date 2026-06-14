use serde::{Deserialize, Serialize};

const SAMPLE_BASE_LENGTH_FRAMES: u32 = 48_000;
const SAMPLE_LENGTH_STEP_FRAMES: u32 = 1_200;
pub const RECORDED_SAMPLE_LENGTH_FRAMES: u32 = 44_100;
pub const IMPORTED_SAMPLE_LENGTH_FRAMES: u32 = 88_200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Main,
    Program,
    Sample,
    Trim,
    Song,
    Midi,
    TimingCorrect,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleSourceKind {
    #[default]
    Generated,
    Recorded,
    Imported,
}

impl SampleSourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Generated => "Generated",
            Self::Recorded => "Recorded",
            Self::Imported => "Imported",
        }
    }

    pub fn is_generated(&self) -> bool {
        *self == Self::Generated
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntheticSample {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "SampleSourceKind::is_generated")]
    pub source_kind: SampleSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_frames: Option<u32>,
}

impl SyntheticSample {
    pub fn effective_length_frames(&self, source_pad: ProgramPad) -> u32 {
        match self.source_kind {
            SampleSourceKind::Generated => generated_sample_length_frames(source_pad),
            SampleSourceKind::Recorded | SampleSourceKind::Imported => self
                .length_frames
                .unwrap_or_else(|| generated_sample_length_frames(source_pad)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleCatalogEntry {
    pub index: usize,
    pub count: usize,
    pub sample: SyntheticSample,
    #[serde(default)]
    pub source_kind: SampleSourceKind,
    pub source_pad: ProgramPad,
    pub start_frame: u32,
    pub end_frame: u32,
    pub window_length_frames: u32,
    pub length_frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleTrim {
    pub sample_id: String,
    pub start_frame: u32,
    pub end_frame: u32,
}

impl SampleTrim {
    pub fn window_length_frames(&self) -> u32 {
        sample_window_length_frames(self.start_frame, self.end_frame)
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    pub tune_cents: i16,
    pub start_frame: u32,
    pub end_frame: u32,
    pub window_length_frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiOutputIntent {
    pub selected_track: u8,
    pub program_index: u8,
    pub program_name: String,
    pub bank: PadBank,
    pub pad_number: u8,
    pub source_sample_id: String,
    pub source_sample_name: String,
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
}

impl<'de> Deserialize<'de> for SamplePlaybackIntent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawSamplePlaybackIntent {
            selected_track: u8,
            program_index: u8,
            program_name: String,
            bank: PadBank,
            pad_number: u8,
            sample_id: String,
            sample_name: String,
            velocity: u8,
            level: u8,
            pan: i8,
            #[serde(default)]
            tune_cents: i16,
            #[serde(default)]
            start_frame: Option<u32>,
            #[serde(default)]
            end_frame: Option<u32>,
            #[serde(default)]
            window_length_frames: Option<u32>,
        }

        let raw = RawSamplePlaybackIntent::deserialize(deserializer)?;
        let provided_window_fields = [
            raw.start_frame.is_some(),
            raw.end_frame.is_some(),
            raw.window_length_frames.is_some(),
        ]
        .into_iter()
        .filter(|provided| *provided)
        .count();
        let (start_frame, end_frame, window_length_frames) = match (
            raw.start_frame,
            raw.end_frame,
            raw.window_length_frames,
        ) {
            (Some(start_frame), Some(end_frame), Some(window_length_frames)) => {
                (start_frame, end_frame, window_length_frames)
            }
            (None, None, None) => {
                let length_frames = generated_sample_length_frames(ProgramPad {
                    bank: raw.bank,
                    pad_number: raw.pad_number,
                });
                (0, length_frames.saturating_sub(1), length_frames)
            }
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "start_frame, end_frame, and window_length_frames must be provided together; got {provided_window_fields} fields"
                )));
            }
        };

        Ok(Self {
            selected_track: raw.selected_track,
            program_index: raw.program_index,
            program_name: raw.program_name,
            bank: raw.bank,
            pad_number: raw.pad_number,
            sample_id: raw.sample_id,
            sample_name: raw.sample_name,
            velocity: raw.velocity,
            level: raw.level,
            pan: raw.pan,
            tune_cents: raw.tune_cents,
            start_frame,
            end_frame,
            window_length_frames,
        })
    }
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
    Assigned,
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
pub enum TrimEditField {
    Start,
    End,
}

impl Default for TrimEditField {
    fn default() -> Self {
        Self::Start
    }
}

impl TrimEditField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Start => Self::Start,
            Self::End => Self::Start,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::End => Self::End,
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
pub enum TimingCorrectDivision {
    Off,
    Eighth,
    EighthTriplet,
    Sixteenth,
    SixteenthTriplet,
    ThirtySecond,
}

impl Default for TimingCorrectDivision {
    fn default() -> Self {
        Self::Off
    }
}

impl TimingCorrectDivision {
    pub const ORDERED: [Self; 6] = [
        Self::Off,
        Self::Eighth,
        Self::EighthTriplet,
        Self::Sixteenth,
        Self::SixteenthTriplet,
        Self::ThirtySecond,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Eighth => "1/8",
            Self::EighthTriplet => "1/8T",
            Self::Sixteenth => "1/16",
            Self::SixteenthTriplet => "1/16T",
            Self::ThirtySecond => "1/32",
        }
    }

    pub fn grid_ticks(self) -> Option<u64> {
        match self {
            Self::Off => None,
            Self::Eighth => Some(48),
            Self::EighthTriplet => Some(32),
            Self::Sixteenth => Some(24),
            Self::SixteenthTriplet => Some(16),
            Self::ThirtySecond => Some(12),
        }
    }

    pub fn uses_swing(self) -> bool {
        matches!(self, Self::Eighth | Self::Sixteenth | Self::ThirtySecond)
    }

    pub fn previous(self) -> Self {
        let index = Self::ORDERED
            .iter()
            .position(|division| *division == self)
            .expect("division should be in ordered list");
        Self::ORDERED[(index + Self::ORDERED.len() - 1) % Self::ORDERED.len()]
    }

    pub fn next(self) -> Self {
        let index = Self::ORDERED
            .iter()
            .position(|division| *division == self)
            .expect("division should be in ordered list");
        Self::ORDERED[(index + 1) % Self::ORDERED.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimingCorrectField {
    Division,
    Swing,
}

impl Default for TimingCorrectField {
    fn default() -> Self {
        Self::Division
    }
}

impl TimingCorrectField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Division => "division",
            Self::Swing => "swing",
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Division => Self::Swing,
            Self::Swing => Self::Division,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Division => Self::Swing,
            Self::Swing => Self::Division,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingCorrectSettings {
    pub division: TimingCorrectDivision,
    pub swing_percent: u8,
}

impl Default for TimingCorrectSettings {
    fn default() -> Self {
        Self {
            division: TimingCorrectDivision::Off,
            swing_percent: 50,
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
pub struct CountInClickIntent {
    pub count_in_tick: u64,
    pub bar_index: u8,
    pub beat_index: u8,
    pub accent: bool,
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
    TimingCorrect,
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
    CountInStarted {
        total_ticks: u64,
        bars: u8,
    },
    CountInCompleted {
        total_ticks: u64,
    },
    MetronomeClick {
        intent: CountInClickIntent,
    },
    PlayheadLocated {
        tick: u64,
    },
    LoopChanged {
        enabled: bool,
    },
    TrackMuteChanged {
        track: u8,
        muted: bool,
        muted_tracks: Vec<u8>,
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
    MidiOutputIntent {
        intent: MidiOutputIntent,
    },
    SamplePlaybackMiss {
        miss: SamplePlaybackMiss,
    },
    SampleSelected {
        entry: SampleCatalogEntry,
    },
    SampleMetadataCreated {
        sample: SyntheticSample,
        source_kind: SampleSourceKind,
        target_pad: ProgramPad,
        length_frames: u32,
    },
    SampleTrimChanged {
        sample_id: String,
        start_frame: u32,
        end_frame: u32,
        window_length_frames: u32,
        selected_field: TrimEditField,
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
    TimingCorrectChanged {
        settings: TimingCorrectSettings,
        selected_field: TimingCorrectField,
    },
    TimingCorrectApplied {
        original_tick: u64,
        quantized_tick: u64,
        division: TimingCorrectDivision,
        swing_percent: u8,
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

pub fn generated_sample_length_frames(pad: ProgramPad) -> u32 {
    SAMPLE_BASE_LENGTH_FRAMES
        .saturating_add(pad_linear_index(pad).saturating_mul(SAMPLE_LENGTH_STEP_FRAMES))
}

pub fn sample_window_length_frames(start_frame: u32, end_frame: u32) -> u32 {
    end_frame.saturating_sub(start_frame).saturating_add(1)
}

fn pad_linear_index(pad: ProgramPad) -> u32 {
    let bank_offset = match pad.bank {
        PadBank::A => 0,
        PadBank::B => 16,
        PadBank::C => 32,
        PadBank::D => 48,
    };
    bank_offset + u32::from(pad.pad_number.saturating_sub(1))
}
