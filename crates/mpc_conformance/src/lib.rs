use anyhow::{Context, Result, bail};
use mpc_audio::{
    AudioRenderKind, AudioRenderSettings, AudioSourceKind, CaptureAudioBackend, ChannelBalance,
    HostAudioEngine, HostAudioEvent, HostAudioMode, HostAudioState, HostAudioVoiceSummary,
    RuntimeSampleLibrary, load_wav_sample_payload, render_intent_with_runtime_samples,
};
use mpc_core::{
    DiskOperation, HardwareEvent, MachineOutput, MainScreenField, MidiSettingsField, Mode, MpcCore,
    MpcState, PROJECT_SNAPSHOT_VERSION, PadBank, ProgramEditField, ProgramPad,
    SamplePlaybackResolution, SampleSourceKind, SequenceEvent, SetupField, SongEditField, SongStep,
    TimingCorrectDivision, TimingCorrectField, TrimEditField,
};
use mpc_storage::{PROJECT_FILE_SUFFIX, load_project_file_with_report, save_project_file};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static PROJECT_ROUND_TRIP_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const PROJECT_FILE_RIGHTS_BOUNDARY: &str = "metadata_only_no_audio_bytes";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fixture {
    pub id: String,
    pub name: String,
    pub source_refs: Vec<String>,
    pub events: Vec<HardwareEvent>,
    #[serde(default)]
    pub runtime_wav_imports: Vec<RuntimeWavImport>,
    #[serde(default)]
    pub post_runtime_wav_import_events: Vec<HardwareEvent>,
    #[serde(default)]
    pub host_audio: Option<HostAudioFixture>,
    #[serde(default)]
    pub expect_output_sequence: Vec<MachineOutput>,
    #[serde(default)]
    pub expect_sample_metadata_created: Vec<ExpectedSampleMetadataCreated>,
    pub expect: ExpectedState,
    #[serde(default)]
    pub project_round_trip: Option<ProjectRoundTripExpectation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedSampleMetadataCreated {
    pub sample_id: String,
    pub sample_name: String,
    pub source_kind: SampleSourceKind,
    pub target_pad: ProgramPad,
    pub length_frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeWavImport {
    pub sample_name: String,
    pub channels: u16,
    pub sample_rate_hz: u32,
    pub samples: Vec<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAudioFixture {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub settings: AudioRenderSettings,
    #[serde(default = "default_capture_history")]
    pub capture_history: usize,
    #[serde(default = "default_host_audio_fixture_voice_limit")]
    pub voice_limit: usize,
    #[serde(default)]
    pub advance_voice_frames: Vec<usize>,
    pub expect: ExpectedHostAudioState,
}

fn default_capture_history() -> usize {
    16
}

fn default_host_audio_fixture_voice_limit() -> usize {
    32
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedHostAudioState {
    #[serde(default)]
    pub mode: Option<HostAudioMode>,
    #[serde(default)]
    pub backend_name: Option<String>,
    #[serde(default)]
    pub render_settings: Option<AudioRenderSettings>,
    #[serde(default)]
    pub queued_render_count: Option<u64>,
    #[serde(default)]
    pub played_render_count: Option<u64>,
    #[serde(default)]
    pub voice_limit: Option<usize>,
    #[serde(default)]
    pub active_voice_count: Option<usize>,
    #[serde(default)]
    pub completed_voice_count: Option<u64>,
    #[serde(default)]
    pub stolen_voice_count: Option<u64>,
    #[serde(default)]
    pub released_voice_count: Option<u64>,
    #[serde(default)]
    pub choked_voice_count: Option<u64>,
    #[serde(default)]
    pub active_voices: Option<Vec<HostAudioVoiceSummary>>,
    #[serde(default)]
    pub capture_count: Option<usize>,
    #[serde(default)]
    pub capture_frame_counts: Option<Vec<usize>>,
    #[serde(default)]
    pub capture_render_kinds: Option<Vec<AudioRenderKind>>,
    #[serde(default)]
    pub capture_source_kinds: Option<Vec<AudioSourceKind>>,
    #[serde(default)]
    pub capture_source_sample_names: Option<Vec<String>>,
    #[serde(default)]
    pub capture_count_in_ticks: Option<Vec<Option<u64>>>,
    #[serde(default)]
    pub capture_accents: Option<Vec<Option<bool>>>,
    #[serde(default)]
    pub last_event_type: Option<ExpectedHostAudioEventType>,
    #[serde(default)]
    pub last_event_backend_name: Option<String>,
    #[serde(default)]
    pub last_event_render_kind: Option<AudioRenderKind>,
    #[serde(default)]
    pub last_event_source_kind: Option<AudioSourceKind>,
    #[serde(default)]
    pub last_event_frame_count: Option<usize>,
    #[serde(default)]
    pub last_event_voice_id: Option<u64>,
    #[serde(default)]
    pub last_event_stolen_voice_id: Option<u64>,
    #[serde(default)]
    pub last_event_choked_voice_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedHostAudioEventType {
    Ignored,
    Enqueued,
    Released,
    Failed,
}

struct HostAudioFixtureRunner {
    engine: HostAudioEngine<CaptureAudioBackend>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRoundTripExpectation {
    #[serde(default)]
    pub post_restore_events: Vec<HardwareEvent>,
    #[serde(default)]
    pub expect_post_restore_output_sequence: Vec<MachineOutput>,
    #[serde(default)]
    pub project_file_round_trip: bool,
    #[serde(default)]
    pub invalid_project_json_cases: Vec<InvalidProjectJsonCase>,
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
pub struct InvalidProjectJsonCase {
    pub name: String,
    pub mutation: ProjectJsonMutation,
    pub expect_error_contains: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProjectJsonMutation {
    UnsupportedVersion { version: u16 },
    InvalidRightsBoundary { rights_boundary: String },
    DuplicateFirstPadAssignment,
    UnknownRootField { field: String },
    EventCountLessThanRecordedEvents,
    LastPlaybackWithZeroEventCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedMidiInputChannel {
    Omni,
    Channel(u8),
}

impl ExpectedMidiInputChannel {
    fn state_value(self) -> Option<u8> {
        match self {
            Self::Omni => None,
            Self::Channel(channel) => Some(channel),
        }
    }
}

impl Serialize for ExpectedMidiInputChannel {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Omni => serializer.serialize_str("omni"),
            Self::Channel(channel) => serializer.serialize_u8(*channel),
        }
    }
}

impl<'de> Deserialize<'de> for ExpectedMidiInputChannel {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ExpectedMidiInputChannelVisitor)
    }
}

struct ExpectedMidiInputChannelVisitor;

impl<'de> Visitor<'de> for ExpectedMidiInputChannelVisitor {
    type Value = ExpectedMidiInputChannel;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(r#"the string "omni" or a MIDI input channel number 1..=16"#)
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.eq_ignore_ascii_case("omni") {
            Ok(ExpectedMidiInputChannel::Omni)
        } else {
            Err(E::custom(format!(
                r#"invalid MIDI input channel "{value}", expected "omni" or 1..=16"#
            )))
        }
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(&value)
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        channel_from_u64(value).map_err(E::custom)
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        let value = u64::try_from(value).map_err(E::custom)?;
        channel_from_u64(value).map_err(E::custom)
    }
}

fn deserialize_optional_expected_midi_input_channel<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<ExpectedMidiInputChannel>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(OptionalExpectedMidiInputChannelVisitor)
}

struct OptionalExpectedMidiInputChannelVisitor;

impl<'de> Visitor<'de> for OptionalExpectedMidiInputChannelVisitor {
    type Value = Option<ExpectedMidiInputChannel>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            r#"an omitted field, the string "omni", or a MIDI input channel number 1..=16"#,
        )
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Err(E::custom(
            r#"midi_input_channel must be omitted to skip validation or set to "omni"/1..=16"#,
        ))
    }

    fn visit_none<E>(self) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_unit()
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        ExpectedMidiInputChannelVisitor.visit_str(value).map(Some)
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(&value)
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        ExpectedMidiInputChannelVisitor.visit_u64(value).map(Some)
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        ExpectedMidiInputChannelVisitor.visit_i64(value).map(Some)
    }
}

fn channel_from_u64(value: u64) -> std::result::Result<ExpectedMidiInputChannel, &'static str> {
    let channel = u8::try_from(value).map_err(|_| "MIDI input channel must be 1..=16")?;
    if (1..=16).contains(&channel) {
        Ok(ExpectedMidiInputChannel::Channel(channel))
    } else {
        Err("MIDI input channel must be 1..=16")
    }
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
    pub muted_tracks: Option<Vec<u8>>,
    #[serde(default)]
    pub pad_bank: Option<PadBank>,
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
    pub count_in_active: Option<bool>,
    #[serde(default)]
    pub count_in_ticks_remaining: Option<u64>,
    #[serde(default)]
    pub count_in_total_ticks: Option<u64>,
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
    pub selected_sample_index: Option<usize>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_expected_midi_input_channel"
    )]
    pub midi_input_channel: Option<ExpectedMidiInputChannel>,
    #[serde(default)]
    pub midi_base_note: Option<u8>,
    #[serde(default)]
    pub selected_midi_settings_field: Option<MidiSettingsField>,
    #[serde(default)]
    pub timing_correct_division: Option<TimingCorrectDivision>,
    #[serde(default)]
    pub timing_correct_swing_percent: Option<u8>,
    #[serde(default)]
    pub selected_timing_correct_field: Option<TimingCorrectField>,
    #[serde(default)]
    pub selected_disk_operation: Option<DiskOperation>,
    #[serde(default)]
    pub selected_setup_field: Option<SetupField>,
    #[serde(default)]
    pub setup_metronome_enabled: Option<bool>,
    #[serde(default)]
    pub setup_count_in_bars: Option<u8>,
    #[serde(default)]
    pub setup_lcd_contrast: Option<u8>,
    #[serde(default)]
    pub song_step_count: Option<usize>,
    #[serde(default)]
    pub selected_song_step_index: Option<usize>,
    #[serde(default)]
    pub selected_song_edit_field: Option<SongEditField>,
    #[serde(default)]
    pub song_step: Option<SongStep>,
    #[serde(default)]
    pub midi_mapped_note_range: Option<[u8; 2]>,
    #[serde(default)]
    pub midi_host_io_enabled: Option<bool>,
    #[serde(default)]
    pub sample_catalog_count: Option<usize>,
    #[serde(default)]
    pub selected_sample_id: Option<String>,
    #[serde(default)]
    pub selected_sample_name: Option<String>,
    #[serde(default)]
    pub selected_sample_source_kind: Option<SampleSourceKind>,
    #[serde(default)]
    pub selected_sample_length_frames: Option<u32>,
    #[serde(default)]
    pub selected_trim_edit_field: Option<TrimEditField>,
    #[serde(default)]
    pub selected_sample_start_frame: Option<u32>,
    #[serde(default)]
    pub selected_sample_end_frame: Option<u32>,
    #[serde(default)]
    pub selected_sample_window_length_frames: Option<u32>,
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
    #[serde(default)]
    pub mute_group: u8,
    #[serde(default)]
    pub start_frame: Option<u32>,
    #[serde(default)]
    pub end_frame: Option<u32>,
    #[serde(default)]
    pub window_length_frames: Option<u32>,
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
    let mut runtime_samples = RuntimeSampleLibrary::default();
    let mut output_sequence = Vec::new();
    let mut details = Vec::new();
    let mut host_audio = build_host_audio_runner(&mut details, fixture);

    for event in &fixture.events {
        let outputs = core.dispatch(event.clone());
        route_host_audio_outputs(&mut details, &mut host_audio, &outputs, &runtime_samples);
        output_sequence.extend(outputs);
    }

    process_runtime_wav_imports(
        &mut details,
        fixture,
        &mut core,
        &mut runtime_samples,
        &mut output_sequence,
    );

    for event in &fixture.post_runtime_wav_import_events {
        let outputs = core.dispatch(event.clone());
        route_host_audio_outputs(&mut details, &mut host_audio, &outputs, &runtime_samples);
        output_sequence.extend(outputs);
    }

    if let (Some(config), Some(runner)) = (&fixture.host_audio, host_audio.as_mut()) {
        for frame_count in &config.advance_voice_frames {
            runner.engine.advance_voice_frames(*frame_count);
        }
    }

    validate_expected_output_sequence(&mut details, &output_sequence, fixture);
    validate_expected_sample_metadata_created(&mut details, &output_sequence, fixture);
    validate_expected_state(
        &mut details,
        "",
        core.state(),
        &fixture.expect,
        &runtime_samples,
    );
    if let Some(project_round_trip) = &fixture.project_round_trip {
        validate_project_round_trip(&mut details, &core, project_round_trip);
    }
    validate_host_audio(&mut details, fixture, host_audio.as_ref());

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

