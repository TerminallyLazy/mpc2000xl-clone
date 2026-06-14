use mpc_core::{CountInClickIntent, PadBank, SamplePlaybackIntent, SampleReleaseIntent};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::VecDeque;

pub const DEFAULT_SAMPLE_RATE_HZ: u32 = 44_100;
pub const DEFAULT_FRAME_COUNT: usize = 512;
/// Foundation guardrail: accept common low-rate audio metadata, but reject
/// malformed zero or near-zero rates before any render allocation happens.
pub const MIN_SAMPLE_RATE_HZ: u32 = 8_000;
/// Foundation guardrail: keep synthetic renders within common high-resolution
/// audio metadata bounds until longer render windows are chunked explicitly.
pub const MAX_SAMPLE_RATE_HZ: u32 = 192_000;
/// Foundation guardrail: cap one synthetic render buffer to one second at the
/// maximum accepted sample rate so malformed fixtures cannot request OOM-sized
/// allocations.
pub const MAX_RENDER_FRAMES: usize = MAX_SAMPLE_RATE_HZ as usize;
/// Deterministic capture backends store render summaries only, but construction
/// still clamps requested history to avoid unchecked upfront allocation.
pub const MAX_CAPTURE_AUDIO_BACKEND_CAPTURES: usize = 1_024;
/// Internal foundation policy for deterministic host-audio voice accounting.
///
/// This is not accepted MPC2000XL hardware evidence and must not be presented as
/// exact reference voice behavior.
pub const DEFAULT_HOST_AUDIO_VOICE_LIMIT: usize = 32;
/// Lower guardrail for configurable deterministic host-audio voice limits.
pub const MIN_HOST_AUDIO_VOICE_LIMIT: usize = 1;
/// Upper guardrail for configurable deterministic host-audio voice limits.
pub const MAX_HOST_AUDIO_VOICE_LIMIT: usize = 128;
const PCM_MAX: i32 = i16::MAX as i32;
const MAX_VELOCITY: i32 = 127;
const MAX_LEVEL: i32 = 127;
const PAN_RANGE: i8 = 100;
const FNV_OFFSET_BASIS: u32 = 2_166_136_261;
const FNV_PRIME: u32 = 16_777_619;
const COUNT_IN_CLICK_ACCENT_PEAK: i32 = 24_000;
const COUNT_IN_CLICK_NORMAL_PEAK: i32 = 14_000;
const COUNT_IN_CLICK_ACTIVE_RATE_DIVISOR: usize = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioRenderSettings {
    pub sample_rate_hz: u32,
    pub frame_count: usize,
}

impl AudioRenderSettings {
    pub fn new(sample_rate_hz: u32, frame_count: usize) -> Result<Self, AudioRenderError> {
        let settings = Self {
            sample_rate_hz,
            frame_count,
        };
        settings.validate()?;
        Ok(settings)
    }

    pub fn preview() -> Self {
        Self {
            sample_rate_hz: DEFAULT_SAMPLE_RATE_HZ,
            frame_count: 256,
        }
    }

    pub fn validate(&self) -> Result<(), AudioRenderError> {
        if self.sample_rate_hz < MIN_SAMPLE_RATE_HZ {
            return Err(AudioRenderError::SampleRateBelowMinimum {
                sample_rate_hz: self.sample_rate_hz,
                min_sample_rate_hz: MIN_SAMPLE_RATE_HZ,
            });
        }

        if self.sample_rate_hz > MAX_SAMPLE_RATE_HZ {
            return Err(AudioRenderError::SampleRateAboveMaximum {
                sample_rate_hz: self.sample_rate_hz,
                max_sample_rate_hz: MAX_SAMPLE_RATE_HZ,
            });
        }

        if self.frame_count > MAX_RENDER_FRAMES {
            return Err(AudioRenderError::FrameCountTooLarge {
                frame_count: self.frame_count,
                max_frame_count: MAX_RENDER_FRAMES,
            });
        }

        Ok(())
    }
}

impl Default for AudioRenderSettings {
    fn default() -> Self {
        Self {
            sample_rate_hz: DEFAULT_SAMPLE_RATE_HZ,
            frame_count: DEFAULT_FRAME_COUNT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioRenderError {
    SampleRateBelowMinimum {
        sample_rate_hz: u32,
        min_sample_rate_hz: u32,
    },
    SampleRateAboveMaximum {
        sample_rate_hz: u32,
        max_sample_rate_hz: u32,
    },
    FrameCountTooLarge {
        frame_count: usize,
        max_frame_count: usize,
    },
}

impl std::fmt::Display for AudioRenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SampleRateBelowMinimum {
                sample_rate_hz,
                min_sample_rate_hz,
            } => write!(
                formatter,
                "sample rate {sample_rate_hz} Hz is below minimum {min_sample_rate_hz} Hz"
            ),
            Self::SampleRateAboveMaximum {
                sample_rate_hz,
                max_sample_rate_hz,
            } => write!(
                formatter,
                "sample rate {sample_rate_hz} Hz exceeds maximum {max_sample_rate_hz} Hz"
            ),
            Self::FrameCountTooLarge {
                frame_count,
                max_frame_count,
            } => write!(
                formatter,
                "frame count {frame_count} exceeds maximum {max_frame_count}"
            ),
        }
    }
}

