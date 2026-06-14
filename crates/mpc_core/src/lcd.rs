use serde::{Deserialize, Serialize};

use crate::events::{
    DiskOperation, MidiSettingsField, PadAssignment, Program, ProgramEditField, ProgramPad,
    SampleCatalogEntry, SetupField, SetupPreferences, SongEditField, SongStep, TrimEditField,
};
use crate::state::MainScreenField;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcdFrame {
    pub title: String,
    pub lines: [String; 4],
    pub soft_keys: [String; 6],
}

impl LcdFrame {
    pub fn main_screen(
        sequence_index: u8,
        sequence_name: &str,
        selected_track: u8,
        selected_track_muted: bool,
        muted_track_count: usize,
        program_name: &str,
        tempo_bpm_x100: u32,
        playing: bool,
        recording: bool,
        loop_enabled: bool,
        bar_count: u16,
        sequence_length_ticks: u64,
        selected_field: MainScreenField,
        playhead_ticks: u64,
        recorded_event_count: usize,
    ) -> Self {
        let tempo = format!("{}.{:02}", tempo_bpm_x100 / 100, tempo_bpm_x100 % 100);
        let status = match (playing, recording) {
            (true, true) => "REC",
            (true, false) => "PLAY",
            (false, true) => "ARM",
            (false, false) => "STOP",
        };
        let loop_text = if loop_enabled { "LP" } else { "--" };
        let mute_text = if selected_track_muted { "on" } else { "off" };
        let marker = |field| {
            if selected_field == field { ">" } else { " " }
        };

        Self {
            title: "MAIN".to_string(),
            lines: [
                format!(
                    "{}Seq {:02} {}",
                    marker(MainScreenField::Sequence),
                    sequence_index,
                    sequence_name
                ),
                format!(
                    "{}Trk {:02} Mute {mute_text}/{:02} Pgm {}",
                    marker(MainScreenField::Track),
                    selected_track,
                    muted_track_count.min(99),
                    program_name
                ),
                format!(
                    "{}Tempo {:>6} BPM {status} {loop_text}",
                    marker(MainScreenField::Tempo),
                    tempo
                ),
                format!(
                    "{}Bars {:03} L{:06} T{:06} E{:03} {}",
                    marker(MainScreenField::Bars),
                    bar_count,
                    sequence_length_ticks.min(999_999),
                    playhead_ticks.min(999_999),
                    recorded_event_count.min(999),
                    selected_field.label(),
                ),
            ],
            soft_keys: [
                "TrList".to_string(),
                "Track+".to_string(),
                "Track-".to_string(),
                "Mute".to_string(),
                "Erase".to_string(),
                "Edit".to_string(),
            ],
        }
    }