fn build_host_audio_runner(
    details: &mut Vec<String>,
    fixture: &Fixture,
) -> Option<HostAudioFixtureRunner> {
    let Some(config) = &fixture.host_audio else {
        return None;
    };

    let backend = CaptureAudioBackend::new(config.capture_history);
    let engine =
        match HostAudioEngine::new_with_voice_limit(backend, config.settings, config.voice_limit) {
            Ok(mut engine) => {
                engine.set_enabled(config.enabled);
                engine
            }
            Err(error) => {
                details.push(format!("host_audio setup error: {error}"));
                return None;
            }
        };

    Some(HostAudioFixtureRunner { engine })
}

fn route_host_audio_outputs(
    _details: &mut Vec<String>,
    host_audio: &mut Option<HostAudioFixtureRunner>,
    outputs: &[MachineOutput],
    runtime_samples: &RuntimeSampleLibrary,
) {
    let Some(runner) = host_audio.as_mut() else {
        return;
    };

    for output in outputs {
        match output {
            MachineOutput::SamplePlaybackIntent { intent } => {
                runner
                    .engine
                    .play_intent_with_runtime_samples_and_render_summary(intent, runtime_samples);
            }
            MachineOutput::MetronomeClick { intent } => {
                runner
                    .engine
                    .play_count_in_click_with_render_summary(intent);
            }
            MachineOutput::SampleReleaseIntent { intent } => {
                runner.engine.release_intent(intent);
            }
            _ => {}
        }
    }
}

