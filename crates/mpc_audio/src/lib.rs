use mpc_core::{PadBank, SamplePlaybackIntent};
use serde::{Deserialize, Serialize};
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
const PCM_MAX: i32 = i16::MAX as i32;
const MAX_VELOCITY: i32 = 127;
const MAX_LEVEL: i32 = 127;
const PAN_RANGE: i8 = 100;
const FNV_OFFSET_BASIS: u32 = 2_166_136_261;
const FNV_PRIME: u32 = 16_777_619;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioRenderSummary {
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
    pub peak_left: i16,
    pub peak_right: i16,
    pub peak_amplitude: i16,
    pub channel_balance: ChannelBalance,
    pub source_kind: AudioSourceKind,
    pub loaded_audio_byte_count: usize,
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
    last_event: Option<HostAudioEvent>,
}

impl<B> HostAudioEngine<B>
where
    B: HostAudioBackend,
{
    pub fn new(backend: B, render_settings: AudioRenderSettings) -> Result<Self, HostAudioError> {
        render_settings.validate().map_err(HostAudioError::render)?;
        Ok(Self {
            backend,
            mode: HostAudioMode::Disabled,
            render_settings,
            queued_render_count: 0,
            played_render_count: 0,
            last_event: None,
        })
    }

    pub fn enabled(
        backend: B,
        render_settings: AudioRenderSettings,
    ) -> Result<Self, HostAudioError> {
        let mut engine = Self::new(backend, render_settings)?;
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

                self.record_enqueued(HostAudioRenderReceipt {
                    summary,
                    frame_count: backend_receipt.frame_count(),
                    queued: backend_receipt.is_queued(),
                    played: backend_receipt.is_played(),
                })
            }
            Err(error) => self.record_failed(HostAudioError::backend(error), Some(summary)),
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

    let seed = stable_seed(intent);
    let mono_peak = scaled_mono_peak(intent.velocity, intent.level);
    let pan = intent.pan.clamp(-PAN_RANGE, PAN_RANGE);
    let (left_gain, right_gain) = stereo_gains(pan);
    let mut frames = Vec::with_capacity(settings.frame_count);
    let mut peak_left = 0_i16;
    let mut peak_right = 0_i16;

    for frame_index in 0..settings.frame_count {
        let wave = seeded_square_wave(seed, frame_index, intent.tune_cents);
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
        sample_rate_hz: settings.sample_rate_hz,
        frame_count: settings.frame_count,
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
        peak_left,
        peak_right,
        peak_amplitude,
        channel_balance,
        source_kind: AudioSourceKind::RightsSafeGenerated,
        loaded_audio_byte_count: 0,
    };

    Ok(RenderedAudio {
        settings,
        summary,
        frames,
    })
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
    use mpc_core::PadBank;

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
    }

    fn settings(sample_rate_hz: u32, frame_count: usize) -> AudioRenderSettings {
        AudioRenderSettings::new(sample_rate_hz, frame_count)
            .expect("test settings should satisfy audio render guardrails")
    }

    fn render(intent: &SamplePlaybackIntent, settings: AudioRenderSettings) -> RenderedAudio {
        render_intent(intent, settings).expect("test render settings should be valid")
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
