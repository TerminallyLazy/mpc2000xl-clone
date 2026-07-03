use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use crate::events::{
    PadAssignment, PadBank, ProgramPad, SamplePlaybackIntent, SamplePlaybackResolution,
    SampleSourceKind, SampleTrim, SyntheticSample, sample_window_length_frames,
};
use crate::state::{ProjectImportedMediaReference, ProjectSnapshot};

pub const SAMPLE_FLIP_PAD_COUNT: u8 = 16;
const DEFAULT_LEVEL: u8 = 100;
const DEFAULT_PAN: i8 = 0;
const DEFAULT_TUNE_CENTS: i16 = 0;
const DEFAULT_MUTE_GROUP: u8 = 0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleFlipSource {
    pub source_id: String,
    pub source_title: String,
    pub source_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_copy_path: Option<String>,
    pub sample_rate_hz: u32,
    pub frame_count: u32,
    pub byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleFlipRegion {
    pub start_frame: u32,
    pub end_frame: u32,
}

impl SampleFlipRegion {
    pub fn window_length_frames(self) -> u32 {
        sample_window_length_frames(self.start_frame, self.end_frame)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleFlipPadSlice {
    pub pad: ProgramPad,
    pub sample_id: String,
    pub sample_name: String,
    pub start_frame: u32,
    pub end_frame: u32,
    pub level: u8,
    pub pan: i8,
    pub tune_cents: i16,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub mute_group: u8,
}

impl SampleFlipPadSlice {
    pub fn window_length_frames(&self) -> u32 {
        sample_window_length_frames(self.start_frame, self.end_frame)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleFlipPlan {
    pub source: SampleFlipSource,
    pub bank: PadBank,
    pub region: SampleFlipRegion,
    pub slices: Vec<SampleFlipPadSlice>,
}

impl SampleFlipPlan {
    pub fn sample_ids(&self) -> BTreeSet<String> {
        self.slices
            .iter()
            .map(|slice| slice.sample_id.clone())
            .collect()
    }

    pub fn slice_for_pad(&self, pad_number: u8) -> Option<&SampleFlipPadSlice> {
        self.slices
            .iter()
            .find(|slice| slice.pad.pad_number == pad_number)
    }

    pub fn program_name(&self) -> String {
        truncate_chars(&format!("Flip {}", sample_flip_name_root(&self.source)), 32)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampleFlipError {
    InvalidSource { message: String },
    InvalidRegion { message: String },
    InvalidPlan { message: String },
}

impl fmt::Display for SampleFlipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSource { message }
            | Self::InvalidRegion { message }
            | Self::InvalidPlan { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SampleFlipError {}

pub fn build_pad_bank_sample_flip_plan(
    source: SampleFlipSource,
    bank: PadBank,
    region: Option<SampleFlipRegion>,
) -> Result<SampleFlipPlan, SampleFlipError> {
    validate_source(&source)?;
    let region = region.unwrap_or(SampleFlipRegion {
        start_frame: 0,
        end_frame: source.frame_count.saturating_sub(1),
    });
    validate_region(region, source.frame_count, SAMPLE_FLIP_PAD_COUNT)?;

    let token = sample_flip_source_token(&source);
    let name_root = sample_flip_name_root(&source);
    let bank_token = bank.label().to_ascii_lowercase();
    let region_start = u64::from(region.start_frame);
    let region_length = u64::from(region.window_length_frames());
    let pad_count = u64::from(SAMPLE_FLIP_PAD_COUNT);
    let mut slices = Vec::with_capacity(usize::from(SAMPLE_FLIP_PAD_COUNT));

    for pad_index in 0..SAMPLE_FLIP_PAD_COUNT {
        let pad_number = pad_index + 1;
        let start_offset = region_length * u64::from(pad_index) / pad_count;
        let end_offset = region_length * u64::from(pad_index + 1) / pad_count;
        let start_frame = u32::try_from(region_start + start_offset)
            .expect("validated sample flip start frame should fit u32");
        let end_frame = u32::try_from(region_start + end_offset - 1)
            .expect("validated sample flip end frame should fit u32");
        let pad = ProgramPad { bank, pad_number };

        slices.push(SampleFlipPadSlice {
            pad,
            sample_id: format!("flip_{token}_{bank_token}_p{pad_number:02}"),
            sample_name: format!("{} {}", name_root, program_pad_label(pad)),
            start_frame,
            end_frame,
            level: DEFAULT_LEVEL,
            pan: DEFAULT_PAN,
            tune_cents: DEFAULT_TUNE_CENTS,
            mute_group: DEFAULT_MUTE_GROUP,
        });
    }

    let plan = SampleFlipPlan {
        source,
        bank,
        region,
        slices,
    };
    validate_plan(&plan)?;
    Ok(plan)
}

pub fn apply_sample_flip_plan_to_project_snapshot(
    snapshot: &mut ProjectSnapshot,
    plan: &SampleFlipPlan,
) -> Result<(), SampleFlipError> {
    validate_plan(plan)?;

    let removed_sample_ids: BTreeSet<String> = snapshot
        .program
        .pad_assignments
        .iter()
        .filter(|assignment| assignment.pad.bank == plan.bank)
        .map(|assignment| assignment.sample.id.clone())
        .collect();
    let plan_sample_ids = plan.sample_ids();

    snapshot.program.pad_assignments.retain(|assignment| {
        assignment.pad.bank != plan.bank && !plan_sample_ids.contains(&assignment.sample.id)
    });
    snapshot.program.sample_trims.retain(|trim| {
        !removed_sample_ids.contains(&trim.sample_id) && !plan_sample_ids.contains(&trim.sample_id)
    });
    snapshot
        .program
        .imported_media_references
        .retain(|reference| {
            !removed_sample_ids.contains(&reference.sample_id)
                && !plan_sample_ids.contains(&reference.sample_id)
        });

    for slice in &plan.slices {
        snapshot.program.pad_assignments.push(PadAssignment {
            pad: slice.pad,
            sample: SyntheticSample {
                id: slice.sample_id.clone(),
                name: slice.sample_name.clone(),
                source_kind: SampleSourceKind::Imported,
                length_frames: Some(plan.source.frame_count),
            },
            level: slice.level,
            pan: slice.pan,
            tune_cents: slice.tune_cents,
            mute_group: slice.mute_group,
        });
        snapshot.program.sample_trims.push(SampleTrim {
            sample_id: slice.sample_id.clone(),
            start_frame: slice.start_frame,
            end_frame: slice.end_frame,
        });
        snapshot
            .program
            .imported_media_references
            .push(ProjectImportedMediaReference {
                sample_id: slice.sample_id.clone(),
                source_path: plan.source.source_path.clone(),
                managed_copy_path: plan.source.managed_copy_path.clone(),
                sample_name: slice.sample_name.clone(),
                sample_rate_hz: plan.source.sample_rate_hz,
                frame_count: plan.source.frame_count,
                byte_count: plan.source.byte_count,
                source_kind: SampleSourceKind::Imported,
            });
    }

    snapshot.program.name = plan.program_name();
    snapshot
        .program
        .pad_assignments
        .sort_by_key(|assignment| assignment.pad);
    snapshot
        .program
        .sample_trims
        .sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
    snapshot
        .program
        .imported_media_references
        .sort_by(|left, right| left.sample_id.cmp(&right.sample_id));

    let program_index = snapshot.program.index;
    let program_name = snapshot.program.name.clone();
    for event in &mut snapshot.sequence.recorded_events {
        if event.pad_bank == plan.bank {
            event.playback = plan.slice_for_pad(event.pad_number).map(|slice| {
                playback_intent_from_slice(
                    slice,
                    program_index,
                    &program_name,
                    event.selected_track,
                    event.velocity,
                )
            });
        }
    }

    snapshot.machine.last_playback = match snapshot.machine.last_playback.take() {
        Some(SamplePlaybackResolution::Intent { intent }) if intent.bank == plan.bank => plan
            .slice_for_pad(intent.pad_number)
            .map(|slice| SamplePlaybackResolution::Intent {
                intent: playback_intent_from_slice(
                    slice,
                    program_index,
                    &program_name,
                    intent.selected_track,
                    intent.velocity,
                ),
            }),
        other => other,
    };

    if let Some(first_slice) = plan.slices.first() {
        snapshot.machine.pad_bank = plan.bank;
        snapshot.machine.selected_program_pad = first_slice.pad;
        snapshot.machine.selected_sample_id = Some(first_slice.sample_id.clone());
    }

    Ok(())
}

fn validate_plan(plan: &SampleFlipPlan) -> Result<(), SampleFlipError> {
    validate_source(&plan.source)?;
    validate_region(plan.region, plan.source.frame_count, SAMPLE_FLIP_PAD_COUNT)?;
    if plan.slices.len() != usize::from(SAMPLE_FLIP_PAD_COUNT) {
        return Err(SampleFlipError::InvalidPlan {
            message: format!(
                "sample flip plan has {} slices, expected {}",
                plan.slices.len(),
                SAMPLE_FLIP_PAD_COUNT
            ),
        });
    }

    let mut pads = BTreeSet::new();
    let mut sample_ids = BTreeSet::new();
    for slice in &plan.slices {
        if slice.pad.bank != plan.bank {
            return Err(SampleFlipError::InvalidPlan {
                message: format!(
                    "sample flip slice {} should be in bank {}",
                    program_pad_label(slice.pad),
                    plan.bank.label()
                ),
            });
        }
        if !(1..=SAMPLE_FLIP_PAD_COUNT).contains(&slice.pad.pad_number) {
            return Err(SampleFlipError::InvalidPlan {
                message: format!("sample flip pad {} is out of range", slice.pad.pad_number),
            });
        }
        if !pads.insert(slice.pad) {
            return Err(SampleFlipError::InvalidPlan {
                message: format!("duplicate sample flip pad {}", program_pad_label(slice.pad)),
            });
        }
        if !sample_ids.insert(slice.sample_id.clone()) {
            return Err(SampleFlipError::InvalidPlan {
                message: format!("duplicate sample flip id {:?}", slice.sample_id),
            });
        }
        validate_region(
            SampleFlipRegion {
                start_frame: slice.start_frame,
                end_frame: slice.end_frame,
            },
            plan.source.frame_count,
            1,
        )?;
    }

    Ok(())
}

fn validate_source(source: &SampleFlipSource) -> Result<(), SampleFlipError> {
    if source.frame_count == 0 {
        return Err(SampleFlipError::InvalidSource {
            message: format!("sample flip source {:?} has no frames", source.source_title),
        });
    }
    if source.sample_rate_hz == 0 {
        return Err(SampleFlipError::InvalidSource {
            message: "sample flip source sample rate must be non-zero".to_string(),
        });
    }
    if source.byte_count == 0 {
        return Err(SampleFlipError::InvalidSource {
            message: "sample flip source byte count must be non-zero".to_string(),
        });
    }
    Ok(())
}

fn validate_region(
    region: SampleFlipRegion,
    frame_count: u32,
    pad_count: u8,
) -> Result<(), SampleFlipError> {
    if region.start_frame > region.end_frame || region.end_frame >= frame_count {
        return Err(SampleFlipError::InvalidRegion {
            message: format!(
                "sample flip region {}..={} is outside source length {}",
                region.start_frame, region.end_frame, frame_count
            ),
        });
    }
    let window_length_frames = region.window_length_frames();
    if window_length_frames < u32::from(pad_count) {
        return Err(SampleFlipError::InvalidRegion {
            message: format!(
                "sample flip region has {window_length_frames} frame(s), which cannot fill {pad_count} pad(s)"
            ),
        });
    }
    Ok(())
}

fn playback_intent_from_slice(
    slice: &SampleFlipPadSlice,
    program_index: u8,
    program_name: &str,
    selected_track: u8,
    velocity: u8,
) -> SamplePlaybackIntent {
    SamplePlaybackIntent {
        selected_track,
        program_index,
        program_name: program_name.to_string(),
        bank: slice.pad.bank,
        pad_number: slice.pad.pad_number,
        sample_id: slice.sample_id.clone(),
        sample_name: slice.sample_name.clone(),
        velocity,
        level: slice.level,
        pan: slice.pan,
        tune_cents: slice.tune_cents,
        mute_group: slice.mute_group,
        start_frame: slice.start_frame,
        end_frame: slice.end_frame,
        window_length_frames: slice.window_length_frames(),
    }
}

fn sample_flip_name_root(source: &SampleFlipSource) -> String {
    let title = source.source_title.trim();
    if !title.is_empty() {
        return truncate_chars(title, 18);
    }
    let stem = source_path_stem(&source.source_path);
    if !stem.is_empty() {
        return truncate_chars(&stem, 18);
    }
    "Sample Flip".to_string()
}

fn sample_flip_source_token(source: &SampleFlipSource) -> String {
    let stem = source_path_stem(&source.source_path);
    sanitized_identifier(&source.source_id)
        .or_else(|| sanitized_identifier(&source.source_title))
        .or_else(|| sanitized_identifier(&stem))
        .unwrap_or_else(|| "source".to_string())
}

fn source_path_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty())
        .unwrap_or("")
        .to_string()
}

fn sanitized_identifier(input: &str) -> Option<String> {
    let mut output = String::new();
    let mut last_was_separator = false;
    for character in input.chars() {
        let character = character.to_ascii_lowercase();
        if character.is_ascii_alphanumeric() {
            output.push(character);
            last_was_separator = false;
        } else if !output.is_empty() && !last_was_separator {
            output.push('_');
            last_was_separator = true;
        }
        if output.len() >= 48 {
            break;
        }
    }
    while output.ends_with('_') {
        output.pop();
    }
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let truncated = value.trim().chars().take(max_chars).collect::<String>();
    if truncated.is_empty() {
        "Sample Flip".to_string()
    } else {
        truncated
    }
}

fn program_pad_label(pad: ProgramPad) -> String {
    format!("{}{:02}", pad.bank.label(), pad.pad_number)
}

fn is_zero_u8(value: &u8) -> bool {
    *value == 0
}