fn process_runtime_wav_imports(
    details: &mut Vec<String>,
    fixture: &Fixture,
    core: &mut MpcCore,
    runtime_samples: &mut RuntimeSampleLibrary,
    output_sequence: &mut Vec<MachineOutput>,
) {
    for (index, import) in fixture.runtime_wav_imports.iter().enumerate() {
        let path = runtime_wav_fixture_path(&fixture.id, index);
        let result = process_runtime_wav_import(&path, import, core, runtime_samples);

        if path.exists() {
            if let Err(error) = fs::remove_file(&path) {
                details.push(format!(
                    "runtime_wav_imports[{index}] cleanup error for {}: {error}",
                    path.display()
                ));
            }
        }

        match result {
            Ok(outputs) => output_sequence.extend(outputs),
            Err(error) => details.push(format!("runtime_wav_imports[{index}] error: {error:#}")),
        }
    }
}

fn process_runtime_wav_import(
    path: &Path,
    import: &RuntimeWavImport,
    core: &mut MpcCore,
    runtime_samples: &mut RuntimeSampleLibrary,
) -> Result<Vec<MachineOutput>> {
    write_runtime_wav_fixture(path, import)?;
    let payload = load_wav_sample_payload(path)
        .with_context(|| format!("failed to load generated WAV fixture {}", path.display()))?;
    let length_frames = payload.length_frames_u32();
    let outputs =
        core.import_sample_metadata_for_selected_pad(import.sample_name.clone(), length_frames);
    let Some((sample_id, sample_name)) = outputs.iter().find_map(|output| {
        if let MachineOutput::SampleMetadataCreated { sample, .. } = output {
            Some((sample.id.clone(), sample.name.clone()))
        } else {
            None
        }
    }) else {
        bail!("runtime WAV import did not create sample metadata");
    };

    runtime_samples.insert(sample_id, sample_name, payload);
    Ok(outputs)
}

fn runtime_wav_fixture_path(fixture_id: &str, index: usize) -> PathBuf {
    let safe_fixture_id = fixture_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    std::env::temp_dir().join(format!(
        "mpc_conformance_{safe_fixture_id}_{}_{}_{}.wav",
        std::process::id(),
        index,
        nanos
    ))
}

fn write_runtime_wav_fixture(path: &Path, import: &RuntimeWavImport) -> Result<()> {
    if import.channels != 1 && import.channels != 2 {
        bail!(
            "runtime WAV fixture supports mono or stereo PCM16 only, got {} channels",
            import.channels
        );
    }
    if import.sample_rate_hz == 0 {
        bail!("runtime WAV fixture sample_rate_hz must be non-zero");
    }
    if import.samples.is_empty() {
        bail!("runtime WAV fixture samples must not be empty");
    }
    if import.samples.len() % usize::from(import.channels) != 0 {
        bail!(
            "runtime WAV fixture sample count {} is not divisible by channel count {}",
            import.samples.len(),
            import.channels
        );
    }

    let data_byte_count = import
        .samples
        .len()
        .checked_mul(std::mem::size_of::<i16>())
        .context("runtime WAV fixture data size overflow")?;
    let data_byte_count =
        u32::try_from(data_byte_count).context("runtime WAV fixture data exceeds u32 WAV size")?;
    let riff_chunk_size = 36u32
        .checked_add(data_byte_count)
        .context("runtime WAV fixture RIFF size overflow")?;
    let byte_rate = import
        .sample_rate_hz
        .checked_mul(u32::from(import.channels))
        .and_then(|value| value.checked_mul(2))
        .context("runtime WAV fixture byte rate overflow")?;
    let block_align = import
        .channels
        .checked_mul(2)
        .context("runtime WAV fixture block align overflow")?;

    let mut file = fs::File::create(path)
        .with_context(|| format!("failed to create runtime WAV fixture {}", path.display()))?;
    file.write_all(b"RIFF")?;
    file.write_all(&riff_chunk_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&import.channels.to_le_bytes())?;
    file.write_all(&import.sample_rate_hz.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_byte_count.to_le_bytes())?;
    for sample in &import.samples {
        file.write_all(&sample.to_le_bytes())?;
    }

    Ok(())
}

fn validate_expected_output_sequence(
    details: &mut Vec<String>,
    actual: &[MachineOutput],
    fixture: &Fixture,
) {
    validate_output_sequence(
        details,
        "output_sequence",
        actual,
        &fixture.expect_output_sequence,
    );
}

fn validate_output_sequence(
    details: &mut Vec<String>,
    label: &str,
    actual: &[MachineOutput],
    expected: &[MachineOutput],
) {
    if expected.is_empty() {
        return;
    }

    if actual != expected {
        details.push(format!(
            "{label} mismatch: expected {:?}, got {:?}",
            expected, actual
        ));
    }
}