impl std::error::Error for AudioRenderError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFrame {
    pub left: i16,
    pub right: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSourceKind {
    RightsSafeGenerated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelBalance {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioRenderKind {
    #[default]
    SamplePlayback,
    CountInClick,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioRenderSummary {
    pub render_kind: AudioRenderKind,
    pub sample_rate_hz: u32,
    pub frame_count: usize,
    pub source_sample_id: String,
    pub source_sample_name: String,
    pub selected_track: u8,
    pub program_index: u8,
    pub program_name: String,
    pub bank: PadBank,
    pub pad_number: u8,
    pub velocity: u8,
    pub level: u8,
    pub pan: i8,
    pub tune_cents: i16,
    pub start_frame: u32,
    pub end_frame: u32,
    pub window_length_frames: u32,
    pub peak_left: i16,
    pub peak_right: i16,
    pub peak_amplitude: i16,
    pub channel_balance: ChannelBalance,
    pub source_kind: AudioSourceKind,
    pub loaded_audio_byte_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count_in_tick: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar_index: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beat_index: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<bool>,
}

impl<'de> Deserialize<'de> for AudioRenderSummary {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct AudioRenderSummaryData {
            #[serde(default)]
            render_kind: AudioRenderKind,
            sample_rate_hz: u32,
            frame_count: usize,
            source_sample_id: String,
            source_sample_name: String,
            selected_track: u8,
            program_index: u8,
            program_name: String,
            bank: PadBank,
            pad_number: u8,
            velocity: u8,
            level: u8,
            pan: i8,
            tune_cents: i16,
            #[serde(default)]
            start_frame: Option<u32>,
            #[serde(default)]
            end_frame: Option<u32>,
            #[serde(default)]
            window_length_frames: Option<u32>,
            peak_left: i16,
            peak_right: i16,
            peak_amplitude: i16,
            channel_balance: ChannelBalance,
            source_kind: AudioSourceKind,
            loaded_audio_byte_count: usize,
            #[serde(default)]
            count_in_tick: Option<u64>,
            #[serde(default)]
            bar_index: Option<u8>,
            #[serde(default)]
            beat_index: Option<u8>,
            #[serde(default)]
            accent: Option<bool>,
        }

        let data = AudioRenderSummaryData::deserialize(deserializer)?;
        let default_window_length_frames = u32::try_from(data.frame_count).unwrap_or(u32::MAX);
        let default_end_frame = data
            .frame_count
            .checked_sub(1)
            .and_then(|frame| u32::try_from(frame).ok())
            .unwrap_or(0);

        Ok(Self {
            render_kind: data.render_kind,
            sample_rate_hz: data.sample_rate_hz,
            frame_count: data.frame_count,
            source_sample_id: data.source_sample_id,
            source_sample_name: data.source_sample_name,
            selected_track: data.selected_track,
            program_index: data.program_index,
            program_name: data.program_name,
            bank: data.bank,
            pad_number: data.pad_number,
            velocity: data.velocity,
            level: data.level,
            pan: data.pan,
            tune_cents: data.tune_cents,
            start_frame: data.start_frame.unwrap_or(0),
            end_frame: data.end_frame.unwrap_or(default_end_frame),
            window_length_frames: data
                .window_length_frames
                .unwrap_or(default_window_length_frames),
            peak_left: data.peak_left,
            peak_right: data.peak_right,
            peak_amplitude: data.peak_amplitude,
            channel_balance: data.channel_balance,
            source_kind: data.source_kind,
            loaded_audio_byte_count: data.loaded_audio_byte_count,
            count_in_tick: data.count_in_tick,
            bar_index: data.bar_index,
            beat_index: data.beat_index,
            accent: data.accent,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedAudio {
    pub settings: AudioRenderSettings,
    pub summary: AudioRenderSummary,
    pub frames: Vec<AudioFrame>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAudioMode {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAudioIgnoreReason {
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAudioRenderReceipt {
    pub summary: AudioRenderSummary,
    pub frame_count: usize,
    pub queued: bool,
    pub played: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_allocation: Option<HostAudioVoiceAllocationReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAudioVoiceAllocationReceipt {
    pub voice_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stolen_voice_id: Option<u64>,
    pub voice_limit: usize,
    pub active_voice_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAudioVoiceSummary {
    pub voice_id: u64,
    pub render_kind: AudioRenderKind,
    pub source_label: String,
    #[serde(default)]
    pub source_sample_id: String,
    #[serde(default = "default_voice_bank")]
    pub bank: PadBank,
    #[serde(default)]
    pub pad_number: u8,
    pub total_frame_count: usize,
    pub remaining_frame_count: usize,
}

fn default_voice_bank() -> PadBank {
    PadBank::A
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAudioReleaseReceipt {
    pub intent: SampleReleaseIntent,
    pub released_voice_ids: Vec<u64>,
    pub released_voice_count: usize,
    pub active_voice_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAudioPlaybackReport {
    pub event: HostAudioEvent,
    pub render_summary: Option<AudioRenderSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostAudioEvent {
    Ignored {
        backend_name: String,
        reason: HostAudioIgnoreReason,
    },
    Enqueued {
        backend_name: String,
        receipt: HostAudioRenderReceipt,
    },
    Released {
        backend_name: String,
        receipt: HostAudioReleaseReceipt,
    },
    Failed {
        backend_name: String,
        error: HostAudioError,
        summary: Option<AudioRenderSummary>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAudioState {
    pub mode: HostAudioMode,
    pub backend_name: String,
    pub render_settings: AudioRenderSettings,
    pub queued_render_count: u64,
    pub played_render_count: u64,
    #[serde(default = "default_host_audio_voice_limit")]
    pub voice_limit: usize,
    #[serde(default)]
    pub active_voice_count: usize,
    #[serde(default)]
    pub completed_voice_count: u64,
    #[serde(default)]
    pub stolen_voice_count: u64,
    #[serde(default)]
    pub released_voice_count: u64,
    #[serde(default)]
    pub active_voices: Vec<HostAudioVoiceSummary>,
    pub last_event: Option<HostAudioEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostAudioError {
    Render { error: AudioRenderError },
    InvalidRenderedAudio { error: InvalidRenderedAudioError },
    InvalidBackendReceipt { error: InvalidBackendReceiptError },
    Backend { error: HostAudioBackendError },
}

impl HostAudioError {
    fn render(error: AudioRenderError) -> Self {
        Self::Render { error }
    }

    fn invalid_rendered_audio(error: InvalidRenderedAudioError) -> Self {
        Self::InvalidRenderedAudio { error }
    }

    fn invalid_backend_receipt(error: InvalidBackendReceiptError) -> Self {
        Self::InvalidBackendReceipt { error }
    }

    fn backend(error: HostAudioBackendError) -> Self {
        Self::Backend { error }
    }
}

impl std::fmt::Display for HostAudioError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Render { error } => write!(formatter, "render failed: {error}"),
            Self::InvalidRenderedAudio { error } => {
                write!(formatter, "invalid rendered audio: {error}")
            }
            Self::InvalidBackendReceipt { error } => {
                write!(formatter, "invalid backend receipt: {error}")
            }
            Self::Backend { error } => write!(formatter, "backend failed: {error}"),
        }
    }
}

impl std::error::Error for HostAudioError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InvalidRenderedAudioError {
    FrameCountMismatch {
        settings_frame_count: usize,
        summary_frame_count: usize,
        actual_frame_count: usize,
    },
}

impl std::fmt::Display for InvalidRenderedAudioError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FrameCountMismatch {
                settings_frame_count,
                summary_frame_count,
                actual_frame_count,
            } => write!(
                formatter,
                "settings frame count {settings_frame_count}, summary frame count {summary_frame_count}, and actual frame count {actual_frame_count} must match"
            ),
        }
    }
}

impl std::error::Error for InvalidRenderedAudioError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InvalidBackendReceiptError {
    PlayedWithoutQueued,
    FrameCountMismatch {
        receipt_frame_count: usize,
        rendered_frame_count: usize,
    },
}

impl std::fmt::Display for InvalidBackendReceiptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlayedWithoutQueued => {
                write!(formatter, "played receipts must also be queued")
            }
            Self::FrameCountMismatch {
                receipt_frame_count,
                rendered_frame_count,
            } => write!(
                formatter,
                "receipt frame count {receipt_frame_count} must match rendered frame count {rendered_frame_count}"
            ),
        }
    }
}

impl std::error::Error for InvalidBackendReceiptError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostAudioBackendError {
    BackendUnavailable {
        backend_name: String,
        message: String,
    },
}

impl HostAudioBackendError {
    pub fn backend_unavailable(
        backend_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::BackendUnavailable {
            backend_name: backend_name.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for HostAudioBackendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendUnavailable {
                backend_name,
                message,
            } => write!(formatter, "{backend_name} unavailable: {message}"),
        }
    }
}

impl std::error::Error for HostAudioBackendError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAudioBackendReceipt {
    frame_count: usize,
    queued: bool,
    played: bool,
}

impl HostAudioBackendReceipt {
    pub fn not_queued(frame_count: usize) -> Self {
        Self {
            frame_count,
            queued: false,
            played: false,
        }
    }

    pub fn queued(frame_count: usize) -> Self {
        Self {
            frame_count,
            queued: true,
            played: false,
        }
    }

    pub fn queued_and_played(frame_count: usize) -> Self {
        Self {
            frame_count,
            queued: true,
            played: true,
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    pub fn is_queued(&self) -> bool {
        self.queued
    }

    pub fn is_played(&self) -> bool {
        self.played
    }
}

pub trait HostAudioBackend {
    fn backend_name(&self) -> &str;

    fn enqueue_render(
        &mut self,
        rendered: &RenderedAudio,
    ) -> Result<HostAudioBackendReceipt, HostAudioBackendError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullAudioBackend {
    backend_name: String,
}

impl NullAudioBackend {
    pub fn new() -> Self {
        Self::named("null")
    }

    pub fn named(backend_name: impl Into<String>) -> Self {
        Self {
            backend_name: backend_name.into(),
        }
    }
}

impl Default for NullAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl HostAudioBackend for NullAudioBackend {
    fn backend_name(&self) -> &str {
        &self.backend_name
    }

    fn enqueue_render(
        &mut self,
        rendered: &RenderedAudio,
    ) -> Result<HostAudioBackendReceipt, HostAudioBackendError> {
        Ok(HostAudioBackendReceipt::queued_and_played(
            rendered.frames.len(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedHostAudioRender {
    pub summary: AudioRenderSummary,
    pub frame_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureAudioBackend {
    backend_name: String,
    max_captures: usize,
    captures: VecDeque<CapturedHostAudioRender>,
}

impl CaptureAudioBackend {
    pub fn new(max_captures: usize) -> Self {
        Self::named("capture", max_captures)
    }

    pub fn named(backend_name: impl Into<String>, max_captures: usize) -> Self {
        let max_captures = max_captures.min(MAX_CAPTURE_AUDIO_BACKEND_CAPTURES);
        Self {
            backend_name: backend_name.into(),
            max_captures,
            captures: VecDeque::with_capacity(max_captures),
        }
    }

    pub fn max_captures(&self) -> usize {
        self.max_captures
    }

    pub fn captured_renders(&self) -> &VecDeque<CapturedHostAudioRender> {
        &self.captures
    }
}

impl HostAudioBackend for CaptureAudioBackend {
    fn backend_name(&self) -> &str {
        &self.backend_name
    }

    fn enqueue_render(
        &mut self,
        rendered: &RenderedAudio,
    ) -> Result<HostAudioBackendReceipt, HostAudioBackendError> {
        if self.max_captures > 0 {
            if self.captures.len() == self.max_captures {
                self.captures.pop_front();
            }
            self.captures.push_back(CapturedHostAudioRender {
                summary: rendered.summary.clone(),
                frame_count: rendered.frames.len(),
            });
        }

        Ok(HostAudioBackendReceipt::queued_and_played(
            rendered.frames.len(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAudioEngine<B>
where
    B: HostAudioBackend,
{
    backend: B,
    mode: HostAudioMode,
    render_settings: AudioRenderSettings,
    queued_render_count: u64,
    played_render_count: u64,
    voice_limit: usize,
    next_voice_id: u64,
    active_voices: VecDeque<HostAudioVoiceSummary>,
    completed_voice_count: u64,
    stolen_voice_count: u64,
    released_voice_count: u64,
    last_event: Option<HostAudioEvent>,
}

impl<B> HostAudioEngine<B>
where
    B: HostAudioBackend,
{
    pub fn new(backend: B, render_settings: AudioRenderSettings) -> Result<Self, HostAudioError> {
        Self::new_with_voice_limit(backend, render_settings, DEFAULT_HOST_AUDIO_VOICE_LIMIT)
    }

    /// Builds a deterministic host-audio engine with a clamped internal voice limit.
    ///
    /// The limit is an internal foundation policy for repeatable host-side
    /// accounting, not accepted MPC2000XL hardware evidence.
    pub fn new_with_voice_limit(
        backend: B,
        render_settings: AudioRenderSettings,
        voice_limit: usize,
    ) -> Result<Self, HostAudioError> {
        render_settings.validate().map_err(HostAudioError::render)?;
        Ok(Self {
            backend,
            mode: HostAudioMode::Disabled,
            render_settings,
            queued_render_count: 0,
            played_render_count: 0,
            voice_limit: clamp_host_audio_voice_limit(voice_limit),
            next_voice_id: 1,
            active_voices: VecDeque::new(),
            completed_voice_count: 0,
            stolen_voice_count: 0,
            released_voice_count: 0,
            last_event: None,
        })
    }

    pub fn enabled(
        backend: B,
        render_settings: AudioRenderSettings,
    ) -> Result<Self, HostAudioError> {
        let mut engine =
            Self::new_with_voice_limit(backend, render_settings, DEFAULT_HOST_AUDIO_VOICE_LIMIT)?;
        engine.set_mode(HostAudioMode::Enabled);
        Ok(engine)
    }

    /// Builds an enabled deterministic host-audio engine with a clamped internal
    /// voice limit. The limit is foundation policy, not hardware evidence.
    pub fn enabled_with_voice_limit(
        backend: B,
        render_settings: AudioRenderSettings,
        voice_limit: usize,
    ) -> Result<Self, HostAudioError> {
        let mut engine = Self::new_with_voice_limit(backend, render_settings, voice_limit)?;
        engine.set_mode(HostAudioMode::Enabled);
        Ok(engine)
    }

    pub fn is_enabled(&self) -> bool {
        self.mode == HostAudioMode::Enabled
    }

    pub fn mode(&self) -> HostAudioMode {
        self.mode
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.set_mode(if enabled {
            HostAudioMode::Enabled
        } else {
            HostAudioMode::Disabled
        });
    }

    pub fn set_mode(&mut self, mode: HostAudioMode) {
        self.mode = mode;
    }

    pub fn render_settings(&self) -> AudioRenderSettings {
        self.render_settings
    }

    pub fn voice_limit(&self) -> usize {
        self.voice_limit
    }

    pub fn active_voices(&self) -> &VecDeque<HostAudioVoiceSummary> {
        &self.active_voices
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn state(&self) -> HostAudioState {
        HostAudioState {
            mode: self.mode,
            backend_name: self.backend.backend_name().to_string(),
            render_settings: self.render_settings,
            queued_render_count: self.queued_render_count,
            played_render_count: self.played_render_count,
            voice_limit: self.voice_limit,
            active_voice_count: self.active_voices.len(),
            completed_voice_count: self.completed_voice_count,
            stolen_voice_count: self.stolen_voice_count,
            released_voice_count: self.released_voice_count,
            active_voices: self.active_voices.iter().cloned().collect(),
            last_event: self.last_event.clone(),
        }
    }

    pub fn play_intent(&mut self, intent: &SamplePlaybackIntent) -> HostAudioEvent {
        self.play_intent_with_render_summary(intent).event
    }

    pub fn play_intent_with_render_summary(
        &mut self,
        intent: &SamplePlaybackIntent,
    ) -> HostAudioPlaybackReport {
        match render_intent(intent, self.render_settings) {
            Ok(rendered) => {
                let render_summary = Some(rendered.summary.clone());
                let event = self.play_rendered(rendered);
                HostAudioPlaybackReport {
                    event,
                    render_summary,
                }
            }
            Err(error) => {
                let event = self.record_failed(HostAudioError::render(error), None);
                HostAudioPlaybackReport {
                    event,
                    render_summary: None,
                }
            }
        }
    }

    pub fn play_count_in_click(&mut self, intent: &CountInClickIntent) -> HostAudioEvent {
        self.play_count_in_click_with_render_summary(intent).event
    }

    pub fn play_count_in_click_with_render_summary(
        &mut self,
        intent: &CountInClickIntent,
    ) -> HostAudioPlaybackReport {
        match render_count_in_click(intent, self.render_settings) {
            Ok(rendered) => {
                let render_summary = Some(rendered.summary.clone());
                let event = self.play_rendered(rendered);
                HostAudioPlaybackReport {
                    event,
                    render_summary,
                }
            }
            Err(error) => {
                let event = self.record_failed(HostAudioError::render(error), None);
                HostAudioPlaybackReport {
                    event,
                    render_summary: None,
                }
            }
        }
    }

    pub fn play_rendered(&mut self, rendered: RenderedAudio) -> HostAudioEvent {
        if self.mode == HostAudioMode::Disabled {
            return self.record_ignored();
        }

        if let Err(error) = validate_rendered_audio(&rendered) {
            return self.record_failed(error, None);
        }

        let summary = rendered.summary.clone();
        match self.backend.enqueue_render(&rendered) {
            Ok(backend_receipt) => {
                if let Err(error) = validate_backend_receipt(&backend_receipt, &rendered) {
                    return self.record_failed(error, Some(summary));
                }

                if backend_receipt.is_queued() {
                    self.queued_render_count = self.queued_render_count.saturating_add(1);
                }
                if backend_receipt.is_played() {
                    self.played_render_count = self.played_render_count.saturating_add(1);
                }

                let voice_allocation =
                    if backend_receipt.is_queued() && backend_receipt.frame_count() > 0 {
                        Some(self.allocate_voice(&summary, backend_receipt.frame_count()))
                    } else {
                        None
                    };

                self.record_enqueued(HostAudioRenderReceipt {
                    summary,
                    frame_count: backend_receipt.frame_count(),
                    queued: backend_receipt.is_queued(),
                    played: backend_receipt.is_played(),
                    voice_allocation,
                })
            }
            Err(error) => self.record_failed(HostAudioError::backend(error), Some(summary)),
        }
    }

    pub fn advance_voice_frames(&mut self, frame_count: usize) {
        if frame_count == 0 {
            return;
        }

        let mut completed = 0_u64;
        let mut active_voices = VecDeque::with_capacity(self.active_voices.len());

        while let Some(mut voice) = self.active_voices.pop_front() {
            voice.remaining_frame_count = voice.remaining_frame_count.saturating_sub(frame_count);
            if voice.remaining_frame_count == 0 {
                completed = completed.saturating_add(1);
            } else {
                active_voices.push_back(voice);
            }
        }

        self.active_voices = active_voices;
        self.completed_voice_count = self.completed_voice_count.saturating_add(completed);
    }

    pub fn release_intent(&mut self, intent: &SampleReleaseIntent) -> HostAudioEvent {
        if self.mode == HostAudioMode::Disabled {
            return self.record_ignored();
        }

        let released_voice_ids = self.release_matching_voices(intent);
        let released_voice_count = released_voice_ids.len();
        self.released_voice_count = self
            .released_voice_count
            .saturating_add(u64::try_from(released_voice_count).unwrap_or(u64::MAX));

        let event = HostAudioEvent::Released {
            backend_name: self.backend.backend_name().to_string(),
            receipt: HostAudioReleaseReceipt {
                intent: intent.clone(),
                released_voice_ids,
                released_voice_count,
                active_voice_count: self.active_voices.len(),
            },
        };
        self.last_event = Some(event.clone());
        event
    }

    fn release_matching_voices(&mut self, intent: &SampleReleaseIntent) -> Vec<u64> {
        let mut released_voice_ids = Vec::new();
        let mut active_voices = VecDeque::with_capacity(self.active_voices.len());

        while let Some(voice) = self.active_voices.pop_front() {
            if voice.render_kind == AudioRenderKind::SamplePlayback
                && voice.source_sample_id == intent.sample_id
                && voice.bank == intent.bank
                && voice.pad_number == intent.pad_number
            {
                released_voice_ids.push(voice.voice_id);
            } else {
                active_voices.push_back(voice);
            }
        }

        self.active_voices = active_voices;
        released_voice_ids
    }

    fn allocate_voice(
        &mut self,
        summary: &AudioRenderSummary,
        frame_count: usize,
    ) -> HostAudioVoiceAllocationReceipt {
        let stolen_voice_id = if self.active_voices.len() >= self.voice_limit {
            let stolen_voice_id = self.active_voices.pop_front().map(|voice| voice.voice_id);
            if stolen_voice_id.is_some() {
                self.stolen_voice_count = self.stolen_voice_count.saturating_add(1);
            }
            stolen_voice_id
        } else {
            None
        };

        let voice_id = self.next_voice_id;
        self.next_voice_id = self.next_voice_id.saturating_add(1);
        self.active_voices.push_back(HostAudioVoiceSummary {
            voice_id,
            render_kind: summary.render_kind,
            source_label: voice_source_label(summary),
            source_sample_id: summary.source_sample_id.clone(),
            bank: summary.bank,
            pad_number: summary.pad_number,
            total_frame_count: frame_count,
            remaining_frame_count: frame_count,
        });

        HostAudioVoiceAllocationReceipt {
            voice_id,
            stolen_voice_id,
            voice_limit: self.voice_limit,
            active_voice_count: self.active_voices.len(),
        }
    }

    fn record_ignored(&mut self) -> HostAudioEvent {
        let event = HostAudioEvent::Ignored {
            backend_name: self.backend.backend_name().to_string(),
            reason: HostAudioIgnoreReason::Disabled,
        };
        self.last_event = Some(event.clone());
        event
    }

    fn record_enqueued(&mut self, receipt: HostAudioRenderReceipt) -> HostAudioEvent {
        let event = HostAudioEvent::Enqueued {
            backend_name: self.backend.backend_name().to_string(),
            receipt,
        };
        self.last_event = Some(event.clone());
        event
    }

    fn record_failed(
        &mut self,
        error: HostAudioError,
        summary: Option<AudioRenderSummary>,
    ) -> HostAudioEvent {
        let event = HostAudioEvent::Failed {
            backend_name: self.backend.backend_name().to_string(),
            error,
            summary,
        };
        self.last_event = Some(event.clone());
        event
    }
}

pub fn render_intent(
    intent: &SamplePlaybackIntent,
    settings: AudioRenderSettings,
) -> Result<RenderedAudio, AudioRenderError> {
    settings.validate()?;

    let render_frame_count = settings
        .frame_count
        .min(usize::try_from(intent.window_length_frames).unwrap_or(usize::MAX));
    let render_settings = AudioRenderSettings {
        sample_rate_hz: settings.sample_rate_hz,
        frame_count: render_frame_count,
    };
    let seed = stable_seed(intent);
    let mono_peak = scaled_mono_peak(intent.velocity, intent.level);
    let pan = intent.pan.clamp(-PAN_RANGE, PAN_RANGE);
    let (left_gain, right_gain) = stereo_gains(pan);
    let mut frames = Vec::with_capacity(render_frame_count);
    let mut peak_left = 0_i16;
    let mut peak_right = 0_i16;

    for frame_index in 0..render_frame_count {
        let source_frame_index = frame_index.saturating_add(intent.start_frame as usize);
        let wave = seeded_square_wave(seed, source_frame_index, intent.tune_cents);
        let mono = wave * mono_peak / 255;
        let left = clamp_i16(mono * left_gain / 100);
        let right = clamp_i16(mono * right_gain / 100);

        peak_left = peak_left.max(left.saturating_abs());
        peak_right = peak_right.max(right.saturating_abs());
        frames.push(AudioFrame { left, right });
    }

    let peak_amplitude = peak_left.max(peak_right);
    let channel_balance = channel_balance(peak_left, peak_right);
    let summary = AudioRenderSummary {
        render_kind: AudioRenderKind::SamplePlayback,
        sample_rate_hz: render_settings.sample_rate_hz,
        frame_count: render_settings.frame_count,
        source_sample_id: intent.sample_id.clone(),
        source_sample_name: intent.sample_name.clone(),
        selected_track: intent.selected_track,
        program_index: intent.program_index,
        program_name: intent.program_name.clone(),
        bank: intent.bank,
        pad_number: intent.pad_number,
        velocity: intent.velocity,
        level: intent.level,
        pan: intent.pan,
        tune_cents: intent.tune_cents,
        start_frame: intent.start_frame,
        end_frame: intent.end_frame,
        window_length_frames: intent.window_length_frames,
        peak_left,
        peak_right,
        peak_amplitude,
        channel_balance,
        source_kind: AudioSourceKind::RightsSafeGenerated,
        loaded_audio_byte_count: 0,
        count_in_tick: None,
        bar_index: None,
        beat_index: None,
        accent: None,
    };

    Ok(RenderedAudio {
        settings: render_settings,
        summary,
        frames,
    })
}

pub fn render_count_in_click(
    intent: &CountInClickIntent,
    settings: AudioRenderSettings,
) -> Result<RenderedAudio, AudioRenderError> {
    settings.validate()?;

    let render_settings = settings;
    let active_frame_count = count_in_click_active_frame_count(render_settings);
    let click_peak = count_in_click_peak(intent.accent);
    let mut frames = Vec::with_capacity(render_settings.frame_count);
    let mut peak_left = 0_i16;
    let mut peak_right = 0_i16;

    for frame_index in 0..render_settings.frame_count {
        let mono = count_in_click_sample(intent, frame_index, active_frame_count, click_peak);
        let left = mono;
        let right = mono;

        peak_left = peak_left.max(left.saturating_abs());
        peak_right = peak_right.max(right.saturating_abs());
        frames.push(AudioFrame { left, right });
    }

    let frame_count_u32 = u32::try_from(render_settings.frame_count).unwrap_or(u32::MAX);
    let peak_amplitude = peak_left.max(peak_right);
    let source_sample_name = if intent.accent {
        "COUNT-IN ACCENT"
    } else {
        "COUNT-IN CLICK"
    };
    let summary = AudioRenderSummary {
        render_kind: AudioRenderKind::CountInClick,
        sample_rate_hz: render_settings.sample_rate_hz,
        frame_count: render_settings.frame_count,
        source_sample_id: "count_in_click".to_string(),
        source_sample_name: source_sample_name.to_string(),
        selected_track: 0,
        program_index: 0,
        program_name: "Metronome".to_string(),
        bank: PadBank::A,
        pad_number: 0,
        velocity: if intent.accent { 127 } else { 96 },
        level: 127,
        pan: 0,
        tune_cents: 0,
        start_frame: 0,
        end_frame: frame_count_u32.saturating_sub(1),
        window_length_frames: frame_count_u32,
        peak_left,
        peak_right,
        peak_amplitude,
        channel_balance: channel_balance(peak_left, peak_right),
        source_kind: AudioSourceKind::RightsSafeGenerated,
        loaded_audio_byte_count: 0,
        count_in_tick: Some(intent.count_in_tick),
        bar_index: Some(intent.bar_index),
        beat_index: Some(intent.beat_index),
        accent: Some(intent.accent),
    };

    Ok(RenderedAudio {
        settings: render_settings,
        summary,
        frames,
    })
}

fn default_host_audio_voice_limit() -> usize {
    DEFAULT_HOST_AUDIO_VOICE_LIMIT
}

fn clamp_host_audio_voice_limit(voice_limit: usize) -> usize {
    voice_limit.clamp(MIN_HOST_AUDIO_VOICE_LIMIT, MAX_HOST_AUDIO_VOICE_LIMIT)
}

fn voice_source_label(summary: &AudioRenderSummary) -> String {
    match summary.render_kind {
        AudioRenderKind::SamplePlayback | AudioRenderKind::CountInClick => {
            summary.source_sample_name.clone()
        }
    }
}

fn validate_rendered_audio(rendered: &RenderedAudio) -> Result<(), HostAudioError> {
    rendered
        .settings
        .validate()
        .map_err(HostAudioError::render)?;

    if rendered.settings.frame_count != rendered.summary.frame_count
        || rendered.settings.frame_count != rendered.frames.len()
    {
        return Err(HostAudioError::invalid_rendered_audio(
            InvalidRenderedAudioError::FrameCountMismatch {
                settings_frame_count: rendered.settings.frame_count,
                summary_frame_count: rendered.summary.frame_count,
                actual_frame_count: rendered.frames.len(),
            },
        ));
    }

    Ok(())
}

fn validate_backend_receipt(
    receipt: &HostAudioBackendReceipt,
    rendered: &RenderedAudio,
) -> Result<(), HostAudioError> {
    if receipt.played && !receipt.queued {
        return Err(HostAudioError::invalid_backend_receipt(
            InvalidBackendReceiptError::PlayedWithoutQueued,
        ));
    }

    if receipt.frame_count != rendered.frames.len() {
        return Err(HostAudioError::invalid_backend_receipt(
            InvalidBackendReceiptError::FrameCountMismatch {
                receipt_frame_count: receipt.frame_count,
                rendered_frame_count: rendered.frames.len(),
            },
        ));
    }

    Ok(())
}

fn stable_seed(intent: &SamplePlaybackIntent) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in intent
        .sample_id
        .bytes()
        .chain([0xff])
        .chain(intent.sample_name.bytes())
    {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn scaled_mono_peak(velocity: u8, level: u8) -> i32 {
    let velocity = i32::from(velocity.min(MAX_VELOCITY as u8));
    let level = i32::from(level.min(MAX_LEVEL as u8));
    PCM_MAX * velocity * level / (MAX_VELOCITY * MAX_LEVEL)
}

fn stereo_gains(pan: i8) -> (i32, i32) {
    let pan = i32::from(pan);
    let left = 100 - pan.max(0);
    let right = 100 + pan.min(0);
    (left, right)
}

fn seeded_square_wave(seed: u32, frame_index: usize, tune_cents: i16) -> i32 {
    let base_period = 12_i32 + i32::try_from(seed % 53).expect("period seed fits i32");
    let semitone_offset = i32::from(tune_cents) / 100;
    let period = usize::try_from((base_period - semitone_offset).clamp(4, 128))
        .expect("tuned period fits usize");
    let duty = 1 + usize::try_from((seed >> 8) % (period as u32 - 1)).expect("duty fits usize");
    let phase_seed = i64::from(seed >> 16) + i64::from(tune_cents);
    let phase_offset =
        usize::try_from(phase_seed.rem_euclid(period as i64)).expect("phase offset fits usize");
    let phase = (frame_index + phase_offset) % period;

    if phase < duty { 255 } else { -255 }
}

fn count_in_click_active_frame_count(settings: AudioRenderSettings) -> usize {
    if settings.frame_count == 0 {
        return 0;
    }

    let active_frame_limit =
        (settings.sample_rate_hz as usize / COUNT_IN_CLICK_ACTIVE_RATE_DIVISOR).max(1);
    settings.frame_count.min(active_frame_limit)
}

fn count_in_click_peak(accent: bool) -> i32 {
    if accent {
        COUNT_IN_CLICK_ACCENT_PEAK
    } else {
        COUNT_IN_CLICK_NORMAL_PEAK
    }
}

fn count_in_click_sample(
    intent: &CountInClickIntent,
    frame_index: usize,
    active_frame_count: usize,
    peak: i32,
) -> i16 {
    if frame_index >= active_frame_count || active_frame_count == 0 {
        return 0;
    }

    let phase_seed =
        ((intent.count_in_tick ^ u64::from(intent.bar_index) ^ u64::from(intent.beat_index)) & 1)
            as usize;
    let polarity_period = if intent.accent { 3 } else { 5 };
    let polarity = if ((frame_index / polarity_period) + phase_seed).is_multiple_of(2) {
        1
    } else {
        -1
    };
    let envelope = peak * i32::try_from(active_frame_count - frame_index).unwrap_or(i32::MAX)
        / i32::try_from(active_frame_count).unwrap_or(1);

    clamp_i16(polarity * envelope)
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn channel_balance(peak_left: i16, peak_right: i16) -> ChannelBalance {
    match peak_left.cmp(&peak_right) {
        std::cmp::Ordering::Greater => ChannelBalance::Left,
        std::cmp::Ordering::Equal => ChannelBalance::Center,
        std::cmp::Ordering::Less => ChannelBalance::Right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpc_core::{CountInClickIntent, PadBank};

    #[test]
    fn same_intent_and_settings_render_exact_same_frames() {
        let intent = test_intent(100, 100, 0);
        let settings = settings(48_000, 96);

        let first = render(&intent, settings);
        let second = render(&intent, settings);

        assert_eq!(first.frames, second.frames);
        assert_eq!(first.summary, second.summary);
    }

    #[test]
    fn velocity_and_level_affect_peak_amplitude() {
        let quiet_velocity = render(&test_intent(40, 100, 0), settings(44_100, 128));
        let loud_velocity = render(&test_intent(100, 100, 0), settings(44_100, 128));
        let quiet_level = render(&test_intent(100, 40, 0), settings(44_100, 128));

        assert!(loud_velocity.summary.peak_amplitude > quiet_velocity.summary.peak_amplitude);
        assert!(loud_velocity.summary.peak_amplitude > quiet_level.summary.peak_amplitude);
    }

    #[test]
    fn pan_affects_left_and_right_channels() {
        let left = render(&test_intent(100, 100, -60), settings(44_100, 128));
        let center = render(&test_intent(100, 100, 0), settings(44_100, 128));
        let right = render(&test_intent(100, 100, 60), settings(44_100, 128));

        assert!(left.summary.peak_left > left.summary.peak_right);
        assert_eq!(left.summary.channel_balance, ChannelBalance::Left);
        assert_eq!(center.summary.peak_left, center.summary.peak_right);
        assert_eq!(center.summary.channel_balance, ChannelBalance::Center);
        assert!(right.summary.peak_right > right.summary.peak_left);
        assert_eq!(right.summary.channel_balance, ChannelBalance::Right);
    }

    #[test]
    fn tune_is_reported_and_changes_deterministic_frames() {
        let settings = settings(44_100, 128);
        let default_tune = render(&test_intent_with_tune(100, 100, 0, 0), settings);
        let raised_tune = render(&test_intent_with_tune(100, 100, 0, 700), settings);

        assert_eq!(default_tune.summary.tune_cents, 0);
        assert_eq!(raised_tune.summary.tune_cents, 700);
        assert_eq!(
            default_tune.summary.peak_amplitude,
            raised_tune.summary.peak_amplitude
        );
        assert_ne!(default_tune.frames, raised_tune.frames);
    }

    #[test]
    fn render_length_and_sample_rate_are_respected() {
        let rendered = render(&test_intent(100, 100, 0), settings(32_000, 17));

        assert_eq!(rendered.frames.len(), 17);
        assert_eq!(rendered.settings.sample_rate_hz, 32_000);
        assert_eq!(rendered.summary.sample_rate_hz, 32_000);
        assert_eq!(rendered.summary.frame_count, 17);
    }

    #[test]
    fn render_is_clipped_to_trim_window() {
        let mut intent = test_intent(100, 100, 0);
        intent.start_frame = 10;
        intent.end_frame = 12;
        intent.window_length_frames = 3;

        let rendered = render(&intent, settings(44_100, 16));

        assert_eq!(rendered.frames.len(), 3);
        assert_eq!(rendered.settings.frame_count, 3);
        assert_eq!(rendered.summary.frame_count, 3);
        assert_eq!(rendered.summary.start_frame, 10);
        assert_eq!(rendered.summary.end_frame, 12);
        assert_eq!(rendered.summary.window_length_frames, 3);
    }

    #[test]
    fn audio_render_summary_deserializes_legacy_windowless_json() {
        let json = r#"{
            "sample_rate_hz": 44100,
            "frame_count": 24,
            "source_sample_id": "synthetic_a_04",
            "source_sample_name": "SYN-A04",
            "selected_track": 1,
            "program_index": 1,
            "program_name": "Program01",
            "bank": "a",
            "pad_number": 4,
            "velocity": 100,
            "level": 100,
            "pan": 0,
            "tune_cents": 0,
            "peak_left": 1000,
            "peak_right": 1000,
            "peak_amplitude": 1000,
            "channel_balance": "center",
            "source_kind": "rights_safe_generated",
            "loaded_audio_byte_count": 0
        }"#;

        let summary: AudioRenderSummary =
            serde_json::from_str(json).expect("legacy summary should deserialize");

        assert_eq!(summary.start_frame, 0);
        assert_eq!(summary.end_frame, 23);
        assert_eq!(summary.window_length_frames, 24);
        assert_eq!(summary.render_kind, AudioRenderKind::SamplePlayback);
        assert_eq!(summary.count_in_tick, None);
        assert_eq!(summary.bar_index, None);
        assert_eq!(summary.beat_index, None);
        assert_eq!(summary.accent, None);
    }

    #[test]
    fn count_in_click_render_is_deterministic_and_reports_metadata() {
        let intent = test_count_in_click_intent(96, 1, 1, true);
        let settings = settings(44_100, 64);

        let first = render_click(&intent, settings);
        let second = render_click(&intent, settings);

        assert_eq!(first.frames, second.frames);
        assert_eq!(first.summary, second.summary);
        assert_eq!(first.settings, settings);
        assert_eq!(first.summary.render_kind, AudioRenderKind::CountInClick);
        assert_eq!(
            first.summary.source_kind,
            AudioSourceKind::RightsSafeGenerated
        );
        assert_eq!(first.summary.loaded_audio_byte_count, 0);
        assert_eq!(first.summary.count_in_tick, Some(96));
        assert_eq!(first.summary.bar_index, Some(1));
        assert_eq!(first.summary.beat_index, Some(1));
        assert_eq!(first.summary.accent, Some(true));
        assert_eq!(first.summary.channel_balance, ChannelBalance::Center);
    }

    #[test]
    fn count_in_click_accent_peak_is_louder_than_non_accent_peak() {
        let settings = settings(44_100, 128);
        let accent = render_click(&test_count_in_click_intent(0, 1, 1, true), settings);
        let click = render_click(&test_count_in_click_intent(96, 1, 2, false), settings);

        assert!(accent.summary.peak_amplitude > click.summary.peak_amplitude);
        assert_eq!(accent.summary.source_sample_name, "COUNT-IN ACCENT");
        assert_eq!(click.summary.source_sample_name, "COUNT-IN CLICK");
    }

    #[test]
    fn disabled_host_audio_ignores_count_in_click_without_enqueueing() {
        let mut engine = HostAudioEngine::new(CaptureAudioBackend::new(4), settings(44_100, 64))
            .expect("host audio settings should be valid");

        let report = engine
            .play_count_in_click_with_render_summary(&test_count_in_click_intent(0, 1, 1, true));

        assert_eq!(
            report.event,
            HostAudioEvent::Ignored {
                backend_name: "capture".to_string(),
                reason: HostAudioIgnoreReason::Disabled,
            }
        );
        let summary = report
            .render_summary
            .expect("engine should expose count-in render summary for UI");
        assert_eq!(summary.render_kind, AudioRenderKind::CountInClick);
        assert_eq!(summary.accent, Some(true));
        let state = engine.state();
        assert_eq!(state.queued_render_count, 0);
        assert_eq!(state.played_render_count, 0);
        assert_eq!(engine.backend().captured_renders().len(), 0);
        assert_eq!(state.last_event.as_ref(), Some(&report.event));
    }

    #[test]
    fn enabled_host_audio_enqueues_and_captures_count_in_click() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 64))
                .expect("host audio settings should be valid");

        let report = engine
            .play_count_in_click_with_render_summary(&test_count_in_click_intent(96, 1, 2, false));

        let HostAudioEvent::Enqueued {
            backend_name,
            receipt,
        } = &report.event
        else {
            panic!("expected enqueued event, got {:?}", report.event);
        };
        assert_eq!(backend_name, "capture");
        assert_eq!(receipt.frame_count, 64);
        assert!(receipt.queued);
        assert!(receipt.played);
        assert_eq!(receipt.summary.render_kind, AudioRenderKind::CountInClick);
        assert_eq!(receipt.summary.count_in_tick, Some(96));
        assert_eq!(receipt.summary.beat_index, Some(2));
        assert_eq!(receipt.summary.accent, Some(false));

        let state = engine.state();
        assert_eq!(state.queued_render_count, 1);
        assert_eq!(state.played_render_count, 1);
        assert_eq!(state.last_event.as_ref(), Some(&report.event));
        let capture = engine
            .backend()
            .captured_renders()
            .back()
            .expect("capture should store latest count-in click");
        assert_eq!(capture.frame_count, 64);
        assert_eq!(capture.summary.render_kind, AudioRenderKind::CountInClick);
        assert_eq!(capture.summary.source_sample_name, "COUNT-IN CLICK");
    }

    #[test]
    fn renderer_reports_that_no_audio_bytes_are_loaded_from_disk() {
        let rendered = render(&test_intent(100, 100, 0), settings(44_100, 32));

        assert_eq!(
            rendered.summary.source_kind,
            AudioSourceKind::RightsSafeGenerated
        );
        assert_eq!(rendered.summary.loaded_audio_byte_count, 0);
    }

    #[test]
    fn settings_reject_oversized_frame_count_before_render_allocation() {
        let frame_count = MAX_RENDER_FRAMES + 1;
        let expected = AudioRenderError::FrameCountTooLarge {
            frame_count,
            max_frame_count: MAX_RENDER_FRAMES,
        };

        assert_eq!(
            AudioRenderSettings::new(DEFAULT_SAMPLE_RATE_HZ, frame_count),
            Err(expected)
        );

        let unchecked_settings = AudioRenderSettings {
            sample_rate_hz: DEFAULT_SAMPLE_RATE_HZ,
            frame_count,
        };
        assert_eq!(
            render_intent(&test_intent(100, 100, 0), unchecked_settings),
            Err(expected)
        );
    }

    #[test]
    fn settings_reject_sample_rates_outside_foundation_bounds() {
        assert_eq!(
            AudioRenderSettings::new(MIN_SAMPLE_RATE_HZ - 1, 1),
            Err(AudioRenderError::SampleRateBelowMinimum {
                sample_rate_hz: MIN_SAMPLE_RATE_HZ - 1,
                min_sample_rate_hz: MIN_SAMPLE_RATE_HZ,
            })
        );
        assert_eq!(
            AudioRenderSettings::new(MAX_SAMPLE_RATE_HZ + 1, 1),
            Err(AudioRenderError::SampleRateAboveMaximum {
                sample_rate_hz: MAX_SAMPLE_RATE_HZ + 1,
                max_sample_rate_hz: MAX_SAMPLE_RATE_HZ,
            })
        );

        assert!(AudioRenderSettings::new(MIN_SAMPLE_RATE_HZ, 1).is_ok());
        assert!(AudioRenderSettings::new(MAX_SAMPLE_RATE_HZ, 1).is_ok());
    }

    #[test]
    fn capture_backend_clamps_oversized_history_capacity_before_allocation() {
        let backend = CaptureAudioBackend::named("oversized", usize::MAX);

        assert_eq!(backend.max_captures(), MAX_CAPTURE_AUDIO_BACKEND_CAPTURES);
        assert_eq!(
            backend.captured_renders().capacity(),
            MAX_CAPTURE_AUDIO_BACKEND_CAPTURES
        );
    }

    #[test]
    fn host_audio_backend_receipt_constructors_expose_valid_states() {
        let queued = HostAudioBackendReceipt::queued(12);
        assert_eq!(queued.frame_count(), 12);
        assert!(queued.is_queued());
        assert!(!queued.is_played());

        let played = HostAudioBackendReceipt::queued_and_played(34);
        assert_eq!(played.frame_count(), 34);
        assert!(played.is_queued());
        assert!(played.is_played());
    }

    #[test]
    fn host_audio_playback_report_exposes_engine_render_summary_when_disabled() {
        let mut engine = HostAudioEngine::new(CaptureAudioBackend::new(4), settings(32_000, 24))
            .expect("host audio settings should be valid");

        let report = engine.play_intent_with_render_summary(&test_intent(100, 100, 0));

        assert_eq!(
            report.event,
            HostAudioEvent::Ignored {
                backend_name: "capture".to_string(),
                reason: HostAudioIgnoreReason::Disabled,
            }
        );
        let summary = report
            .render_summary
            .expect("engine should expose render summary for UI");
        assert_eq!(summary.sample_rate_hz, 32_000);
        assert_eq!(summary.frame_count, 24);
        assert_eq!(engine.backend().captured_renders().len(), 0);
    }

    #[test]
    fn enabled_host_audio_enqueues_and_captures_render_summary() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 64))
                .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        let HostAudioEvent::Enqueued {
            backend_name,
            receipt,
        } = &event
        else {
            panic!("expected enqueued event, got {event:?}");
        };
        assert_eq!(backend_name, "capture");
        assert_eq!(receipt.frame_count, 64);
        assert!(receipt.queued);
        assert!(receipt.played);
        assert_eq!(receipt.summary.source_sample_id, "synthetic_a_04");

        let state = engine.state();
        assert_eq!(state.mode, HostAudioMode::Enabled);
        assert_eq!(state.backend_name, "capture");
        assert_eq!(state.queued_render_count, 1);
        assert_eq!(state.played_render_count, 1);
        assert_eq!(state.last_event.as_ref(), Some(&event));
        assert_eq!(engine.backend().captured_renders().len(), 1);
        assert_eq!(
            engine
                .backend()
                .captured_renders()
                .back()
                .expect("capture should store latest render")
                .summary
                .source_sample_name,
            "SYN-A04"
        );
    }

    #[test]
    fn disabled_host_audio_ignores_without_enqueueing() {
        let mut engine = HostAudioEngine::new(CaptureAudioBackend::new(4), settings(44_100, 64))
            .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        assert_eq!(
            event,
            HostAudioEvent::Ignored {
                backend_name: "capture".to_string(),
                reason: HostAudioIgnoreReason::Disabled,
            }
        );
        let state = engine.state();
        assert_eq!(state.mode, HostAudioMode::Disabled);
        assert_eq!(state.queued_render_count, 0);
        assert_eq!(state.played_render_count, 0);
        assert_eq!(engine.backend().captured_renders().len(), 0);
        assert_eq!(state.last_event.as_ref(), Some(&event));
    }

    #[test]
    fn backend_errors_are_propagated_without_incrementing_counters() {
        let mut engine = HostAudioEngine::enabled(FailingAudioBackend, settings(44_100, 64))
            .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        let HostAudioEvent::Failed {
            backend_name,
            error,
            summary,
        } = &event
        else {
            panic!("expected failed event, got {event:?}");
        };
        assert_eq!(backend_name, "failing-test");
        assert!(matches!(
            error,
            HostAudioError::Backend {
                error: HostAudioBackendError::BackendUnavailable { .. }
            }
        ));
        assert_eq!(
            summary
                .as_ref()
                .expect("backend failure should retain render summary")
                .source_sample_id,
            "synthetic_a_04"
        );

        let state = engine.state();
        assert_eq!(state.queued_render_count, 0);
        assert_eq!(state.played_render_count, 0);
        assert_eq!(state.last_event.as_ref(), Some(&event));
    }

    #[test]
    fn host_audio_state_counters_track_multiple_playbacks() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(2), settings(44_100, 32))
                .expect("host audio settings should be valid");

        engine.play_intent(&test_intent(100, 100, -40));
        engine.play_intent(&test_intent(80, 100, 0));
        engine.play_intent(&test_intent(60, 100, 40));

        let state = engine.state();
        assert_eq!(state.queued_render_count, 3);
        assert_eq!(state.played_render_count, 3);
        assert_eq!(engine.backend().captured_renders().len(), 2);
        assert!(
            engine
                .backend()
                .captured_renders()
                .iter()
                .all(|capture| { capture.frame_count == 32 && capture.summary.frame_count == 32 })
        );
    }

    #[test]
    fn null_backend_accepts_playback_without_device_setup() {
        let mut engine = HostAudioEngine::enabled(NullAudioBackend::new(), settings(44_100, 16))
            .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        assert!(matches!(event, HostAudioEvent::Enqueued { .. }));
        let state = engine.state();
        assert_eq!(state.backend_name, "null");
        assert_eq!(state.queued_render_count, 1);
        assert_eq!(state.played_render_count, 1);
    }

    #[test]
    fn host_audio_rejects_invalid_rendered_audio_before_backend_enqueue() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 64))
                .expect("host audio settings should be valid");
        let mut rendered = render(&test_intent(100, 100, 0), settings(44_100, 64));
        rendered.summary.frame_count = 63;

        let event = engine.play_rendered(rendered);

        assert!(matches!(
            event,
            HostAudioEvent::Failed {
                error: HostAudioError::InvalidRenderedAudio { .. },
                ..
            }
        ));
        let state = engine.state();
        assert_eq!(state.queued_render_count, 0);
        assert_eq!(state.played_render_count, 0);
        assert_eq!(state.active_voice_count, 0);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.stolen_voice_count, 0);
        assert!(state.active_voices.is_empty());
        assert_eq!(engine.backend().captured_renders().len(), 0);
    }

    #[test]
    fn host_audio_rejects_played_without_queued_backend_receipt() {
        let receipt = HostAudioBackendReceipt {
            frame_count: 64,
            queued: false,
            played: true,
        };
        let mut engine =
            HostAudioEngine::enabled(ReceiptAudioBackend::new(receipt), settings(44_100, 64))
                .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        assert!(matches!(
            event,
            HostAudioEvent::Failed {
                error: HostAudioError::InvalidBackendReceipt {
                    error: InvalidBackendReceiptError::PlayedWithoutQueued
                },
                ..
            }
        ));
        let state = engine.state();
        assert_eq!(state.queued_render_count, 0);
        assert_eq!(state.played_render_count, 0);
        assert_eq!(state.active_voice_count, 0);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.stolen_voice_count, 0);
        assert!(state.active_voices.is_empty());
    }

    #[test]
    fn host_audio_rejects_backend_receipt_frame_count_mismatch() {
        let mut engine = HostAudioEngine::enabled(
            ReceiptAudioBackend::new(HostAudioBackendReceipt::queued_and_played(65)),
            settings(44_100, 64),
        )
        .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        assert!(matches!(
            event,
            HostAudioEvent::Failed {
                error: HostAudioError::InvalidBackendReceipt {
                    error: InvalidBackendReceiptError::FrameCountMismatch {
                        receipt_frame_count: 65,
                        rendered_frame_count: 64,
                    }
                },
                ..
            }
        ));
        let state = engine.state();
        assert_eq!(state.queued_render_count, 0);
        assert_eq!(state.played_render_count, 0);
        assert_eq!(state.active_voice_count, 0);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.stolen_voice_count, 0);
        assert!(state.active_voices.is_empty());
    }

    #[test]
    fn enabled_sample_playback_allocates_default_voice() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 64))
                .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        let HostAudioEvent::Enqueued { receipt, .. } = &event else {
            panic!("expected enqueued event, got {event:?}");
        };
        let allocation = receipt
            .voice_allocation
            .as_ref()
            .expect("queued render should allocate a voice");
        assert_eq!(allocation.voice_id, 1);
        assert_eq!(allocation.stolen_voice_id, None);
        assert_eq!(allocation.voice_limit, DEFAULT_HOST_AUDIO_VOICE_LIMIT);
        assert_eq!(allocation.active_voice_count, 1);

        let state = engine.state();
        assert_eq!(state.voice_limit, DEFAULT_HOST_AUDIO_VOICE_LIMIT);
        assert_eq!(state.active_voice_count, 1);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.stolen_voice_count, 0);
        assert_eq!(
            state.active_voices,
            vec![HostAudioVoiceSummary {
                voice_id: 1,
                render_kind: AudioRenderKind::SamplePlayback,
                source_label: "SYN-A04".to_string(),
                source_sample_id: "synthetic_a_04".to_string(),
                bank: PadBank::A,
                pad_number: 4,
                total_frame_count: 64,
                remaining_frame_count: 64,
            }]
        );
    }

    #[test]
    fn zero_frame_queued_render_does_not_allocate_voice() {
        let mut engine = HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 0))
            .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        let HostAudioEvent::Enqueued { receipt, .. } = &event else {
            panic!("expected enqueued event, got {event:?}");
        };
        assert!(receipt.queued);
        assert!(receipt.played);
        assert_eq!(receipt.frame_count, 0);
        assert_eq!(receipt.voice_allocation, None);

        let state = engine.state();
        assert_eq!(state.queued_render_count, 1);
        assert_eq!(state.played_render_count, 1);
        assert_eq!(state.active_voice_count, 0);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.stolen_voice_count, 0);
        assert!(state.active_voices.is_empty());
    }

    #[test]
    fn disabled_sample_playback_does_not_allocate_voice() {
        let mut engine = HostAudioEngine::new(CaptureAudioBackend::new(4), settings(44_100, 64))
            .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        assert!(matches!(event, HostAudioEvent::Ignored { .. }));
        let state = engine.state();
        assert_eq!(state.queued_render_count, 0);
        assert_eq!(state.played_render_count, 0);
        assert_eq!(state.active_voice_count, 0);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.stolen_voice_count, 0);
        assert!(state.active_voices.is_empty());
    }

    #[test]
    fn backend_failure_does_not_allocate_voice() {
        let mut engine = HostAudioEngine::enabled(FailingAudioBackend, settings(44_100, 64))
            .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        assert!(matches!(event, HostAudioEvent::Failed { .. }));
        let state = engine.state();
        assert_eq!(state.queued_render_count, 0);
        assert_eq!(state.played_render_count, 0);
        assert_eq!(state.active_voice_count, 0);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.stolen_voice_count, 0);
        assert!(state.active_voices.is_empty());
    }

    #[test]
    fn advance_voice_frames_completes_active_voice() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 64))
                .expect("host audio settings should be valid");
        engine.play_intent(&test_intent(100, 100, 0));

        engine.advance_voice_frames(0);
        assert_eq!(engine.state().active_voices[0].remaining_frame_count, 64);

        engine.advance_voice_frames(63);
        let state = engine.state();
        assert_eq!(state.active_voice_count, 1);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.active_voices[0].remaining_frame_count, 1);

