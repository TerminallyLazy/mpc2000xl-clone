use anyhow::{Context, Result, bail};
use mpc_core::{
    HardwareEvent, MainScreenField, Mode, MpcCore, ProgramPad, SamplePlaybackResolution,
    SequenceEvent,
};
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
    #[serde(default)]
    pub selected_field: Option<MainScreenField>,
    #[serde(default)]
    pub selected_track: Option<u8>,
    #[serde(default)]
    pub tempo_bpm_x100: Option<u32>,
    #[serde(default)]
    pub sequence_index: Option<u8>,
    #[serde(default)]
    pub sequence_name: Option<String>,
    #[serde(default)]
    pub bar_count: Option<u16>,
    #[serde(default)]
    pub recorded_event_count: Option<usize>,
    #[serde(default)]
    pub playhead_ticks: Option<u64>,
    #[serde(default)]
    pub last_recorded_event: Option<SequenceEvent>,
    #[serde(default)]
    pub current_program_index: Option<u8>,
    #[serde(default)]
    pub current_program_name: Option<String>,
    #[serde(default)]
    pub pad_assignment_count: Option<usize>,
    #[serde(default)]
    pub selected_program_pad: Option<ProgramPad>,
    #[serde(default)]
    pub last_playback: Option<SamplePlaybackResolution>,
    #[serde(default)]
    pub last_recorded_sample_id: Option<String>,
    #[serde(default)]
    pub last_recorded_sample_name: Option<String>,
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

    if let Some(selected_field) = fixture.expect.selected_field {
        if state.selected_main_field != selected_field {
            details.push(format!(
                "selected_field mismatch: expected {:?}, got {:?}",
                selected_field, state.selected_main_field
            ));
        }
    }

    if let Some(selected_track) = fixture.expect.selected_track {
        if state.selected_track != selected_track {
            details.push(format!(
                "selected_track mismatch: expected {}, got {}",
                selected_track, state.selected_track
            ));
        }
    }

    if let Some(tempo_bpm_x100) = fixture.expect.tempo_bpm_x100 {
        if state.tempo_bpm_x100 != tempo_bpm_x100 {
            details.push(format!(
                "tempo_bpm_x100 mismatch: expected {}, got {}",
                tempo_bpm_x100, state.tempo_bpm_x100
            ));
        }
    }

    if let Some(sequence_index) = fixture.expect.sequence_index {
        if state.sequence_index != sequence_index {
            details.push(format!(
                "sequence_index mismatch: expected {}, got {}",
                sequence_index, state.sequence_index
            ));
        }
    }

    if let Some(sequence_name) = &fixture.expect.sequence_name {
        if state.sequence_name != *sequence_name {
            details.push(format!(
                "sequence_name mismatch: expected {}, got {}",
                sequence_name, state.sequence_name
            ));
        }
    }

    if let Some(bar_count) = fixture.expect.bar_count {
        if state.bar_count != bar_count {
            details.push(format!(
                "bar_count mismatch: expected {}, got {}",
                bar_count, state.bar_count
            ));
        }
    }

    if let Some(recorded_event_count) = fixture.expect.recorded_event_count {
        if state.recorded_events.len() != recorded_event_count {
            details.push(format!(
                "recorded_event_count mismatch: expected {}, got {}",
                recorded_event_count,
                state.recorded_events.len()
            ));
        }
    }

    if let Some(playhead_ticks) = fixture.expect.playhead_ticks {
        if state.playhead_ticks != playhead_ticks {
            details.push(format!(
                "playhead_ticks mismatch: expected {}, got {}",
                playhead_ticks, state.playhead_ticks
            ));
        }
    }

    if let Some(last_recorded_event) = &fixture.expect.last_recorded_event {
        if state.recorded_events.last() != Some(last_recorded_event) {
            details.push(format!(
                "last_recorded_event mismatch: expected {:?}, got {:?}",
                last_recorded_event,
                state.recorded_events.last()
            ));
        }
    }

    if let Some(current_program_index) = fixture.expect.current_program_index {
        if state.current_program.index != current_program_index {
            details.push(format!(
                "current_program_index mismatch: expected {}, got {}",
                current_program_index, state.current_program.index
            ));
        }
    }

    if let Some(current_program_name) = &fixture.expect.current_program_name {
        if state.current_program.name != *current_program_name {
            details.push(format!(
                "current_program_name mismatch: expected {}, got {}",
                current_program_name, state.current_program.name
            ));
        }
    }

    if let Some(pad_assignment_count) = fixture.expect.pad_assignment_count {
        if state.current_program.pad_assignments.len() != pad_assignment_count {
            details.push(format!(
                "pad_assignment_count mismatch: expected {}, got {}",
                pad_assignment_count,
                state.current_program.pad_assignments.len()
            ));
        }
    }

    if let Some(selected_program_pad) = fixture.expect.selected_program_pad {
        if state.selected_program_pad != selected_program_pad {
            details.push(format!(
                "selected_program_pad mismatch: expected {:?}, got {:?}",
                selected_program_pad, state.selected_program_pad
            ));
        }
    }

    if let Some(last_playback) = &fixture.expect.last_playback {
        if state.last_playback.as_ref() != Some(last_playback) {
            details.push(format!(
                "last_playback mismatch: expected {:?}, got {:?}",
                last_playback, state.last_playback
            ));
        }
    }

    if let Some(last_recorded_sample_id) = &fixture.expect.last_recorded_sample_id {
        let actual = state
            .recorded_events
            .last()
            .and_then(|event| event.playback.as_ref())
            .map(|intent| intent.sample_id.as_str());
        if actual != Some(last_recorded_sample_id.as_str()) {
            details.push(format!(
                "last_recorded_sample_id mismatch: expected {}, got {:?}",
                last_recorded_sample_id, actual
            ));
        }
    }

    if let Some(last_recorded_sample_name) = &fixture.expect.last_recorded_sample_name {
        let actual = state
            .recorded_events
            .last()
            .and_then(|event| event.playback.as_ref())
            .map(|intent| intent.sample_name.as_str());
        if actual != Some(last_recorded_sample_name.as_str()) {
            details.push(format!(
                "last_recorded_sample_name mismatch: expected {}, got {:?}",
                last_recorded_sample_name, actual
            ));
        }
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