fn validate_expected_sample_metadata_created(
    details: &mut Vec<String>,
    actual: &[MachineOutput],
    fixture: &Fixture,
) {
    if fixture.expect_sample_metadata_created.is_empty() {
        return;
    }

    let actual_created = actual
        .iter()
        .filter_map(|output| {
            if let MachineOutput::SampleMetadataCreated {
                sample,
                source_kind,
                target_pad,
                length_frames,
            } = output
            {
                Some(ExpectedSampleMetadataCreated {
                    sample_id: sample.id.clone(),
                    sample_name: sample.name.clone(),
                    source_kind: *source_kind,
                    target_pad: *target_pad,
                    length_frames: *length_frames,
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if actual_created != fixture.expect_sample_metadata_created {
        details.push(format!(
            "sample_metadata_created sequence mismatch: expected {:?}, got {:?}",
            fixture.expect_sample_metadata_created, actual_created
        ));
    }
}

fn validate_host_audio(
    details: &mut Vec<String>,
    fixture: &Fixture,
    runner: Option<&HostAudioFixtureRunner>,
) {
    let Some(config) = &fixture.host_audio else {
        return;
    };
    let Some(runner) = runner else {
        details.push("host_audio state mismatch: host audio runner was not created".to_string());
        return;
    };

    let state = runner.engine.state();
    let expected = &config.expect;

    if let Some(mode) = expected.mode {
        push_mismatch(details, "host_audio.mode", &mode, &state.mode);
    }
    if let Some(backend_name) = &expected.backend_name {
        push_mismatch(
            details,
            "host_audio.backend_name",
            backend_name,
            &state.backend_name,
        );
    }
    if let Some(render_settings) = expected.render_settings {
        push_mismatch(
            details,
            "host_audio.render_settings",
            &render_settings,
            &state.render_settings,
        );
    }
    if let Some(queued_render_count) = expected.queued_render_count {
        push_mismatch(
            details,
            "host_audio.queued_render_count",
            &queued_render_count,
            &state.queued_render_count,
        );
    }
    if let Some(played_render_count) = expected.played_render_count {
        push_mismatch(
            details,
            "host_audio.played_render_count",
            &played_render_count,
            &state.played_render_count,
        );
    }
    if let Some(voice_limit) = expected.voice_limit {
        push_mismatch(
            details,
            "host_audio.voice_limit",
            &voice_limit,
            &state.voice_limit,
        );
    }
    if let Some(active_voice_count) = expected.active_voice_count {
        push_mismatch(
            details,
            "host_audio.active_voice_count",
            &active_voice_count,
            &state.active_voice_count,
        );
    }
    if let Some(completed_voice_count) = expected.completed_voice_count {
        push_mismatch(
            details,
            "host_audio.completed_voice_count",
            &completed_voice_count,
            &state.completed_voice_count,
        );
    }
    if let Some(stolen_voice_count) = expected.stolen_voice_count {
        push_mismatch(
            details,
            "host_audio.stolen_voice_count",
            &stolen_voice_count,
            &state.stolen_voice_count,
        );
    }
    if let Some(released_voice_count) = expected.released_voice_count {
        push_mismatch(
            details,
            "host_audio.released_voice_count",
            &released_voice_count,
            &state.released_voice_count,
        );
    }
    if let Some(choked_voice_count) = expected.choked_voice_count {
        push_mismatch(
            details,
            "host_audio.choked_voice_count",
            &choked_voice_count,
            &state.choked_voice_count,
        );
    }
    if let Some(active_voices) = &expected.active_voices {
        push_mismatch(
            details,
            "host_audio.active_voices",
            active_voices,
            &state.active_voices,
        );
    }
    validate_expected_host_audio_captures(details, runner, expected);

    validate_expected_host_audio_last_event(details, &state, expected);
}

fn validate_expected_host_audio_captures(
    details: &mut Vec<String>,
    runner: &HostAudioFixtureRunner,
    expected: &ExpectedHostAudioState,
) {
    let captures = runner.engine.backend().captured_renders();

    if let Some(capture_count) = expected.capture_count {
        push_mismatch(
            details,
            "host_audio.capture_count",
            &capture_count,
            &captures.len(),
        );
    }
    if let Some(capture_frame_counts) = &expected.capture_frame_counts {
        let actual = captures
            .iter()
            .map(|capture| capture.frame_count)
            .collect::<Vec<_>>();
        push_mismatch(
            details,
            "host_audio.capture_frame_counts",
            capture_frame_counts,
            &actual,
        );
    }
    if let Some(capture_render_kinds) = &expected.capture_render_kinds {
        let actual = captures
            .iter()
            .map(|capture| capture.summary.render_kind)
            .collect::<Vec<_>>();
        push_mismatch(
            details,
            "host_audio.capture_render_kinds",
            capture_render_kinds,
            &actual,
        );
    }
    if let Some(capture_source_kinds) = &expected.capture_source_kinds {
        let actual = captures
            .iter()
            .map(|capture| capture.summary.source_kind)
            .collect::<Vec<_>>();
        push_mismatch(
            details,
            "host_audio.capture_source_kinds",
            capture_source_kinds,
            &actual,
        );
    }
    if let Some(capture_source_sample_names) = &expected.capture_source_sample_names {
        let actual = captures
            .iter()
            .map(|capture| capture.summary.source_sample_name.clone())
            .collect::<Vec<_>>();
        push_mismatch(
            details,
            "host_audio.capture_source_sample_names",
            capture_source_sample_names,
            &actual,
        );
    }
    if let Some(capture_count_in_ticks) = &expected.capture_count_in_ticks {
        let actual = captures
            .iter()
            .map(|capture| capture.summary.count_in_tick)
            .collect::<Vec<_>>();
        push_mismatch(
            details,
            "host_audio.capture_count_in_ticks",
            capture_count_in_ticks,
            &actual,
        );
    }
    if let Some(capture_accents) = &expected.capture_accents {
        let actual = captures
            .iter()
            .map(|capture| capture.summary.accent)
            .collect::<Vec<_>>();
        push_mismatch(
            details,
            "host_audio.capture_accents",
            capture_accents,
            &actual,
        );
    }
}

fn validate_expected_host_audio_last_event(
    details: &mut Vec<String>,
    state: &HostAudioState,
    expected: &ExpectedHostAudioState,
) {
    let Some(event) = &state.last_event else {
        if expected.last_event_type.is_some() {
            details.push("host_audio.last_event mismatch: expected event, got None".to_string());
        }
        return;
    };

    if let Some(event_type) = expected.last_event_type {
        push_mismatch(
            details,
            "host_audio.last_event_type",
            &event_type,
            &host_audio_event_type(event),
        );
    }
    if let Some(backend_name) = &expected.last_event_backend_name {
        push_mismatch(
            details,
            "host_audio.last_event_backend_name",
            backend_name,
            &host_audio_event_backend_name(event).to_string(),
        );
    }
    if let Some(render_kind) = expected.last_event_render_kind {
        let actual = host_audio_event_render_kind(event);
        if actual != Some(render_kind) {
            details.push(format!(
                "host_audio.last_event_render_kind mismatch: expected {:?}, got {:?}",
                render_kind, actual
            ));
        }
    }
    if let Some(source_kind) = expected.last_event_source_kind {
        let actual = host_audio_event_source_kind(event);
        if actual != Some(source_kind) {
            details.push(format!(
                "host_audio.last_event_source_kind mismatch: expected {:?}, got {:?}",
                source_kind, actual
            ));
        }
    }
    if let Some(frame_count) = expected.last_event_frame_count {
        let actual = host_audio_event_frame_count(event);
        if actual != Some(frame_count) {
            details.push(format!(
                "host_audio.last_event_frame_count mismatch: expected {}, got {:?}",
                frame_count, actual
            ));
        }
    }
    if let Some(voice_id) = expected.last_event_voice_id {
        let actual = host_audio_event_voice_id(event);
        if actual != Some(voice_id) {
            details.push(format!(
                "host_audio.last_event_voice_id mismatch: expected {}, got {:?}",
                voice_id, actual
            ));
        }
    }
    if let Some(stolen_voice_id) = expected.last_event_stolen_voice_id {
        let actual = host_audio_event_stolen_voice_id(event);
        if actual != Some(stolen_voice_id) {
            details.push(format!(
                "host_audio.last_event_stolen_voice_id mismatch: expected {}, got {:?}",
                stolen_voice_id, actual
            ));
        }
    }
    if let Some(choked_voice_count) = expected.last_event_choked_voice_count {
        let actual = host_audio_event_choked_voice_count(event);
        if actual != Some(choked_voice_count) {
            details.push(format!(
                "host_audio.last_event_choked_voice_count mismatch: expected {}, got {:?}",
                choked_voice_count, actual
            ));
        }
    }
}

fn host_audio_event_type(event: &HostAudioEvent) -> ExpectedHostAudioEventType {
    match event {
        HostAudioEvent::Ignored { .. } => ExpectedHostAudioEventType::Ignored,
        HostAudioEvent::Enqueued { .. } => ExpectedHostAudioEventType::Enqueued,
        HostAudioEvent::Released { .. } => ExpectedHostAudioEventType::Released,
        HostAudioEvent::Failed { .. } => ExpectedHostAudioEventType::Failed,
    }
}

fn host_audio_event_backend_name(event: &HostAudioEvent) -> &str {
    match event {
        HostAudioEvent::Ignored { backend_name, .. }
        | HostAudioEvent::Enqueued { backend_name, .. }
        | HostAudioEvent::Released { backend_name, .. }
        | HostAudioEvent::Failed { backend_name, .. } => backend_name,
    }
}

fn host_audio_event_render_kind(event: &HostAudioEvent) -> Option<AudioRenderKind> {
    match event {
        HostAudioEvent::Enqueued { receipt, .. } => Some(receipt.summary.render_kind),
        HostAudioEvent::Failed { summary, .. } => {
            summary.as_ref().map(|summary| summary.render_kind)
        }
        _ => None,
    }
}

fn host_audio_event_source_kind(event: &HostAudioEvent) -> Option<AudioSourceKind> {
    match event {
        HostAudioEvent::Enqueued { receipt, .. } => Some(receipt.summary.source_kind),
        HostAudioEvent::Failed { summary, .. } => {
            summary.as_ref().map(|summary| summary.source_kind)
        }
        _ => None,
    }
}

fn host_audio_event_frame_count(event: &HostAudioEvent) -> Option<usize> {
    match event {
        HostAudioEvent::Enqueued { receipt, .. } => Some(receipt.frame_count),
        _ => None,
    }
}

fn host_audio_event_voice_id(event: &HostAudioEvent) -> Option<u64> {
    match event {
        HostAudioEvent::Enqueued { receipt, .. } => receipt
            .voice_allocation
            .as_ref()
            .map(|allocation| allocation.voice_id),
        _ => None,
    }
}

fn host_audio_event_stolen_voice_id(event: &HostAudioEvent) -> Option<u64> {
    match event {
        HostAudioEvent::Enqueued { receipt, .. } => receipt
            .voice_allocation
            .as_ref()
            .and_then(|allocation| allocation.stolen_voice_id),
        _ => None,
    }
}

fn host_audio_event_choked_voice_count(event: &HostAudioEvent) -> Option<usize> {
    match event {
        HostAudioEvent::Enqueued { receipt, .. } => receipt
            .voice_allocation
            .as_ref()
            .map(|allocation| allocation.choked_voice_count),
        _ => None,
    }
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

    let mut post_restore_output_sequence = Vec::new();
    for event in &expected.post_restore_events {
        post_restore_output_sequence.extend(restored.dispatch(event.clone()));
    }
    validate_output_sequence(
        details,
        "project_round_trip.post_restore_output_sequence",
        &post_restore_output_sequence,
        &expected.expect_post_restore_output_sequence,
    );

    validate_expected_state(
        details,
        "project_round_trip.",
        restored.state(),
        &expected.expect,
        &RuntimeSampleLibrary::default(),
    );

    if expected.project_file_round_trip {
        validate_project_file_round_trip(details, core, expected);
    }

    validate_invalid_project_json_cases(details, core, &expected.invalid_project_json_cases);
}

fn validate_project_file_round_trip(
    details: &mut Vec<String>,
    core: &MpcCore,
    expected: &ProjectRoundTripExpectation,
) {
    let temp_dir = project_round_trip_temp_dir("project_file");
    let nested_parent = temp_dir.join("nested").join("projects");
    let project_path = nested_parent.join(format!("fixture{PROJECT_FILE_SUFFIX}"));

    let save_report = match save_project_file(core, &project_path) {
        Ok(report) => report,
        Err(error) => {
            details.push(format!("project_round_trip.file.save error: {error}"));
            cleanup_project_round_trip_temp_dir(details, &temp_dir);
            return;
        }
    };

    if !nested_parent.is_dir() {
        details.push(format!(
            "project_round_trip.file.parent_dir missing after save: {}",
            nested_parent.display()
        ));
    }
    if save_report.byte_count == 0 {
        details.push("project_round_trip.file.save byte_count is zero".to_string());
    }

    let saved_json = match fs::read_to_string(&project_path) {
        Ok(json) => json,
        Err(error) => {
            details.push(format!(
                "project_round_trip.file.read saved JSON error: {error}"
            ));
            cleanup_project_round_trip_temp_dir(details, &temp_dir);
            return;
        }
    };
    validate_project_file_json_boundary(details, &saved_json);

    let load = match load_project_file_with_report(&project_path) {
        Ok(load) => load,
        Err(error) => {
            details.push(format!("project_round_trip.file.load error: {error}"));
            cleanup_project_round_trip_temp_dir(details, &temp_dir);
            return;
        }
    };
    let expected_report_path = match fs::canonicalize(&project_path) {
        Ok(path) => Some(path),
        Err(error) => {
            details.push(format!(
                "project_round_trip.file.canonicalize path error: {error}"
            ));
            None
        }
    };

    if load.report.byte_count == 0 {
        details.push("project_round_trip.file.load byte_count is zero".to_string());
    }
    if save_report.byte_count != load.report.byte_count {
        details.push(format!(
            "project_round_trip.file.byte_count mismatch: save {}, load {}",
            save_report.byte_count, load.report.byte_count
        ));
    }
    if save_report.snapshot_version != load.report.snapshot_version {
        details.push(format!(
            "project_round_trip.file.snapshot_version mismatch: save {}, load {}",
            save_report.snapshot_version, load.report.snapshot_version
        ));
    }
    if save_report.path != load.report.path {
        details.push(format!(
            "project_round_trip.file.report_path mismatch: save {}, load {}",
            save_report.path.display(),
            load.report.path.display()
        ));
    }
    if let Some(expected_report_path) = expected_report_path {
        if save_report.path != expected_report_path {
            details.push(format!(
                "project_round_trip.file.report_path expectation mismatch: expected {}, got {}",
                expected_report_path.display(),
                save_report.path.display()
            ));
        }
        if !expected_report_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(PROJECT_FILE_SUFFIX))
        {
            details.push(format!(
                "project_round_trip.file.report_path suffix mismatch: {}",
                expected_report_path.display()
            ));
        }
    }

    let expected_version = expected
        .expect_snapshot_version
        .unwrap_or(PROJECT_SNAPSHOT_VERSION);
    if save_report.snapshot_version != expected_version {
        details.push(format!(
            "project_round_trip.file.snapshot_version expectation mismatch: expected {}, got {}",
            expected_version, save_report.snapshot_version
        ));
    }

    let mut restored = MpcCore::new();
    if let Err(error) = restored.restore_project_snapshot(load.snapshot) {
        details.push(format!("project_round_trip.file.restore error: {error}"));
        cleanup_project_round_trip_temp_dir(details, &temp_dir);
        return;
    }

    let mut post_load_output_sequence = Vec::new();
    for event in &expected.post_restore_events {
        post_load_output_sequence.extend(restored.dispatch(event.clone()));
    }
    validate_output_sequence(
        details,
        "project_round_trip.file.post_load_output_sequence",
        &post_load_output_sequence,
        &expected.expect_post_restore_output_sequence,
    );

    validate_expected_state(
        details,
        "project_round_trip.file.",
        restored.state(),
        &expected.expect,
        &RuntimeSampleLibrary::default(),
    );

    cleanup_project_round_trip_temp_dir(details, &temp_dir);
}

fn validate_project_file_json_boundary(details: &mut Vec<String>, saved_json: &str) {
    match serde_json::from_str::<serde_json::Value>(saved_json) {
        Ok(value) => {
            let rights_boundary = value
                .get("rights_boundary")
                .and_then(serde_json::Value::as_str);
            if rights_boundary != Some(PROJECT_FILE_RIGHTS_BOUNDARY) {
                details.push(format!(
                    "project_round_trip.file.saved_json rights_boundary mismatch: expected {:?}, got {:?}",
                    PROJECT_FILE_RIGHTS_BOUNDARY, rights_boundary
                ));
            }
            let mut forbidden_paths = Vec::new();
            collect_forbidden_project_json_keys(&value, "", &mut forbidden_paths);
            if !forbidden_paths.is_empty() {
                details.push(format!(
                    "project_round_trip.file.saved_json contains rights-unsafe fields: {:?}",
                    forbidden_paths
                ));
            }
        }
        Err(error) => {
            details.push(format!(
                "project_round_trip.file.saved_json parse error: {error}"
            ));
        }
    }
}

fn collect_forbidden_project_json_keys(
    value: &serde_json::Value,
    path: &str,
    forbidden_paths: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object {
                let child_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                if matches!(
                    key.as_str(),
                    "audio_bytes" | "sample_file_contents" | "file_path" | "sample_file_path"
                ) {
                    forbidden_paths.push(child_path.clone());
                }
                collect_forbidden_project_json_keys(child, &child_path, forbidden_paths);
            }
        }
        serde_json::Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                collect_forbidden_project_json_keys(
                    child,
                    &format!("{path}[{index}]"),
                    forbidden_paths,
                );
            }
        }
        _ => {}
    }
}

