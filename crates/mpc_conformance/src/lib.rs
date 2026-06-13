use anyhow::{Context, Result, bail};
use mpc_audio::{AudioRenderSettings, AudioSourceKind, ChannelBalance, render_intent};
use mpc_core::{
    HardwareEvent, MainScreenField, Mode, MpcCore, MpcState, PROJECT_SNAPSHOT_VERSION,
    ProgramEditField, ProgramPad, SamplePlaybackResolution, SequenceEvent,
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
    #[serde(default)]
    pub project_round_trip: Option<ProjectRoundTripExpectation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRoundTripExpectation {
    #[serde(default)]
    pub post_restore_events: Vec<HardwareEvent>,
    #[serde(default)]
    pub restore_playhead_ticks: Option<u64>,
    #[serde(default)]
    pub restore_playhead_tick_remainder: Option<u64>,
    #[serde(default)]
    pub clear_last_playback_before_restore: bool,
    pub expect: ExpectedState,
    #[serde(default)]
    pub expect_snapshot_version: Option<u16>,
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
    pub loop_enabled: Option<bool>,
    #[serde(default)]
    pub sequence_length_ticks: Option<u64>,
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
    pub selected_program_edit_field: Option<ProgramEditField>,
    #[serde(default)]
    pub last_playback: Option<SamplePlaybackResolution>,
    #[serde(default)]
    pub last_recorded_sample_id: Option<String>,
    #[serde(default)]
    pub last_recorded_sample_name: Option<String>,
    #[serde(default)]
    pub last_audio_render: Option<ExpectedAudioRender>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedAudioRender {
    pub settings: AudioRenderSettings,
    pub sample_rate_hz: u32,
    pub frame_count: usize,
    pub source_sample_id: String,
    pub source_sample_name: String,
    pub selected_track: u8,
    pub program_index: u8,
    pub program_name: String,
    pub bank: mpc_core::PadBank,
    pub pad_number: u8,
    #[serde(default)]
    pub tune_cents: i16,
    pub peak_left: i16,
    pub peak_right: i16,
    pub peak_amplitude: i16,
    pub channel_balance: ChannelBalance,
    pub source_kind: AudioSourceKind,
    pub loaded_audio_byte_count: usize,
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

    let mut details = Vec::new();
    validate_expected_state(&mut details, "", core.state(), &fixture.expect);
    if let Some(project_round_trip) = &fixture.project_round_trip {
        validate_project_round_trip(&mut details, &core, project_round_trip);
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

fn validate_project_round_trip(
    details: &mut Vec<String>,
    core: &MpcCore,
    expected: &ProjectRoundTripExpectation,
) {
    let mut snapshot = core.export_project_snapshot();
    let expected_version = expected
        .expect_snapshot_version
        .unwrap_or(PROJECT_SNAPSHOT_VERSION);
    if snapshot.version != expected_version {
        details.push(format!(
            "project_round_trip.snapshot_version mismatch: expected {}, got {}",
            expected_version, snapshot.version
        ));
    }

    if let Some(playhead_ticks) = expected.restore_playhead_ticks {
        snapshot.machine.playhead_ticks = playhead_ticks;
    }
    if let Some(playhead_tick_remainder) = expected.restore_playhead_tick_remainder {
        snapshot.machine.playhead_tick_remainder = playhead_tick_remainder;
    }
    if expected.clear_last_playback_before_restore {
        snapshot.machine.last_playback = None;
    }

    let json = match serde_json::to_string_pretty(&snapshot) {
        Ok(json) => json,
        Err(error) => {
            details.push(format!("project_round_trip.encode error: {error}"));
            return;
        }
    };

    let mut restored = MpcCore::new();
    if let Err(error) = restored.restore_project_json(&json) {
        details.push(format!("project_round_trip.restore error: {error}"));
        return;
    }

    for event in &expected.post_restore_events {
        restored.dispatch(event.clone());
    }

    validate_expected_state(
        details,
        "project_round_trip.",
        restored.state(),
        &expected.expect,
    );
}

fn validate_expected_state(
    details: &mut Vec<String>,
    prefix: &str,
    state: &MpcState,
    expected: &ExpectedState,
) {
    if state.mode != expected.mode {
        details.push(format!(
            "{prefix}mode mismatch: expected {:?}, got {:?}",
            expected.mode, state.mode
        ));
    }

    if state.lcd.title != expected.lcd_title {
        details.push(format!(
            "{prefix}lcd title mismatch: expected {}, got {}",
            expected.lcd_title, state.lcd.title
        ));
    }

    if state.playing != expected.playing {
        details.push(format!(
            "{prefix}playing mismatch: expected {}, got {}",
            expected.playing, state.playing
        ));
    }

    if state.recording != expected.recording {
        details.push(format!(
            "{prefix}recording mismatch: expected {}, got {}",
            expected.recording, state.recording
        ));
    }

    if state.event_count != expected.event_count {
        details.push(format!(
            "{prefix}event_count mismatch: expected {}, got {}",
            expected.event_count, state.event_count
        ));
    }

    if let Some(selected_field) = expected.selected_field {
        if state.selected_main_field != selected_field {
            details.push(format!(
                "{prefix}selected_field mismatch: expected {:?}, got {:?}",
                selected_field, state.selected_main_field
            ));
        }
    }

    if let Some(selected_track) = expected.selected_track {
        if state.selected_track != selected_track {
            details.push(format!(
                "{prefix}selected_track mismatch: expected {}, got {}",
                selected_track, state.selected_track
            ));
        }
    }

    if let Some(tempo_bpm_x100) = expected.tempo_bpm_x100 {
        if state.tempo_bpm_x100 != tempo_bpm_x100 {
            details.push(format!(
                "{prefix}tempo_bpm_x100 mismatch: expected {}, got {}",
                tempo_bpm_x100, state.tempo_bpm_x100
            ));
        }
    }

    if let Some(sequence_index) = expected.sequence_index {
        if state.sequence_index != sequence_index {
            details.push(format!(
                "{prefix}sequence_index mismatch: expected {}, got {}",
                sequence_index, state.sequence_index
            ));
        }
    }

    if let Some(sequence_name) = &expected.sequence_name {
        if state.sequence_name != *sequence_name {
            details.push(format!(
                "{prefix}sequence_name mismatch: expected {}, got {}",
                sequence_name, state.sequence_name
            ));
        }
    }

    if let Some(bar_count) = expected.bar_count {
        if state.bar_count != bar_count {
            details.push(format!(
                "{prefix}bar_count mismatch: expected {}, got {}",
                bar_count, state.bar_count
            ));
        }
    }

    if let Some(loop_enabled) = expected.loop_enabled {
        if state.loop_enabled != loop_enabled {
            details.push(format!(
                "{prefix}loop_enabled mismatch: expected {}, got {}",
                loop_enabled, state.loop_enabled
            ));
        }
    }

    if let Some(sequence_length_ticks) = expected.sequence_length_ticks {
        if state.sequence_length_ticks() != sequence_length_ticks {
            details.push(format!(
                "{prefix}sequence_length_ticks mismatch: expected {}, got {}",
                sequence_length_ticks,
                state.sequence_length_ticks()
            ));
        }
    }

    if let Some(recorded_event_count) = expected.recorded_event_count {
        if state.recorded_events.len() != recorded_event_count {
            details.push(format!(
                "{prefix}recorded_event_count mismatch: expected {}, got {}",
                recorded_event_count,
                state.recorded_events.len()
            ));
        }
    }

    if let Some(playhead_ticks) = expected.playhead_ticks {
        if state.playhead_ticks != playhead_ticks {
            details.push(format!(
                "{prefix}playhead_ticks mismatch: expected {}, got {}",
                playhead_ticks, state.playhead_ticks
            ));
        }
    }

    if let Some(last_recorded_event) = &expected.last_recorded_event {
        if state.recorded_events.last() != Some(last_recorded_event) {
            details.push(format!(
                "{prefix}last_recorded_event mismatch: expected {:?}, got {:?}",
                last_recorded_event,
                state.recorded_events.last()
            ));
        }
    }

    if let Some(current_program_index) = expected.current_program_index {
        if state.current_program.index != current_program_index {
            details.push(format!(
                "{prefix}current_program_index mismatch: expected {}, got {}",
                current_program_index, state.current_program.index
            ));
        }
    }

    if let Some(current_program_name) = &expected.current_program_name {
        if state.current_program.name != *current_program_name {
            details.push(format!(
                "{prefix}current_program_name mismatch: expected {}, got {}",
                current_program_name, state.current_program.name
            ));
        }
    }

    if let Some(pad_assignment_count) = expected.pad_assignment_count {
        if state.current_program.pad_assignments.len() != pad_assignment_count {
            details.push(format!(
                "{prefix}pad_assignment_count mismatch: expected {}, got {}",
                pad_assignment_count,
                state.current_program.pad_assignments.len()
            ));
        }
    }

    if let Some(selected_program_pad) = expected.selected_program_pad {
        if state.selected_program_pad != selected_program_pad {
            details.push(format!(
                "{prefix}selected_program_pad mismatch: expected {:?}, got {:?}",
                selected_program_pad, state.selected_program_pad
            ));
        }
    }

    if let Some(selected_program_edit_field) = expected.selected_program_edit_field {
        if state.selected_program_edit_field != selected_program_edit_field {
            details.push(format!(
                "{prefix}selected_program_edit_field mismatch: expected {:?}, got {:?}",
                selected_program_edit_field, state.selected_program_edit_field
            ));
        }
    }

    if let Some(last_playback) = &expected.last_playback {
        if state.last_playback.as_ref() != Some(last_playback) {
            details.push(format!(
                "{prefix}last_playback mismatch: expected {:?}, got {:?}",
                last_playback, state.last_playback
            ));
        }
    }

    if let Some(last_recorded_sample_id) = &expected.last_recorded_sample_id {
        let actual = state
            .recorded_events
            .last()
            .and_then(|event| event.playback.as_ref())
            .map(|intent| intent.sample_id.as_str());
        if actual != Some(last_recorded_sample_id.as_str()) {
            details.push(format!(
                "{prefix}last_recorded_sample_id mismatch: expected {}, got {:?}",
                last_recorded_sample_id, actual
            ));
        }
    }

    if let Some(last_recorded_sample_name) = &expected.last_recorded_sample_name {
        let actual = state
            .recorded_events
            .last()
            .and_then(|event| event.playback.as_ref())
            .map(|intent| intent.sample_name.as_str());
        if actual != Some(last_recorded_sample_name.as_str()) {
            details.push(format!(
                "{prefix}last_recorded_sample_name mismatch: expected {}, got {:?}",
                last_recorded_sample_name, actual
            ));
        }
    }

    if let Some(expected_audio_render) = &expected.last_audio_render {
        validate_expected_audio_render(
            details,
            state.last_playback.as_ref(),
            expected_audio_render,
        );
    }
}

fn validate_expected_audio_render(
    details: &mut Vec<String>,
    last_playback: Option<&SamplePlaybackResolution>,
    expected: &ExpectedAudioRender,
) {
    let Some(SamplePlaybackResolution::Intent { intent }) = last_playback else {
        details.push(format!(
            "last_audio_render mismatch: expected renderable SamplePlaybackIntent, got {last_playback:?}"
        ));
        return;
    };

    let rendered = match render_intent(intent, expected.settings) {
        Ok(rendered) => rendered,
        Err(error) => {
            details.push(format!("last_audio_render render error: {error}"));
            return;
        }
    };
    let summary = rendered.summary;

    push_mismatch(
        details,
        "last_audio_render.sample_rate_hz",
        &expected.sample_rate_hz,
        &summary.sample_rate_hz,
    );
    push_mismatch(
        details,
        "last_audio_render.frame_count",
        &expected.frame_count,
        &summary.frame_count,
    );
    push_mismatch(
        details,
        "last_audio_render.frames.len",
        &expected.frame_count,
        &rendered.frames.len(),
    );
    push_mismatch(
        details,
        "last_audio_render.source_sample_id",
        &expected.source_sample_id,
        &summary.source_sample_id,
    );
    push_mismatch(
        details,
        "last_audio_render.source_sample_name",
        &expected.source_sample_name,
        &summary.source_sample_name,
    );
    push_mismatch(
        details,
        "last_audio_render.selected_track",
        &expected.selected_track,
        &summary.selected_track,
    );
    push_mismatch(
        details,
        "last_audio_render.program_index",
        &expected.program_index,
        &summary.program_index,
    );
    push_mismatch(
        details,
        "last_audio_render.program_name",
        &expected.program_name,
        &summary.program_name,
    );
    push_mismatch(
        details,
        "last_audio_render.bank",
        &expected.bank,
        &summary.bank,
    );
    push_mismatch(
        details,
        "last_audio_render.pad_number",
        &expected.pad_number,
        &summary.pad_number,
    );
    push_mismatch(
        details,
        "last_audio_render.tune_cents",
        &expected.tune_cents,
        &summary.tune_cents,
    );
    push_mismatch(
        details,
        "last_audio_render.peak_left",
        &expected.peak_left,
        &summary.peak_left,
    );
    push_mismatch(
        details,
        "last_audio_render.peak_right",
        &expected.peak_right,
        &summary.peak_right,
    );
    push_mismatch(
        details,
        "last_audio_render.peak_amplitude",
        &expected.peak_amplitude,
        &summary.peak_amplitude,
    );
    push_mismatch(
        details,
        "last_audio_render.channel_balance",
        &expected.channel_balance,
        &summary.channel_balance,
    );
    push_mismatch(
        details,
        "last_audio_render.source_kind",
        &expected.source_kind,
        &summary.source_kind,
    );
    push_mismatch(
        details,
        "last_audio_render.loaded_audio_byte_count",
        &expected.loaded_audio_byte_count,
        &summary.loaded_audio_byte_count,
    );
}

fn push_mismatch<T>(details: &mut Vec<String>, label: &str, expected: &T, actual: &T)
where
    T: std::fmt::Debug + PartialEq,
{
    if expected != actual {
        details.push(format!(
            "{label} mismatch: expected {expected:?}, got {actual:?}"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpc_core::{PadBank, SamplePlaybackIntent};

    #[test]
    fn invalid_audio_render_settings_are_reported_as_fixture_details() {
        let frame_count = mpc_audio::MAX_RENDER_FRAMES + 1;
        let playback = SamplePlaybackResolution::Intent {
            intent: SamplePlaybackIntent {
                selected_track: 1,
                program_index: 1,
                program_name: "Program01".to_string(),
                bank: PadBank::A,
                pad_number: 1,
                sample_id: "synthetic_a_01".to_string(),
                sample_name: "SYN-A01".to_string(),
                velocity: 100,
                level: 100,
                pan: 0,
                tune_cents: 0,
            },
        };
        let expected = ExpectedAudioRender {
            settings: AudioRenderSettings {
                sample_rate_hz: mpc_audio::DEFAULT_SAMPLE_RATE_HZ,
                frame_count,
            },
            sample_rate_hz: mpc_audio::DEFAULT_SAMPLE_RATE_HZ,
            frame_count,
            source_sample_id: "synthetic_a_01".to_string(),
            source_sample_name: "SYN-A01".to_string(),
            selected_track: 1,
            program_index: 1,
            program_name: "Program01".to_string(),
            bank: PadBank::A,
            pad_number: 1,
            tune_cents: 0,
            peak_left: 0,
            peak_right: 0,
            peak_amplitude: 0,
            channel_balance: ChannelBalance::Center,
            source_kind: AudioSourceKind::RightsSafeGenerated,
            loaded_audio_byte_count: 0,
        };
        let mut details = Vec::new();

        validate_expected_audio_render(&mut details, Some(&playback), &expected);

        assert_eq!(
            details,
            vec![format!(
                "last_audio_render render error: frame count {frame_count} exceeds maximum {}",
                mpc_audio::MAX_RENDER_FRAMES
            )]
        );
    }
}