    pub fn program_screen(
        program: &Program,
        selected_pad: ProgramPad,
        selected_field: ProgramEditField,
        assignment: Option<&PadAssignment>,
    ) -> Self {
        let pad_label = format!(
            "{}{:02}",
            selected_pad.bank.label(),
            selected_pad.pad_number
        );
        let marker = |field| {
            if selected_field == field { ">" } else { " " }
        };
        let (assignment_line, sample_line, mix_line) = match assignment {
            Some(assignment) => (
                format!(
                    "{}Pad {pad_label} -> {}",
                    marker(ProgramEditField::Pad),
                    assignment.sample.name
                ),
                format!("Sample {}", assignment.sample.id),
                format!(
                    "{}Level {:03} {}Pan {} {}Tune {}",
                    marker(ProgramEditField::Level),
                    assignment.level,
                    marker(ProgramEditField::Pan),
                    pan_text(assignment.pan),
                    marker(ProgramEditField::Tune),
                    tune_text(assignment.tune_cents)
                ),
            ),
            None => (
                format!(
                    "{}Pad {pad_label} -> unassigned",
                    marker(ProgramEditField::Pad)
                ),
                "Sample none".to_string(),
                format!(
                    "{}Level --- {}Pan -- {}Tune ----",
                    marker(ProgramEditField::Level),
                    marker(ProgramEditField::Pan),
                    marker(ProgramEditField::Tune)
                ),
            ),
        };

        Self {
            title: "PROGRAM".to_string(),
            lines: [
                format!(
                    "Program {:02} {} Edit {}",
                    program.index,
                    program.name,
                    selected_field.label()
                ),
                assignment_line,
                sample_line,
                mix_line,
            ],
            soft_keys: [
                "Clear".to_string(),
                "Assign".to_string(),
                "F3".to_string(),
                "F4".to_string(),
                "F5".to_string(),
                "F6".to_string(),
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

    pub fn song_screen(
        steps: &[SongStep],
        selected_index: usize,
        selected_field: SongEditField,
        sequence_name: &str,
    ) -> Self {
        let selected_step = steps[selected_index];
        let display_index = selected_index + 1;
        let sequence_display_index = u16::from(selected_step.sequence_index) + 1;
        let marker = |field| {
            if selected_field == field { ">" } else { " " }
        };

        Self {
            title: "SONG".to_string(),
            lines: [
                format!(
                    "Step {:02}/{:02} Edit {}",
                    display_index.min(99),
                    steps.len().min(99),
                    selected_field.label()
                ),
                format!(
                    "{}Step {:02}  {}Seq {:02}",
                    marker(SongEditField::Step),
                    display_index.min(99),
                    marker(SongEditField::Sequence),
                    sequence_display_index
                ),
                format!("Sequence {sequence_name}"),
                format!(
                    "{}Repeats {:02}",
                    marker(SongEditField::Repeats),
                    selected_step.repeats
                ),
            ],
            soft_keys: [
                "F1".to_string(),
                "Insert".to_string(),
                "Delete".to_string(),
                "F4".to_string(),
                "F5".to_string(),
                "F6".to_string(),
            ],
        }
    }

    pub fn sample_screen(selected_sample: Option<&SampleCatalogEntry>) -> Self {
        match selected_sample {
            Some(entry) => Self {
                title: "SAMPLE".to_string(),
                lines: [
                    format!(
                        "Sample {:02}/{:02} {}",
                        entry.index.min(99),
                        entry.count.min(99),
                        entry.sample.name
                    ),
                    format!("ID {}", entry.sample.id),
                    format!(
                        "Pad {} Len {:06}",
                        pad_label(entry.source_pad),
                        entry.length_frames.min(999_999)
                    ),
                    "Metadata only - no audio bytes".to_string(),
                ],
                soft_keys: sample_soft_keys(),
            },
            None => Self {
                title: "SAMPLE".to_string(),
                lines: [
                    "Sample 00/00 empty catalog".to_string(),
                    "ID none".to_string(),
                    "Pad -- Len ------".to_string(),
                    "Metadata only - no audio bytes".to_string(),
                ],
                soft_keys: sample_soft_keys(),
            },
        }
    }

    pub fn trim_screen(
        selected_sample: Option<&SampleCatalogEntry>,
        selected_field: TrimEditField,
    ) -> Self {
        let marker = |field| {
            if selected_field == field { ">" } else { " " }
        };
        match selected_sample {
            Some(entry) => Self {
                title: "TRIM".to_string(),
                lines: [
                    format!(
                        "Trim {:02}/{:02} {} Edit {}",
                        entry.index.min(99),
                        entry.count.min(99),
                        entry.sample.name,
                        selected_field.label()
                    ),
                    format!(
                        "{}Start {:06} {}End {:06}",
                        marker(TrimEditField::Start),
                        entry.start_frame.min(999_999),
                        marker(TrimEditField::End),
                        entry.end_frame.min(999_999)
                    ),
                    format!(
                        "Window {:06} Src {}",
                        entry.window_length_frames.min(999_999),
                        pad_label(entry.source_pad)
                    ),
                    "Metadata only - no waveform".to_string(),
                ],
                soft_keys: trim_soft_keys(),
            },
            None => Self {
                title: "TRIM".to_string(),
                lines: [
                    format!("Trim 00/00 empty catalog Edit {}", selected_field.label()),
                    ">Start ------  End ------".to_string(),
                    "Window ------ Src --".to_string(),
                    "Metadata only - no waveform".to_string(),
                ],
                soft_keys: trim_soft_keys(),
            },
        }
    }

    pub fn midi_screen(
        input_channel: Option<u8>,
        base_note: u8,
        selected_field: MidiSettingsField,
    ) -> Self {
        let marker = |field| {
            if selected_field == field { ">" } else { " " }
        };
        let range_end = base_note.saturating_add(15);

        Self {
            title: "MIDI".to_string(),
            lines: [
                format!(
                    "{}Input {}",
                    marker(MidiSettingsField::InputChannel),
                    midi_input_channel_text(input_channel)
                ),
                format!(
                    "{}Base {:03} Range {:03}-{:03}",
                    marker(MidiSettingsField::BaseNote),
                    base_note,
                    base_note,
                    range_end
                ),
                "Host MIDI I/O: off".to_string(),
                format!("Sim note-on only  Edit {}", selected_field.label()),
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

    pub fn disk_screen(selected_operation: DiskOperation) -> Self {
        let marker = |operation| {
            if selected_operation == operation {
                ">"
            } else {
                " "
            }
        };

        Self {
            title: "DISK".to_string(),
            lines: [
                format!(
                    "Op {}Save Project  {}Load Project",
                    marker(DiskOperation::SaveProject),
                    marker(DiskOperation::LoadProject)
                ),
                "Project file JSON only".to_string(),
                "Virtual disk via host path".to_string(),
                "No MPC disk/image formats".to_string(),
            ],
            soft_keys: [
                "F1".to_string(),
                "Save".to_string(),
                "Load".to_string(),
                "F4".to_string(),
                "F5".to_string(),
                "F6".to_string(),
            ],
        }
    }

    pub fn setup_screen(preferences: SetupPreferences, selected_field: SetupField) -> Self {
        let marker = |field| {
            if selected_field == field { ">" } else { " " }
        };

        Self {
            title: "SETUP".to_string(),
            lines: [
                format!(
                    "{}Metronome {}",
                    marker(SetupField::Metronome),
                    on_off_text(preferences.metronome_enabled)
                ),
                format!(
                    "{}Count-in bars {}",
                    marker(SetupField::CountInBars),
                    preferences.count_in_bars
                ),
                format!(
                    "{}LCD contrast {:02}",
                    marker(SetupField::LcdContrast),
                    preferences.lcd_contrast
                ),
                format!("Edit {}", selected_field.label()),
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

fn sample_soft_keys() -> [String; 6] {
    [
        "Prev".to_string(),
        "Next".to_string(),
        "F3".to_string(),
        "F4".to_string(),
        "F5".to_string(),
        "Trim".to_string(),
    ]
}

fn trim_soft_keys() -> [String; 6] {
    [
        "Prev".to_string(),
        "Next".to_string(),
        "F3".to_string(),
        "F4".to_string(),
        "F5".to_string(),
        "Sample".to_string(),
    ]
}

fn midi_input_channel_text(input_channel: Option<u8>) -> String {
    match input_channel {
        Some(channel) => format!("Ch {channel:02}"),
        None => "Omni".to_string(),
    }
}

fn on_off_text(enabled: bool) -> &'static str {
    if enabled { "On" } else { "Off" }
}

fn pad_label(pad: ProgramPad) -> String {
    format!("{}{:02}", pad.bank.label(), pad.pad_number)
}

fn pan_text(pan: i8) -> String {
    match pan.cmp(&0) {
        std::cmp::Ordering::Less => format!("L{}", pan.abs()),
        std::cmp::Ordering::Equal => "C".to_string(),
        std::cmp::Ordering::Greater => format!("R{pan}"),
    }
}

fn tune_text(tune_cents: i16) -> String {
    format!("{tune_cents:+04}")
}