fn validate_invalid_project_json_cases(
    details: &mut Vec<String>,
    core: &MpcCore,
    cases: &[InvalidProjectJsonCase],
) {
    for case in cases {
        let mut value = match serde_json::to_value(core.export_project_snapshot()) {
            Ok(value) => value,
            Err(error) => {
                details.push(format!(
                    "project_round_trip.invalid_project_json.{} encode error: {error}",
                    case.name
                ));
                continue;
            }
        };

        if let Err(error) = apply_project_json_mutation(&mut value, &case.mutation) {
            details.push(format!(
                "project_round_trip.invalid_project_json.{} mutation error: {error}",
                case.name
            ));
            continue;
        }

        let json = match serde_json::to_string_pretty(&value) {
            Ok(json) => json,
            Err(error) => {
                details.push(format!(
                    "project_round_trip.invalid_project_json.{} encode mutated JSON error: {error}",
                    case.name
                ));
                continue;
            }
        };

        match MpcCore::from_project_json(&json) {
            Ok(_) => details.push(format!(
                "project_round_trip.invalid_project_json.{} unexpectedly passed",
                case.name
            )),
            Err(error) => {
                let error = error.to_string();
                if !error.contains(&case.expect_error_contains) {
                    details.push(format!(
                        "project_round_trip.invalid_project_json.{} error mismatch: expected substring {:?}, got {:?}",
                        case.name, case.expect_error_contains, error
                    ));
                }
            }
        }
    }
}