        engine.advance_voice_frames(1);
        let state = engine.state();
        assert_eq!(state.active_voice_count, 0);
        assert_eq!(state.completed_voice_count, 1);
        assert!(state.active_voices.is_empty());
    }

    #[test]
    fn voice_limit_steals_oldest_active_voice() {
        let mut engine = HostAudioEngine::enabled_with_voice_limit(
            CaptureAudioBackend::new(4),
            settings(44_100, 32),
            2,
        )
        .expect("host audio settings should be valid");

        engine.play_intent(&test_intent_with_sample("sample-1", "SYN-01"));
        engine.play_intent(&test_intent_with_sample("sample-2", "SYN-02"));
        let event = engine.play_intent(&test_intent_with_sample("sample-3", "SYN-03"));

        let HostAudioEvent::Enqueued { receipt, .. } = &event else {
            panic!("expected enqueued event, got {event:?}");
        };
        let allocation = receipt
            .voice_allocation
            .as_ref()
            .expect("queued render should allocate a voice");
        assert_eq!(allocation.voice_id, 3);
        assert_eq!(allocation.stolen_voice_id, Some(1));
        assert_eq!(allocation.voice_limit, 2);
        assert_eq!(allocation.active_voice_count, 2);

        let state = engine.state();
        assert_eq!(state.voice_limit, 2);
        assert_eq!(state.active_voice_count, 2);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.stolen_voice_count, 1);
        assert_eq!(
            state
                .active_voices
                .iter()
                .map(|voice| (voice.voice_id, voice.source_label.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "SYN-02"), (3, "SYN-03")]
        );
    }

    #[test]
    fn count_in_click_allocates_voice_through_same_host_path() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 64))
                .expect("host audio settings should be valid");

        let event = engine.play_count_in_click(&test_count_in_click_intent(96, 1, 2, false));

        let HostAudioEvent::Enqueued { receipt, .. } = &event else {
            panic!("expected enqueued event, got {event:?}");
        };
        let allocation = receipt
            .voice_allocation
            .as_ref()
            .expect("queued count-in click should allocate a voice");
        assert_eq!(allocation.voice_id, 1);
        assert_eq!(allocation.stolen_voice_id, None);
        assert_eq!(receipt.summary.render_kind, AudioRenderKind::CountInClick);

        let state = engine.state();
        assert_eq!(state.active_voice_count, 1);
        assert_eq!(
            state.active_voices,
            vec![HostAudioVoiceSummary {
                voice_id: 1,
                render_kind: AudioRenderKind::CountInClick,
                source_label: "COUNT-IN CLICK".to_string(),
                source_sample_id: "count_in_click".to_string(),
                bank: PadBank::A,
                pad_number: 0,
                total_frame_count: 64,
                remaining_frame_count: 64,
            }]
        );
    }

    #[test]
    fn release_intent_removes_matching_active_sample_voices() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 64))
                .expect("host audio settings should be valid");
        engine.play_intent(&test_intent_with_sample("sample-1", "SYN-01"));
        engine.play_intent(&test_intent_with_sample("sample-2", "SYN-02"));
        engine.play_intent(&test_intent_with_sample("sample-1", "SYN-01"));

        let event = engine.release_intent(&test_release_intent("sample-1", "SYN-01"));

        let HostAudioEvent::Released { receipt, .. } = &event else {
            panic!("expected released event, got {event:?}");
        };
        assert_eq!(receipt.released_voice_ids, vec![1, 3]);
        assert_eq!(receipt.released_voice_count, 2);
        assert_eq!(receipt.active_voice_count, 1);

        let state = engine.state();
        assert_eq!(state.active_voice_count, 1);
        assert_eq!(state.released_voice_count, 2);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(
            state
                .active_voices
                .iter()
                .map(|voice| (voice.voice_id, voice.source_sample_id.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "sample-2")]
        );
    }

    #[test]
    fn release_intent_does_not_remove_count_in_or_other_pads() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 64))
                .expect("host audio settings should be valid");
        engine.play_intent(&test_intent_with_sample("sample-1", "SYN-01"));
        engine.play_count_in_click(&test_count_in_click_intent(96, 1, 2, false));
        let mut release = test_release_intent("sample-1", "SYN-01");
        release.pad_number = 5;

        let event = engine.release_intent(&release);

        let HostAudioEvent::Released { receipt, .. } = &event else {
            panic!("expected released event, got {event:?}");
        };
        assert!(receipt.released_voice_ids.is_empty());
        assert_eq!(receipt.active_voice_count, 2);
        let state = engine.state();
        assert_eq!(state.released_voice_count, 0);
        assert_eq!(state.active_voice_count, 2);
    }

    #[test]
    fn disabled_release_intent_is_ignored_without_voice_mutation() {
        let mut engine =
            HostAudioEngine::enabled(CaptureAudioBackend::new(4), settings(44_100, 64))
                .expect("host audio settings should be valid");
        engine.play_intent(&test_intent_with_sample("sample-1", "SYN-01"));
        let before_release = engine.state();
        assert_eq!(before_release.active_voice_count, 1);

        engine.set_mode(HostAudioMode::Disabled);

        let event = engine.release_intent(&test_release_intent("sample-1", "SYN-01"));

        assert!(matches!(event, HostAudioEvent::Ignored { .. }));
        let state = engine.state();
        assert_eq!(state.released_voice_count, 0);
        assert_eq!(state.active_voices, before_release.active_voices);
        assert_eq!(state.active_voice_count, before_release.active_voice_count);
    }

    #[test]
    fn host_audio_voice_summary_deserializes_legacy_json_without_release_metadata() {
        let summary: HostAudioVoiceSummary = serde_json::from_value(serde_json::json!({
            "voice_id": 7,
            "render_kind": "sample_playback",
            "source_label": "SYN-A01",
            "total_frame_count": 64,
            "remaining_frame_count": 32
        }))
        .expect("legacy voice summary should deserialize with metadata defaults");

        assert_eq!(
            summary,
            HostAudioVoiceSummary {
                voice_id: 7,
                render_kind: AudioRenderKind::SamplePlayback,
                source_label: "SYN-A01".to_string(),
                source_sample_id: String::new(),
                bank: PadBank::A,
                pad_number: 0,
                total_frame_count: 64,
                remaining_frame_count: 32,
            }
        );
    }

    #[test]
    fn non_queued_backend_receipt_does_not_allocate_voice() {
        let mut engine = HostAudioEngine::enabled(
            ReceiptAudioBackend::new(HostAudioBackendReceipt::not_queued(64)),
            settings(44_100, 64),
        )
        .expect("host audio settings should be valid");

        let event = engine.play_intent(&test_intent(100, 100, 0));

        let HostAudioEvent::Enqueued { receipt, .. } = event else {
            panic!("expected receipt event, got {event:?}");
        };
        assert!(!receipt.queued);
        assert!(!receipt.played);
        assert_eq!(receipt.voice_allocation, None);
        let state = engine.state();
        assert_eq!(state.queued_render_count, 0);
        assert_eq!(state.played_render_count, 0);
        assert_eq!(state.active_voice_count, 0);
        assert_eq!(state.completed_voice_count, 0);
        assert_eq!(state.stolen_voice_count, 0);
        assert!(state.active_voices.is_empty());
    }

    #[test]
    fn configurable_voice_limit_is_clamped_to_foundation_bounds() {
        let low = HostAudioEngine::enabled_with_voice_limit(
            NullAudioBackend::new(),
            settings(44_100, 16),
            0,
        )
        .expect("host audio settings should be valid");
        let high = HostAudioEngine::enabled_with_voice_limit(
            NullAudioBackend::new(),
            settings(44_100, 16),
            usize::MAX,
        )
        .expect("host audio settings should be valid");

        assert_eq!(low.voice_limit(), MIN_HOST_AUDIO_VOICE_LIMIT);
        assert_eq!(high.voice_limit(), MAX_HOST_AUDIO_VOICE_LIMIT);
    }

    fn settings(sample_rate_hz: u32, frame_count: usize) -> AudioRenderSettings {
        AudioRenderSettings::new(sample_rate_hz, frame_count)
            .expect("test settings should satisfy audio render guardrails")
    }

    fn render(intent: &SamplePlaybackIntent, settings: AudioRenderSettings) -> RenderedAudio {
        render_intent(intent, settings).expect("test render settings should be valid")
    }

    fn render_click(intent: &CountInClickIntent, settings: AudioRenderSettings) -> RenderedAudio {
        render_count_in_click(intent, settings).expect("test render settings should be valid")
    }

    fn test_intent(velocity: u8, level: u8, pan: i8) -> SamplePlaybackIntent {
        test_intent_with_tune(velocity, level, pan, 0)
    }

    fn test_intent_with_tune(
        velocity: u8,
        level: u8,
        pan: i8,
        tune_cents: i16,
    ) -> SamplePlaybackIntent {
        SamplePlaybackIntent {
            selected_track: 1,
            program_index: 1,
            program_name: "Program01".to_string(),
            bank: PadBank::A,
            pad_number: 4,
            sample_id: "synthetic_a_04".to_string(),
            sample_name: "SYN-A04".to_string(),
            velocity,
            level,
            pan,
            tune_cents,
            start_frame: 0,
            end_frame: 51_599,
            window_length_frames: 51_600,
        }
    }

    fn test_intent_with_sample(sample_id: &str, sample_name: &str) -> SamplePlaybackIntent {
        SamplePlaybackIntent {
            sample_id: sample_id.to_string(),
            sample_name: sample_name.to_string(),
            ..test_intent(100, 100, 0)
        }
    }

    fn test_release_intent(sample_id: &str, sample_name: &str) -> SampleReleaseIntent {
        SampleReleaseIntent {
            selected_track: 1,
            program_index: 1,
            program_name: "Program01".to_string(),
            bank: PadBank::A,
            pad_number: 4,
            sample_id: sample_id.to_string(),
            sample_name: sample_name.to_string(),
            release_velocity: 64,
        }
    }

    fn test_count_in_click_intent(
        count_in_tick: u64,
        bar_index: u8,
        beat_index: u8,
        accent: bool,
    ) -> CountInClickIntent {
        CountInClickIntent {
            count_in_tick,
            bar_index,
            beat_index,
            accent,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct FailingAudioBackend;

    impl HostAudioBackend for FailingAudioBackend {
        fn backend_name(&self) -> &str {
            "failing-test"
        }

        fn enqueue_render(
            &mut self,
            _rendered: &RenderedAudio,
        ) -> Result<HostAudioBackendReceipt, HostAudioBackendError> {
            Err(HostAudioBackendError::backend_unavailable(
                self.backend_name(),
                "injected test failure",
            ))
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct ReceiptAudioBackend {
        receipt: HostAudioBackendReceipt,
    }

    impl ReceiptAudioBackend {
        fn new(receipt: HostAudioBackendReceipt) -> Self {
            Self { receipt }
        }
    }

    impl HostAudioBackend for ReceiptAudioBackend {
        fn backend_name(&self) -> &str {
            "receipt-test"
        }

        fn enqueue_render(
            &mut self,
            _rendered: &RenderedAudio,
        ) -> Result<HostAudioBackendReceipt, HostAudioBackendError> {
            Ok(self.receipt)
        }
    }
}
