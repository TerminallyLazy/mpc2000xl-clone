use mpc_core::{PadBank, SamplePlaybackIntent};
use serde::{Deserialize, Serialize};

const DEFAULT_SAMPLE_RATE_HZ: u32 = 44_100;
const DEFAULT_FRAME_COUNT: usize = 512;
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
    pub fn new(sample_rate_hz: u32, frame_count: usize) -> Self {
        Self {
            sample_rate_hz,
            frame_count,
        }
    }

    pub fn preview() -> Self {
        Self {
            sample_rate_hz: DEFAULT_SAMPLE_RATE_HZ,
            frame_count: 256,
        }
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

pub fn render_intent(
    intent: &SamplePlaybackIntent,
    settings: AudioRenderSettings,
) -> RenderedAudio {
    let seed = stable_seed(intent);
    let mono_peak = scaled_mono_peak(intent.velocity, intent.level);
    let pan = intent.pan.clamp(-PAN_RANGE, PAN_RANGE);
    let (left_gain, right_gain) = stereo_gains(pan);
    let mut frames = Vec::with_capacity(settings.frame_count);
    let mut peak_left = 0_i16;
    let mut peak_right = 0_i16;

    for frame_index in 0..settings.frame_count {
        let wave = seeded_square_wave(seed, frame_index);
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
        peak_left,
        peak_right,
        peak_amplitude,
        channel_balance,
        source_kind: AudioSourceKind::RightsSafeGenerated,
        loaded_audio_byte_count: 0,
    };

    RenderedAudio {
        settings,
        summary,
        frames,
    }
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

fn seeded_square_wave(seed: u32, frame_index: usize) -> i32 {
    let period = 12 + usize::try_from(seed % 53).expect("period seed fits usize");
    let duty = 1 + usize::try_from((seed >> 8) % (period as u32 - 1)).expect("duty fits usize");
    let phase =
        (frame_index + usize::try_from(seed >> 16).expect("phase seed fits usize")) % period;

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
        let settings = AudioRenderSettings::new(48_000, 96);

        let first = render_intent(&intent, settings);
        let second = render_intent(&intent, settings);

        assert_eq!(first.frames, second.frames);
        assert_eq!(first.summary, second.summary);
    }

    #[test]
    fn velocity_and_level_affect_peak_amplitude() {
        let quiet_velocity = render_intent(
            &test_intent(40, 100, 0),
            AudioRenderSettings::new(44_100, 128),
        );
        let loud_velocity = render_intent(
            &test_intent(100, 100, 0),
            AudioRenderSettings::new(44_100, 128),
        );
        let quiet_level = render_intent(
            &test_intent(100, 40, 0),
            AudioRenderSettings::new(44_100, 128),
        );

        assert!(loud_velocity.summary.peak_amplitude > quiet_velocity.summary.peak_amplitude);
        assert!(loud_velocity.summary.peak_amplitude > quiet_level.summary.peak_amplitude);
    }

    #[test]
    fn pan_affects_left_and_right_channels() {
        let left = render_intent(
            &test_intent(100, 100, -60),
            AudioRenderSettings::new(44_100, 128),
        );
        let center = render_intent(
            &test_intent(100, 100, 0),
            AudioRenderSettings::new(44_100, 128),
        );
        let right = render_intent(
            &test_intent(100, 100, 60),
            AudioRenderSettings::new(44_100, 128),
        );

        assert!(left.summary.peak_left > left.summary.peak_right);
        assert_eq!(left.summary.channel_balance, ChannelBalance::Left);
        assert_eq!(center.summary.peak_left, center.summary.peak_right);
        assert_eq!(center.summary.channel_balance, ChannelBalance::Center);
        assert!(right.summary.peak_right > right.summary.peak_left);
        assert_eq!(right.summary.channel_balance, ChannelBalance::Right);
    }

    #[test]
    fn render_length_and_sample_rate_are_respected() {
        let rendered = render_intent(
            &test_intent(100, 100, 0),
            AudioRenderSettings::new(32_000, 17),
        );

        assert_eq!(rendered.frames.len(), 17);
        assert_eq!(rendered.settings.sample_rate_hz, 32_000);
        assert_eq!(rendered.summary.sample_rate_hz, 32_000);
        assert_eq!(rendered.summary.frame_count, 17);
    }

    #[test]
    fn renderer_reports_that_no_audio_bytes_are_loaded_from_disk() {
        let rendered = render_intent(
            &test_intent(100, 100, 0),
            AudioRenderSettings::new(44_100, 32),
        );

        assert_eq!(
            rendered.summary.source_kind,
            AudioSourceKind::RightsSafeGenerated
        );
        assert_eq!(rendered.summary.loaded_audio_byte_count, 0);
    }

    fn test_intent(velocity: u8, level: u8, pan: i8) -> SamplePlaybackIntent {
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
        }
    }
}