fn apply_project_json_mutation(
    value: &mut serde_json::Value,
    mutation: &ProjectJsonMutation,
) -> Result<()> {
    match mutation {
        ProjectJsonMutation::UnsupportedVersion { version } => {
            set_project_json_pointer(value, "/version", serde_json::json!(version))?;
        }
        ProjectJsonMutation::InvalidRightsBoundary { rights_boundary } => {
            set_project_json_pointer(
                value,
                "/rights_boundary",
                serde_json::json!(rights_boundary),
            )?;
        }
        ProjectJsonMutation::DuplicateFirstPadAssignment => {
            let assignments = value
                .pointer_mut("/program/pad_assignments")
                .and_then(serde_json::Value::as_array_mut)
                .context("program.pad_assignments must be an array")?;
            let first = assignments
                .first()
                .cloned()
                .context("program.pad_assignments must not be empty")?;
            assignments.push(first);
        }
        ProjectJsonMutation::UnknownRootField { field } => {
            let object = value
                .as_object_mut()
                .context("project snapshot root must be an object")?;
            object.insert(
                field.clone(),
                serde_json::json!("rights-unsafe fixture mutation payload"),
            );
        }
        ProjectJsonMutation::EventCountLessThanRecordedEvents => {
            let recorded_event_count = value
                .pointer("/sequence/recorded_events")
                .and_then(serde_json::Value::as_array)
                .context("sequence.recorded_events must be an array")?
                .len();
            if recorded_event_count == 0 {
                bail!("sequence.recorded_events must not be empty for this mutation");
            }
            set_project_json_pointer(
                value,
                "/machine/event_count",
                serde_json::json!(recorded_event_count - 1),
            )?;
        }
        ProjectJsonMutation::LastPlaybackWithZeroEventCount => {
            if value
                .pointer("/machine/last_playback")
                .is_none_or(|value| value.is_null())
            {
                bail!("machine.last_playback must be present for this mutation");
            }
            set_project_json_pointer(value, "/machine/event_count", serde_json::json!(0))?;
            set_project_json_pointer(value, "/sequence/recorded_events", serde_json::json!([]))?;
        }
    }

    Ok(())
}

fn set_project_json_pointer(
    value: &mut serde_json::Value,
    pointer: &str,
    replacement: serde_json::Value,
) -> Result<()> {
    let target = value
        .pointer_mut(pointer)
        .with_context(|| format!("project snapshot JSON pointer {pointer} must exist"))?;
    *target = replacement;
    Ok(())
}

fn project_round_trip_temp_dir(prefix: &str) -> PathBuf {
    let counter = PROJECT_ROUND_TRIP_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "mpc_conformance_{prefix}_{}_{}",
        std::process::id(),
        counter
    ))
}

fn cleanup_project_round_trip_temp_dir(details: &mut Vec<String>, temp_dir: &Path) {
    if !temp_dir.exists() {
        return;
    }

    if let Err(error) = fs::remove_dir_all(temp_dir) {
        details.push(format!(
            "project_round_trip.file.cleanup error for {}: {error}",
            temp_dir.display()
        ));
    }
}

fn validate_expected_state(
    details: &mut Vec<String>,
    prefix: &str,
    state: &MpcState,
    expected: &ExpectedState,
    runtime_samples: &RuntimeSampleLibrary,
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

    if let Some(muted_tracks) = &expected.muted_tracks {
        if &state.muted_tracks != muted_tracks {
            details.push(format!(
                "{prefix}muted_tracks mismatch: expected {:?}, got {:?}",
                muted_tracks, state.muted_tracks
            ));
        }
    }

    if let Some(pad_bank) = expected.pad_bank {
        if state.pad_bank != pad_bank {
            details.push(format!(
                "{prefix}pad_bank mismatch: expected {:?}, got {:?}",
                pad_bank, state.pad_bank
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

    if let Some(count_in_active) = expected.count_in_active {
        if state.count_in_active != count_in_active {
            details.push(format!(
                "{prefix}count_in_active mismatch: expected {}, got {}",
                count_in_active, state.count_in_active
            ));
        }
    }

    if let Some(count_in_ticks_remaining) = expected.count_in_ticks_remaining {
        if state.count_in_ticks_remaining != count_in_ticks_remaining {
            details.push(format!(
                "{prefix}count_in_ticks_remaining mismatch: expected {}, got {}",
                count_in_ticks_remaining, state.count_in_ticks_remaining
            ));
        }
    }

    if let Some(count_in_total_ticks) = expected.count_in_total_ticks {
        if state.count_in_total_ticks != count_in_total_ticks {
            details.push(format!(
                "{prefix}count_in_total_ticks mismatch: expected {}, got {}",
                count_in_total_ticks, state.count_in_total_ticks
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

    if let Some(midi_input_channel) = expected.midi_input_channel {
        let expected_channel = midi_input_channel.state_value();
        if state.midi_input_channel != expected_channel {
            details.push(format!(
                "{prefix}midi_input_channel mismatch: expected {:?}, got {:?}",
                expected_channel, state.midi_input_channel
            ));
        }
    }

    if let Some(midi_base_note) = expected.midi_base_note {
        if state.midi_base_note != midi_base_note {
            details.push(format!(
                "{prefix}midi_base_note mismatch: expected {}, got {}",
                midi_base_note, state.midi_base_note
            ));
        }
    }

    if let Some(selected_midi_settings_field) = expected.selected_midi_settings_field {
        if state.selected_midi_settings_field != selected_midi_settings_field {
            details.push(format!(
                "{prefix}selected_midi_settings_field mismatch: expected {:?}, got {:?}",
                selected_midi_settings_field, state.selected_midi_settings_field
            ));
        }
    }

    if let Some(timing_correct_division) = expected.timing_correct_division {
        if state.timing_correct.division != timing_correct_division {
            details.push(format!(
                "{prefix}timing_correct_division mismatch: expected {:?}, got {:?}",
                timing_correct_division, state.timing_correct.division
            ));
        }
    }

    if let Some(timing_correct_swing_percent) = expected.timing_correct_swing_percent {
        if state.timing_correct.swing_percent != timing_correct_swing_percent {
            details.push(format!(
                "{prefix}timing_correct_swing_percent mismatch: expected {}, got {}",
                timing_correct_swing_percent, state.timing_correct.swing_percent
            ));
        }
    }

    if let Some(selected_timing_correct_field) = expected.selected_timing_correct_field {
        if state.selected_timing_correct_field != selected_timing_correct_field {
            details.push(format!(
                "{prefix}selected_timing_correct_field mismatch: expected {:?}, got {:?}",
                selected_timing_correct_field, state.selected_timing_correct_field
            ));
        }
    }

    if let Some(selected_disk_operation) = expected.selected_disk_operation {
        if state.selected_disk_operation != selected_disk_operation {
            details.push(format!(
                "{prefix}selected_disk_operation mismatch: expected {:?}, got {:?}",
                selected_disk_operation, state.selected_disk_operation
            ));
        }
    }

    if let Some(selected_setup_field) = expected.selected_setup_field {
        if state.selected_setup_field != selected_setup_field {
            details.push(format!(
                "{prefix}selected_setup_field mismatch: expected {:?}, got {:?}",
                selected_setup_field, state.selected_setup_field
            ));
        }
    }

    if let Some(setup_metronome_enabled) = expected.setup_metronome_enabled {
        if state.setup_preferences.metronome_enabled != setup_metronome_enabled {
            details.push(format!(
                "{prefix}setup_metronome_enabled mismatch: expected {}, got {}",
                setup_metronome_enabled, state.setup_preferences.metronome_enabled
            ));
        }
    }

    if let Some(setup_count_in_bars) = expected.setup_count_in_bars {
        if state.setup_preferences.count_in_bars != setup_count_in_bars {
            details.push(format!(
                "{prefix}setup_count_in_bars mismatch: expected {}, got {}",
                setup_count_in_bars, state.setup_preferences.count_in_bars
            ));
        }
    }

    if let Some(setup_lcd_contrast) = expected.setup_lcd_contrast {
        if state.setup_preferences.lcd_contrast != setup_lcd_contrast {
            details.push(format!(
                "{prefix}setup_lcd_contrast mismatch: expected {}, got {}",
                setup_lcd_contrast, state.setup_preferences.lcd_contrast
            ));
        }
    }

    if let Some(song_step_count) = expected.song_step_count {
        if state.song_steps.len() != song_step_count {
            details.push(format!(
                "{prefix}song_step_count mismatch: expected {}, got {}",
                song_step_count,
                state.song_steps.len()
            ));
        }
    }

    if let Some(selected_song_step_index) = expected.selected_song_step_index {
        if state.selected_song_step_index != selected_song_step_index {
            details.push(format!(
                "{prefix}selected_song_step_index mismatch: expected {}, got {}",
                selected_song_step_index, state.selected_song_step_index
            ));
        }
    }

    if let Some(selected_song_edit_field) = expected.selected_song_edit_field {
        if state.selected_song_edit_field != selected_song_edit_field {
            details.push(format!(
                "{prefix}selected_song_edit_field mismatch: expected {:?}, got {:?}",
                selected_song_edit_field, state.selected_song_edit_field
            ));
        }
    }

    if let Some(song_step) = expected.song_step {
        let actual = state
            .song_steps
            .get(state.selected_song_step_index)
            .copied();
        if actual != Some(song_step) {
            details.push(format!(
                "{prefix}song_step mismatch: expected {:?}, got {:?}",
                song_step, actual
            ));
        }
    }

    if let Some([expected_start, expected_end]) = expected.midi_mapped_note_range {
        let actual_start = state.midi_base_note;
        let actual_end = state.midi_base_note.saturating_add(15);
        if [actual_start, actual_end] != [expected_start, expected_end] {
            details.push(format!(
                "{prefix}midi_mapped_note_range mismatch: expected {:?}, got {:?}",
                [expected_start, expected_end],
                [actual_start, actual_end]
            ));
        }
    }

    if let Some(midi_host_io_enabled) = expected.midi_host_io_enabled {
        let expected_line = if midi_host_io_enabled {
            "Host MIDI Out: capture"
        } else {
            "Host MIDI I/O: off"
        };

        if !state.lcd.lines.iter().any(|line| line == expected_line) {
            details.push(format!(
                "{prefix}midi_host_io_enabled mismatch: expected LCD line {expected_line:?}, got {:?}",
                state.lcd.lines
            ));
        }
    }

    let selected_sample = state.selected_sample();
    if let Some(selected_sample_index) = expected.selected_sample_index {
        let actual = selected_sample.as_ref().map(|entry| entry.index);
        if actual != Some(selected_sample_index) {
            details.push(format!(
                "{prefix}selected_sample_index mismatch: expected {}, got {:?}",
                selected_sample_index, actual
            ));
        }
    }

    if let Some(sample_catalog_count) = expected.sample_catalog_count {
        let actual = selected_sample
            .as_ref()
            .map(|entry| entry.count)
            .unwrap_or_else(|| state.sample_catalog().len());
        if actual != sample_catalog_count {
            details.push(format!(
                "{prefix}sample_catalog_count mismatch: expected {}, got {}",
                sample_catalog_count, actual
            ));
        }
    }

    if let Some(selected_sample_id) = &expected.selected_sample_id {
        let actual = selected_sample
            .as_ref()
            .map(|entry| entry.sample.id.as_str());
        if actual != Some(selected_sample_id.as_str()) {
            details.push(format!(
                "{prefix}selected_sample_id mismatch: expected {}, got {:?}",
                selected_sample_id, actual
            ));
        }
    }

    if let Some(selected_sample_name) = &expected.selected_sample_name {
        let actual = selected_sample
            .as_ref()
            .map(|entry| entry.sample.name.as_str());
        if actual != Some(selected_sample_name.as_str()) {
            details.push(format!(
                "{prefix}selected_sample_name mismatch: expected {}, got {:?}",
                selected_sample_name, actual
            ));
        }
    }

    if let Some(selected_sample_source_kind) = expected.selected_sample_source_kind {
        let actual = selected_sample.as_ref().map(|entry| entry.source_kind);
        if actual != Some(selected_sample_source_kind) {
            details.push(format!(
                "{prefix}selected_sample_source_kind mismatch: expected {:?}, got {:?}",
                selected_sample_source_kind, actual
            ));
        }
    }

    if let Some(selected_sample_length_frames) = expected.selected_sample_length_frames {
        let actual = selected_sample.as_ref().map(|entry| entry.length_frames);
        if actual != Some(selected_sample_length_frames) {
            details.push(format!(
                "{prefix}selected_sample_length_frames mismatch: expected {}, got {:?}",
                selected_sample_length_frames, actual
            ));
        }
    }

    if let Some(selected_trim_edit_field) = expected.selected_trim_edit_field {
        if state.selected_trim_edit_field != selected_trim_edit_field {
            details.push(format!(
                "{prefix}selected_trim_edit_field mismatch: expected {:?}, got {:?}",
                selected_trim_edit_field, state.selected_trim_edit_field
            ));
        }
    }

    if let Some(selected_sample_start_frame) = expected.selected_sample_start_frame {
        let actual = selected_sample.as_ref().map(|entry| entry.start_frame);
        if actual != Some(selected_sample_start_frame) {
            details.push(format!(
                "{prefix}selected_sample_start_frame mismatch: expected {}, got {:?}",
                selected_sample_start_frame, actual
            ));
        }
    }

    if let Some(selected_sample_end_frame) = expected.selected_sample_end_frame {
        let actual = selected_sample.as_ref().map(|entry| entry.end_frame);
        if actual != Some(selected_sample_end_frame) {
            details.push(format!(
                "{prefix}selected_sample_end_frame mismatch: expected {}, got {:?}",
                selected_sample_end_frame, actual
            ));
        }
    }

    if let Some(selected_sample_window_length_frames) =
        expected.selected_sample_window_length_frames
    {
        let actual = selected_sample
            .as_ref()
            .map(|entry| entry.window_length_frames);
        if actual != Some(selected_sample_window_length_frames) {
            details.push(format!(
                "{prefix}selected_sample_window_length_frames mismatch: expected {}, got {:?}",
                selected_sample_window_length_frames, actual
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
            runtime_samples,
        );
    }
}

fn validate_expected_audio_render(
    details: &mut Vec<String>,
    last_playback: Option<&SamplePlaybackResolution>,
    expected: &ExpectedAudioRender,
    runtime_samples: &RuntimeSampleLibrary,
) {
    let Some(SamplePlaybackResolution::Intent { intent }) = last_playback else {
        details.push(format!(
            "last_audio_render mismatch: expected renderable SamplePlaybackIntent, got {last_playback:?}"
        ));
        return;
    };

    let rendered =
        match render_intent_with_runtime_samples(intent, expected.settings, runtime_samples) {
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
        "last_audio_render.mute_group",
        &expected.mute_group,
        &summary.mute_group,
    );
    if let Some(start_frame) = expected.start_frame {
        push_mismatch(
            details,
            "last_audio_render.start_frame",
            &start_frame,
            &summary.start_frame,
        );
    }
    if let Some(end_frame) = expected.end_frame {
        push_mismatch(
            details,
            "last_audio_render.end_frame",
            &end_frame,
            &summary.end_frame,
        );
    }
    if let Some(window_length_frames) = expected.window_length_frames {
        push_mismatch(
            details,
            "last_audio_render.window_length_frames",
            &window_length_frames,
            &summary.window_length_frames,
        );
    }
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
                mute_group: 0,
                start_frame: 0,
                end_frame: 47_999,
                window_length_frames: 48_000,
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
            mute_group: 0,
            start_frame: Some(0),
            end_frame: Some(47_999),
            window_length_frames: Some(48_000),
            peak_left: 0,
            peak_right: 0,
            peak_amplitude: 0,
            channel_balance: ChannelBalance::Center,
            source_kind: AudioSourceKind::RightsSafeGenerated,
            loaded_audio_byte_count: 0,
        };
        let mut details = Vec::new();

        validate_expected_audio_render(
            &mut details,
            Some(&playback),
            &expected,
            &RuntimeSampleLibrary::default(),
        );

        assert_eq!(
            details,
            vec![format!(
                "last_audio_render render error: frame count {frame_count} exceeds maximum {}",
                mpc_audio::MAX_RENDER_FRAMES
            )]
        );
    }

    #[test]
    fn expected_state_rejects_null_midi_input_channel_expectation() {
        let error = serde_json::from_str::<ExpectedState>(
            r#"{
                "mode": "main",
                "lcd_title": "MAIN",
                "playing": false,
                "recording": false,
                "event_count": 0,
                "midi_input_channel": null
            }"#,
        )
        .expect_err("explicit null must not silently skip MIDI input channel validation");

        assert!(
            error
                .to_string()
                .contains("midi_input_channel must be omitted to skip validation"),
            "{error}"
        );
    }

    #[test]
    fn expected_state_parses_omni_midi_input_channel_as_explicit_expectation() {
        let expected = serde_json::from_str::<ExpectedState>(
            r#"{
                "mode": "main",
                "lcd_title": "MAIN",
                "playing": false,
                "recording": false,
                "event_count": 0,
                "midi_input_channel": "omni"
            }"#,
        )
        .expect("explicit omni should parse as a MIDI input channel expectation");

        assert_eq!(
            expected.midi_input_channel,
            Some(ExpectedMidiInputChannel::Omni)
        );
    }
}
