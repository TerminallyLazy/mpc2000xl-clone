# MPC2000XL Runtime I/O And Pad Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add explicit host audio device selection, rights-safe WAV relinking after project reload, outbound MIDI note-off scheduling, and layered modern-MPC-style pad lighting.

**Architecture:** Keep `mpc_core` deterministic and portable, with only project media-reference metadata added there. Put host device discovery and WAV decoding in `mpc_audio`, outbound MIDI message/scheduler behavior in `mpc_midi`, and runtime orchestration plus pad visual state in `apps/desktop`.

**Tech Stack:** Rust 2024 workspace, serde/serde_json, hound, CPAL 0.18.1, midir 0.11.0, eframe/egui 0.34.3.

---

## Scope Check

The approved spec spans four runtime areas, but they are coupled by the same live instrument flow: pad or sequence playback produces audio, MIDI, media status, and pad feedback. Keep this as one plan with independently committable tasks. Do not broaden into resampling, native MPC disk formats, persistent host preferences, exact MPC LED behavior, or exact MIDI timing evidence.

## File Structure

- Modify `crates/mpc_core/src/events.rs`: add backward-compatible MIDI output kind and note-window metadata.
- Modify `crates/mpc_core/src/state.rs`: add project imported-media references, validation, export/restore, and output intent metadata.
- Modify `crates/mpc_core/src/lib.rs`: re-export new core metadata types.
- Modify `crates/mpc_core/tests/core_flow.rs`: add project media-reference and MIDI output metadata tests.
- Modify `crates/mpc_storage/src/lib.rs`: add storage-level JSON metadata-only coverage for imported media references.
- Modify `crates/mpc_audio/src/lib.rs`: add host output device descriptors and open-by-id path.
- Modify `crates/mpc_midi/src/lib.rs`: add note-off encoding, MIDI message kind, and pending outbound note scheduler.
- Modify `apps/desktop/src/main.rs`: add audio picker state/UI, imported WAV relink state, MIDI scheduler advancement, and pad light projection.
- Modify `README.md`: document runtime behavior and rights-safe media references.

## Task 1: Add Core Project Media References

**Files:**
- Modify: `crates/mpc_core/src/state.rs`
- Modify: `crates/mpc_core/src/lib.rs`
- Modify: `crates/mpc_core/tests/core_flow.rs`

- [ ] **Step 1: Write failing core tests for imported media references**

Add `ProjectImportedMediaReference` to the existing `mpc_core` import list in `crates/mpc_core/tests/core_flow.rs`.

Append these tests near the existing project snapshot tests:

```rust
#[test]
fn imported_media_reference_round_trips_without_audio_bytes() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Sample,
    });
    let outputs = core.import_sample_metadata_for_selected_pad("KICK".to_string(), 44_100);
    let sample_id = outputs
        .iter()
        .find_map(|output| match output {
            MachineOutput::SampleMetadataCreated { sample, .. } => Some(sample.id.clone()),
            _ => None,
        })
        .expect("import should create sample metadata");

    let reference = ProjectImportedMediaReference {
        sample_id: sample_id.clone(),
        source_path: "local-assets/samples/kick.wav".to_string(),
        managed_copy_path: Some("local-assets/projects/media/kick.wav".to_string()),
        sample_name: "KICK".to_string(),
        sample_rate_hz: 44_100,
        frame_count: 44_100,
        byte_count: 88_244,
        source_kind: SampleSourceKind::Imported,
    };
    core.upsert_imported_media_reference(reference.clone())
        .expect("imported sample reference should attach");

    let json = core.to_project_json().expect("project should encode");
    assert!(json.contains("\"imported_media_references\""));
    assert!(json.contains("local-assets/samples/kick.wav"));
    assert!(!json.contains("audio_bytes"));
    assert!(!json.contains("sample_file_contents"));

    let snapshot = MpcCore::from_project_json(&json).expect("project should decode");
    assert_eq!(snapshot.program.imported_media_references, vec![reference]);
}

#[test]
fn imported_media_reference_rejects_unknown_or_generated_samples() {
    let mut core = MpcCore::new();
    let unknown = ProjectImportedMediaReference {
        sample_id: "missing".to_string(),
        source_path: "local-assets/samples/missing.wav".to_string(),
        managed_copy_path: None,
        sample_name: "MISSING".to_string(),
        sample_rate_hz: 44_100,
        frame_count: 1,
        byte_count: 44,
        source_kind: SampleSourceKind::Imported,
    };
    assert!(core.upsert_imported_media_reference(unknown).is_err());

    let generated = ProjectImportedMediaReference {
        sample_id: "synthetic_a_01".to_string(),
        source_path: "local-assets/samples/a01.wav".to_string(),
        managed_copy_path: None,
        sample_name: "A01".to_string(),
        sample_rate_hz: 44_100,
        frame_count: 1,
        byte_count: 44,
        source_kind: SampleSourceKind::Generated,
    };
    assert!(core.upsert_imported_media_reference(generated).is_err());
}
```

- [ ] **Step 2: Run failing core tests**

Run:

```bash
cargo test -p mpc_core --test core_flow imported_media_reference -- --test-threads=1
```

Expected: FAIL because `ProjectImportedMediaReference` and `upsert_imported_media_reference` do not exist.

- [ ] **Step 3: Add imported media metadata types and state fields**

In `crates/mpc_core/src/state.rs`, add this struct after `ProjectProgramSnapshot`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectImportedMediaReference {
    pub sample_id: String,
    pub source_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_copy_path: Option<String>,
    pub sample_name: String,
    pub sample_rate_hz: u32,
    pub frame_count: u32,
    pub byte_count: usize,
    pub source_kind: SampleSourceKind,
}
```

Add this field to `ProjectProgramSnapshot`:

```rust
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imported_media_references: Vec<ProjectImportedMediaReference>,
```

Add this field to `MpcState` immediately after `sample_trims`:

```rust
    #[serde(default)]
    pub imported_media_references: Vec<ProjectImportedMediaReference>,
```

Initialize it in `Default for MpcState`:

```rust
            imported_media_references: Vec::new(),
```

- [ ] **Step 4: Export, restore, prune, and mutate media references**

In `export_project_snapshot`, include:

```rust
                imported_media_references: normalized_imported_media_references(
                    &self.state.current_program,
                    &self.state.imported_media_references,
                ),
```

In `restore_project_snapshot`, after assigning `self.state.sample_trims`, add:

```rust
        self.state.imported_media_references = normalized_imported_media_references(
            &self.state.current_program,
            &snapshot.program.imported_media_references,
        );
```

Add this public method inside `impl MpcCore`:

```rust
    pub fn upsert_imported_media_reference(
        &mut self,
        reference: ProjectImportedMediaReference,
    ) -> Result<(), ProjectSnapshotError> {
        validate_imported_media_reference(
            "program.imported_media_references[]",
            &reference,
            &self.state.current_program.pad_assignments,
        )?;
        self.state
            .imported_media_references
            .retain(|existing| existing.sample_id != reference.sample_id);
        self.state.imported_media_references.push(reference);
        self.state
            .imported_media_references
            .sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
        Ok(())
    }
```

In `prune_sample_trims_for_current_program`, add pruning for media references:

```rust
        self.state.imported_media_references = normalized_imported_media_references(
            &self.state.current_program,
            &self.state.imported_media_references,
        );
```

Add this helper near `normalized_sample_trims`:

```rust
fn normalized_imported_media_references(
    program: &Program,
    references: &[ProjectImportedMediaReference],
) -> Vec<ProjectImportedMediaReference> {
    let imported_sample_ids = program
        .pad_assignments
        .iter()
        .filter(|assignment| assignment.sample.source_kind == SampleSourceKind::Imported)
        .map(|assignment| assignment.sample.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut normalized = references
        .iter()
        .filter(|reference| imported_sample_ids.contains(reference.sample_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.sample_id.cmp(&right.sample_id));
    normalized.dedup_by(|left, right| left.sample_id == right.sample_id);
    normalized
}
```

- [ ] **Step 5: Add JSON and semantic validation**

In `validate_program_json_fields`, include `"imported_media_references"` in the allowed field list and validate each entry:

```rust
    if let Some(imported_media_references) = program
        .get("imported_media_references")
        .and_then(Value::as_array)
    {
        for (index, reference) in imported_media_references.iter().enumerate() {
            validate_imported_media_reference_json_fields(
                &format!("program.imported_media_references[{index}]"),
                reference,
            )?;
        }
    }
```

Add this JSON validator near `validate_sample_trim_json_fields`:

```rust
fn validate_imported_media_reference_json_fields(
    field: &str,
    value: &Value,
) -> Result<(), ProjectSnapshotError> {
    reject_unknown_json_fields(
        field,
        value,
        &[
            "sample_id",
            "source_path",
            "managed_copy_path",
            "sample_name",
            "sample_rate_hz",
            "frame_count",
            "byte_count",
            "source_kind",
        ],
    )?;
    Ok(())
}
```

In `validate_project_snapshot`, after the call to `validate_sample_trims(&snapshot.program.sample_trims, &default_catalog)?;`, add:

```rust
    validate_imported_media_references(
        &snapshot.program.imported_media_references,
        &snapshot.program.pad_assignments,
    )?;
```

Add these helpers near `validate_sample_trims`:

```rust
fn validate_imported_media_references(
    references: &[ProjectImportedMediaReference],
    assignments: &[PadAssignment],
) -> Result<(), ProjectSnapshotError> {
    let mut seen = BTreeSet::new();
    for (index, reference) in references.iter().enumerate() {
        let field = format!("program.imported_media_references[{index}]");
        if !seen.insert(reference.sample_id.as_str()) {
            return Err(invalid_value(
                &format!("{field}.sample_id"),
                "duplicate imported media reference",
            ));
        }
        validate_imported_media_reference(&field, reference, assignments)?;
    }
    Ok(())
}

fn validate_imported_media_reference(
    field: &str,
    reference: &ProjectImportedMediaReference,
    assignments: &[PadAssignment],
) -> Result<(), ProjectSnapshotError> {
    validate_non_empty(&format!("{field}.sample_id"), &reference.sample_id)?;
    validate_non_empty(&format!("{field}.source_path"), &reference.source_path)?;
    if let Some(path) = &reference.managed_copy_path {
        validate_non_empty(&format!("{field}.managed_copy_path"), path)?;
    }
    validate_non_empty(&format!("{field}.sample_name"), &reference.sample_name)?;
    validate_range_u32(
        &format!("{field}.sample_rate_hz"),
        reference.sample_rate_hz,
        8_000,
        192_000,
    )?;
    validate_range_u32(&format!("{field}.frame_count"), reference.frame_count, 1, MAX_USER_SAMPLE_LENGTH_FRAMES)?;
    if reference.byte_count == 0 {
        return Err(invalid_value(&format!("{field}.byte_count"), "must be > 0"));
    }
    if reference.source_kind != SampleSourceKind::Imported {
        return Err(invalid_value(
            &format!("{field}.source_kind"),
            "must be imported",
        ));
    }
    let Some(assignment) = assignments
        .iter()
        .find(|assignment| assignment.sample.id == reference.sample_id)
    else {
        return Err(invalid_value(
            &format!("{field}.sample_id"),
            format!("unknown sample id {:?}", reference.sample_id),
        ));
    };
    if assignment.sample.source_kind != SampleSourceKind::Imported {
        return Err(invalid_value(
            &format!("{field}.sample_id"),
            "referenced sample must be imported",
        ));
    }
    Ok(())
}
```

- [ ] **Step 6: Re-export the new type**

In `crates/mpc_core/src/lib.rs`, add `ProjectImportedMediaReference` to the existing public exports from `state`.

- [ ] **Step 7: Run core tests**

Run:

```bash
cargo test -p mpc_core --test core_flow imported_media_reference -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 8: Commit core media references**

```bash
git add crates/mpc_core/src/state.rs crates/mpc_core/src/lib.rs crates/mpc_core/tests/core_flow.rs
git commit -m "feat: persist imported media references"
```

## Task 2: Add Storage Coverage For Metadata-Only Media References

**Files:**
- Modify: `crates/mpc_storage/src/lib.rs`
- Test: `crates/mpc_storage/src/lib.rs`

- [ ] **Step 1: Write failing storage test**

Add `ProjectImportedMediaReference` to the existing `mpc_core` import list inside the storage tests module.

Append this test after `saved_json_is_metadata_only`:

```rust
#[test]
fn saved_json_keeps_imported_media_references_metadata_only() {
    let root = temp_root("imported_media_metadata");
    let path = root.join("metadata.mpc2000xl-project.json");
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Sample,
    });
    let outputs = core.import_sample_metadata_for_selected_pad("KICK".to_string(), 44_100);
    let sample_id = outputs
        .iter()
        .find_map(|output| match output {
            MachineOutput::SampleMetadataCreated { sample, .. } => Some(sample.id.clone()),
            _ => None,
        })
        .expect("import should create sample metadata");

    core.upsert_imported_media_reference(ProjectImportedMediaReference {
        sample_id,
        source_path: "local-assets/samples/kick.wav".to_string(),
        managed_copy_path: None,
        sample_name: "KICK".to_string(),
        sample_rate_hz: 44_100,
        frame_count: 44_100,
        byte_count: 88_244,
        source_kind: mpc_core::SampleSourceKind::Imported,
    })
    .expect("media reference should attach");

    save_project_file(&core, &path).expect("project file should save");
    let json = fs::read_to_string(&path).expect("saved file should be readable");

    assert!(json.contains("\"imported_media_references\""));
    assert!(json.contains("local-assets/samples/kick.wav"));
    assert!(!json.contains("\"audio_bytes\""));
    assert!(!json.contains("\"sample_file_contents\""));

    remove_temp_root(root);
}
```

- [ ] **Step 2: Run failing storage test**

Run:

```bash
cargo test -p mpc_storage saved_json_keeps_imported_media_references_metadata_only -- --test-threads=1
```

Expected: PASS if Task 1 is complete. If it fails, fix only import paths or type visibility.

- [ ] **Step 3: Run all storage tests**

Run:

```bash
cargo test -p mpc_storage -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 4: Commit storage coverage**

```bash
git add crates/mpc_storage/src/lib.rs
git commit -m "test: cover imported media project storage"
```

## Task 3: Add Explicit Host Audio Output Device API

**Files:**
- Modify: `crates/mpc_audio/src/lib.rs`

- [ ] **Step 1: Write failing audio descriptor tests**

Add these tests inside `#[cfg(test)] mod tests` in `crates/mpc_audio/src/lib.rs`:

```rust
#[test]
fn audio_output_descriptor_label_marks_default_and_format() {
    let descriptor = AudioOutputDeviceDescriptor {
        index: 2,
        id: "built-in-output".to_string(),
        name: "Built-in Output".to_string(),
        is_default: true,
        sample_rate_hz: Some(48_000),
        channels: Some(2),
        sample_format: Some("F32".to_string()),
    };

    assert_eq!(
        descriptor.display_label(),
        "Built-in Output (default) - 48000 Hz, 2 ch, F32"
    );
}

#[test]
fn audio_output_descriptor_label_handles_unknown_config() {
    let descriptor = AudioOutputDeviceDescriptor {
        index: 0,
        id: "external".to_string(),
        name: "External Interface".to_string(),
        is_default: false,
        sample_rate_hz: None,
        channels: None,
        sample_format: None,
    };

    assert_eq!(descriptor.display_label(), "External Interface");
}
```

- [ ] **Step 2: Run failing descriptor tests**

Run:

```bash
cargo test -p mpc_audio audio_output_descriptor_label -- --test-threads=1
```

Expected: FAIL because `AudioOutputDeviceDescriptor` does not exist.

- [ ] **Step 3: Add descriptor type and listing/opening functions**

In `crates/mpc_audio/src/lib.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioOutputDeviceDescriptor {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
    pub sample_format: Option<String>,
}

impl AudioOutputDeviceDescriptor {
    pub fn display_label(&self) -> String {
        let mut label = self.name.clone();
        if self.is_default {
            label.push_str(" (default)");
        }
        if let (Some(sample_rate_hz), Some(channels), Some(sample_format)) = (
            self.sample_rate_hz,
            self.channels,
            self.sample_format.as_ref(),
        ) {
            label.push_str(&format!(
                " - {sample_rate_hz} Hz, {channels} ch, {sample_format}"
            ));
        }
        label
    }
}
```

Add these helpers before `impl DeviceAudioBackend`:

```rust
pub fn list_output_devices() -> Result<Vec<AudioOutputDeviceDescriptor>, HostAudioBackendError> {
    let host = cpal::default_host();
    let default_id = host
        .default_output_device()
        .and_then(|device| device_audio_device_id(&device).ok());
    let devices = host
        .output_devices()
        .map_err(|error| device_audio_backend_error(format!("output device list failed: {error}")))?;

    devices
        .enumerate()
        .map(|(index, device)| audio_output_device_descriptor(index, &device, default_id.as_deref()))
        .collect()
}

fn audio_output_device_descriptor(
    index: usize,
    device: &cpal::Device,
    default_id: Option<&str>,
) -> Result<AudioOutputDeviceDescriptor, HostAudioBackendError> {
    let id = device_audio_device_id(device)?;
    let name = device.name().unwrap_or_else(|error| {
        format!("unknown output device ({error})")
    });
    let config = device.default_output_config().ok();
    Ok(AudioOutputDeviceDescriptor {
        index,
        is_default: default_id == Some(id.as_str()),
        id,
        name,
        sample_rate_hz: config.as_ref().map(|config| config.config().sample_rate.0),
        channels: config.as_ref().map(|config| config.config().channels),
        sample_format: config.as_ref().map(|config| format!("{:?}", config.sample_format())),
    })
}

fn device_audio_device_id(device: &cpal::Device) -> Result<String, HostAudioBackendError> {
    device
        .id()
        .map(|id| id.to_string())
        .map_err(|error| device_audio_backend_error(format!("output device id failed: {error}")))
}
```

Update `DeviceAudioBackend::open_default` to delegate:

```rust
    pub fn open_default(config: DeviceAudioBackendConfig) -> Result<Self, HostAudioBackendError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| device_audio_backend_error("default output device is not available"))?;
        Self::open_device(device, config)
    }
```

Add explicit open-by-id and shared open helper inside `impl DeviceAudioBackend`:

```rust
    pub fn open_output_device_id(
        device_id: &str,
        config: DeviceAudioBackendConfig,
    ) -> Result<Self, HostAudioBackendError> {
        let host = cpal::default_host();
        let devices = host.output_devices().map_err(|error| {
            device_audio_backend_error(format!("output device list failed: {error}"))
        })?;
        for device in devices {
            if device_audio_device_id(&device).ok().as_deref() == Some(device_id) {
                return Self::open_device(device, config);
            }
        }
        Err(device_audio_backend_error(format!(
            "output device id {device_id:?} is not available"
        )))
    }

    fn open_device(
        device: cpal::Device,
        config: DeviceAudioBackendConfig,
    ) -> Result<Self, HostAudioBackendError> {
        let device_name = device_audio_device_id(&device)?;
        let supported_config = device.default_output_config().map_err(|error| {
            device_audio_backend_error(format!("default output config failed: {error}"))
        })?;
        let sample_format = supported_config.sample_format();
        let stream_config = supported_config.config();
        let sample_rate_hz = stream_config.sample_rate.0;
        if !(MIN_SAMPLE_RATE_HZ..=MAX_SAMPLE_RATE_HZ).contains(&sample_rate_hz) {
            return Err(device_audio_backend_error(format!(
                "default output sample rate {sample_rate_hz} Hz is outside renderer bounds {MIN_SAMPLE_RATE_HZ}..={MAX_SAMPLE_RATE_HZ} Hz"
            )));
        }
        let channels = stream_config.channels;
        let sample_format_text = format!("{sample_format:?}");
        let shared = Arc::new(Mutex::new(DeviceAudioOutputQueue::new(
            config.max_queued_frames,
        )));
        let stream =
            build_device_output_stream(&device, stream_config, sample_format, Arc::clone(&shared))?;
        stream
            .play()
            .map_err(|error| device_audio_backend_error(format!("stream play failed: {error}")))?;

        Ok(Self {
            backend_name: DEVICE_AUDIO_BACKEND_NAME.to_string(),
            device_name,
            sample_rate_hz,
            channels,
            sample_format: sample_format_text,
            shared,
            _stream: stream,
        })
    }
```

- [ ] **Step 4: Run audio tests**

Run:

```bash
cargo test -p mpc_audio audio_output_descriptor_label -- --test-threads=1
cargo test -p mpc_audio device_audio -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 5: Commit audio device API**

```bash
git add crates/mpc_audio/src/lib.rs
git commit -m "feat: add audio output device descriptors"
```

## Task 4: Wire Desktop Audio Device Picker And WAV Relink

**Files:**
- Modify: `apps/desktop/src/main.rs`

- [ ] **Step 1: Update desktop imports and struct fields**

Update the `mpc_audio` import list:

```rust
use mpc_audio::{
    list_output_devices, AudioOutputDeviceDescriptor, AudioRenderKind, AudioRenderSettings,
    AudioRenderSummary, CaptureAudioBackend, DeviceAudioBackend, DeviceAudioBackendConfig,
    DeviceAudioBackendStatus, HostAudioBackend, HostAudioBackendError, HostAudioEngine,
    HostAudioError, HostAudioEvent, HostAudioPlaybackReport, HostAudioState,
    RuntimeSampleLibrary, WavSamplePayload, load_wav_sample_payload,
};
```

Update the `std::collections` import:

```rust
use std::collections::{BTreeMap, BTreeSet};
```

Add these fields to `MpcDesktopApp`:

```rust
    audio_output_devices: Vec<AudioOutputDeviceDescriptor>,
    selected_audio_output_device: usize,
    runtime_sample_statuses: BTreeMap<String, RuntimeSampleStatus>,
```

Add this enum near `DesktopMidiBackend`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeSampleStatus {
    Loaded {
        path: String,
        frame_count: usize,
        sample_rate_hz: u32,
        byte_count: usize,
    },
    Missing {
        attempted_paths: Vec<String>,
    },
    MetadataMismatch {
        path: String,
        expected_sample_rate_hz: u32,
        actual_sample_rate_hz: u32,
        expected_frame_count: u32,
        actual_frame_count: usize,
    },
    LoadFailed {
        path: String,
        message: String,
    },
}
```

Initialize new fields in `Default for MpcDesktopApp`:

```rust
            audio_output_devices: Vec::new(),
            selected_audio_output_device: 0,
            runtime_sample_statuses: BTreeMap::new(),
```

- [ ] **Step 2: Add relink helpers**

Add these methods inside `impl MpcDesktopApp`:

```rust
    fn relink_runtime_samples_from_project(&mut self, reason: &str) {
        self.runtime_samples.clear();
        self.runtime_sample_statuses.clear();
        let references = self.core.state().imported_media_references.clone();
        let mut loaded_count = 0_usize;
        let mut missing_count = 0_usize;

        for reference in references {
            match self.load_runtime_sample_reference(&reference) {
                Ok(()) => loaded_count = loaded_count.saturating_add(1),
                Err(()) => missing_count = missing_count.saturating_add(1),
            }
        }

        self.last_audio_render = None;
        self.last_audio_render_error = None;
        self.last_runtime_sample_status = format!(
            "Runtime WAV: relink after {reason}: {loaded_count} loaded, {missing_count} missing"
        );
    }

    fn load_runtime_sample_reference(
        &mut self,
        reference: &mpc_core::ProjectImportedMediaReference,
    ) -> Result<(), ()> {
        let mut attempted_paths = Vec::new();
        for path in media_reference_candidate_paths(reference) {
            attempted_paths.push(path.clone());
            match load_wav_sample_payload(&path) {
                Ok(payload) if runtime_payload_matches_reference(&payload, reference) => {
                    self.runtime_samples.insert(
                        reference.sample_id.clone(),
                        reference.sample_name.clone(),
                        payload.clone(),
                    );
                    self.runtime_sample_statuses.insert(
                        reference.sample_id.clone(),
                        RuntimeSampleStatus::Loaded {
                            path,
                            frame_count: payload.frame_count,
                            sample_rate_hz: payload.sample_rate_hz,
                            byte_count: payload.byte_count,
                        },
                    );
                    return Ok(());
                }
                Ok(payload) => {
                    self.runtime_sample_statuses.insert(
                        reference.sample_id.clone(),
                        RuntimeSampleStatus::MetadataMismatch {
                            path,
                            expected_sample_rate_hz: reference.sample_rate_hz,
                            actual_sample_rate_hz: payload.sample_rate_hz,
                            expected_frame_count: reference.frame_count,
                            actual_frame_count: payload.frame_count,
                        },
                    );
                    return Err(());
                }
                Err(error) => {
                    self.runtime_sample_statuses.insert(
                        reference.sample_id.clone(),
                        RuntimeSampleStatus::LoadFailed {
                            path,
                            message: error.to_string(),
                        },
                    );
                }
            }
        }

        self.runtime_sample_statuses.insert(
            reference.sample_id.clone(),
            RuntimeSampleStatus::Missing { attempted_paths },
        );
        Err(())
    }
```

Add these free functions near `runtime_sample_ids_referenced_by_project`:

```rust
fn media_reference_candidate_paths(
    reference: &mpc_core::ProjectImportedMediaReference,
) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(path) = &reference.managed_copy_path {
        paths.push(path.clone());
    }
    if !reference.source_path.is_empty()
        && !paths.iter().any(|path| path == &reference.source_path)
    {
        paths.push(reference.source_path.clone());
    }
    paths
}

fn runtime_payload_matches_reference(
    payload: &WavSamplePayload,
    reference: &mpc_core::ProjectImportedMediaReference,
) -> bool {
    payload.sample_rate_hz == reference.sample_rate_hz
        && payload.frame_count == reference.frame_count as usize
}
```

- [ ] **Step 3: Save media references when importing WAVs**

In `load_wav_to_selected_pad`, after inserting into `self.runtime_samples`, add:

```rust
            let reference = mpc_core::ProjectImportedMediaReference {
                sample_id: sample.id.clone(),
                source_path: path.to_string(),
                managed_copy_path: None,
                sample_name: sample.name.clone(),
                sample_rate_hz,
                frame_count: length_frames,
                byte_count,
                source_kind: mpc_core::SampleSourceKind::Imported,
            };
            if let Err(error) = self.core.upsert_imported_media_reference(reference) {
                let message = format!("Runtime WAV import metadata failed: {error}");
                self.last_runtime_sample_status = message.clone();
                self.last_status = message;
                return;
            }
            self.runtime_sample_statuses.insert(
                sample.id.clone(),
                RuntimeSampleStatus::Loaded {
                    path: path.to_string(),
                    frame_count: length_frames as usize,
                    sample_rate_hz,
                    byte_count,
                },
            );
```

Replace both calls to `self.clear_runtime_sample_payloads("snapshot load")` and `self.clear_runtime_sample_payloads("project file load")` with:

```rust
                        self.relink_runtime_samples_from_project("snapshot load");
```

and:

```rust
                        self.relink_runtime_samples_from_project("project file load");
```

Update `clear_runtime_sample_payloads` to also clear statuses:

```rust
        self.runtime_sample_statuses.clear();
```

- [ ] **Step 4: Add audio device picker helpers**

Add these methods inside `impl MpcDesktopApp`:

```rust
    fn refresh_audio_output_devices(&mut self) {
        match list_output_devices() {
            Ok(devices) => {
                self.audio_output_devices = devices;
                self.selected_audio_output_device = clamp_port_index(
                    self.selected_audio_output_device,
                    self.audio_output_devices.len(),
                );
                self.last_status = format!(
                    "Audio devices: {} output device(s)",
                    self.audio_output_devices.len()
                );
            }
            Err(error) => {
                self.audio_output_devices.clear();
                self.selected_audio_output_device = 0;
                self.last_status = format!("Audio device refresh failed: {error}");
            }
        }
    }

    fn switch_host_audio_to_selected_device(&mut self) {
        let Some(device) = self
            .audio_output_devices
            .get(self.selected_audio_output_device)
            .cloned()
        else {
            self.last_status = "No audio output device selected".to_string();
            return;
        };

        match DeviceAudioBackend::open_output_device_id(
            &device.id,
            DeviceAudioBackendConfig::default(),
        ) {
            Ok(backend) => {
                let status = backend.status();
                let current_render_settings = self.host_audio.render_settings();
                let device_render_settings = match AudioRenderSettings::new(
                    status.sample_rate_hz,
                    current_render_settings.frame_count,
                ) {
                    Ok(settings) => settings,
                    Err(error) => {
                        self.last_status =
                            format!("Host audio device render settings unsupported: {error}");
                        return;
                    }
                };
                self.replace_host_audio_backend(
                    DesktopAudioBackend::Device(backend),
                    device_render_settings,
                    format!(
                        "Host audio backend: device {} {} Hz {} ch {}",
                        status.device_name,
                        status.sample_rate_hz,
                        status.channels,
                        status.sample_format
                    ),
                );
            }
            Err(error) => {
                self.last_status = format!("Host audio device unavailable: {error}");
            }
        }
    }
```

Add this free function near `midi_port_combo`:

```rust
fn audio_output_device_combo(
    ui: &mut egui::Ui,
    devices: &[AudioOutputDeviceDescriptor],
    selected_index: &mut usize,
) {
    *selected_index = clamp_port_index(*selected_index, devices.len());
    egui::ComboBox::from_label("Audio out")
        .selected_text(selected_audio_output_device_text(devices, *selected_index))
        .show_ui(ui, |ui| {
            for (index, device) in devices.iter().enumerate() {
                ui.selectable_value(selected_index, index, device.display_label());
            }
        });
}

fn selected_audio_output_device_text(
    devices: &[AudioOutputDeviceDescriptor],
    selected_index: usize,
) -> String {
    devices
        .get(selected_index)
        .map(AudioOutputDeviceDescriptor::display_label)
        .unwrap_or_else(|| "none".to_string())
}
```

- [ ] **Step 5: Wire audio picker UI**

In `draw_host_audio_status`, after the default device button block, add:

```rust
            if ui.button("Refresh audio").clicked() {
                self.refresh_audio_output_devices();
            }
            audio_output_device_combo(
                ui,
                &self.audio_output_devices,
                &mut self.selected_audio_output_device,
            );
            ui.add_enabled_ui(!self.audio_output_devices.is_empty(), |ui| {
                if ui.button("Open selected").clicked() {
                    self.switch_host_audio_to_selected_device();
                }
            });
```

- [ ] **Step 6: Run desktop compile**

Run:

```bash
cargo check -p mpc_desktop
```

Expected: PASS.

- [ ] **Step 7: Commit desktop audio/relink**

```bash
git add apps/desktop/src/main.rs
git commit -m "feat: wire audio picker and wav relink"
```

## Task 5: Add MIDI Note-Off Encoding And Pending Scheduler

**Files:**
- Modify: `crates/mpc_core/src/events.rs`
- Modify: `crates/mpc_core/src/lib.rs`
- Modify: `crates/mpc_core/src/state.rs`
- Modify: `crates/mpc_midi/src/lib.rs`
- Modify: `crates/mpc_core/tests/core_flow.rs`

- [ ] **Step 1: Write failing MIDI tests**

In `crates/mpc_midi/src/lib.rs`, add these tests near `note_on_message_encodes_to_midi_bytes`:

```rust
#[test]
fn note_off_message_encodes_to_midi_bytes() {
    let bytes = encode_midi_message(&MidiMessage {
        kind: MidiMessageKind::NoteOff,
        channel: 2,
        note: 55,
        velocity: 0,
    })
    .expect("valid note-off should encode");

    assert_eq!(bytes, [0x81, 55, 0]);
}

#[test]
fn outbound_note_scheduler_expires_note_on_into_note_off_intent() {
    let mut scheduler = OutboundMidiNoteScheduler::default();
    let note_on = intent();

    scheduler.register_note_on(&note_on, 100, 250);
    assert_eq!(scheduler.pending_count(), 1);
    assert!(scheduler.drain_due_note_offs(349).is_empty());

    let due = scheduler.drain_due_note_offs(350);
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].kind, mpc_core::MidiOutputIntentKind::NoteOff);
    assert_eq!(due[0].channel, note_on.channel);
    assert_eq!(due[0].note, note_on.note);
    assert_eq!(due[0].velocity, 0);
    assert_eq!(scheduler.pending_count(), 0);
}

#[test]
fn outbound_note_scheduler_releases_matching_note_early() {
    let mut scheduler = OutboundMidiNoteScheduler::default();
    let note_on = intent();

    scheduler.register_note_on(&note_on, 100, 250);
    let released = scheduler.release_matching_note(&note_on, 150);

    assert_eq!(released.len(), 1);
    assert_eq!(released[0].kind, mpc_core::MidiOutputIntentKind::NoteOff);
    assert_eq!(released[0].note, note_on.note);
    assert_eq!(scheduler.pending_count(), 0);
    assert!(scheduler.drain_due_note_offs(400).is_empty());
}
```

In `crates/mpc_core/tests/core_flow.rs`, add:

```rust
#[test]
fn midi_output_intent_includes_note_on_kind_and_window_length() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 100,
    });

    let intent = outputs
        .iter()
        .find_map(|output| match output {
            MachineOutput::MidiOutputIntent { intent } => Some(intent),
            _ => None,
        })
        .expect("pad playback should emit midi output");

    assert_eq!(intent.kind, mpc_core::MidiOutputIntentKind::NoteOn);
    assert_eq!(intent.window_length_frames, 48_000);
}
```

- [ ] **Step 2: Run failing MIDI tests**

Run:

```bash
cargo test -p mpc_midi note_off_message -- --test-threads=1
cargo test -p mpc_midi outbound_note_scheduler -- --test-threads=1
cargo test -p mpc_core --test core_flow midi_output_intent_includes_note_on_kind_and_window_length -- --test-threads=1
```

Expected: FAIL because note kinds and scheduler do not exist.

- [ ] **Step 3: Add MIDI output kind and window metadata to core**

In `crates/mpc_core/src/events.rs`, add before `MidiOutputIntent`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiOutputIntentKind {
    #[default]
    NoteOn,
    NoteOff,
}

impl MidiOutputIntentKind {
    pub fn is_note_on(&self) -> bool {
        *self == Self::NoteOn
    }
}
```

Update `MidiOutputIntent`:

```rust
pub struct MidiOutputIntent {
    #[serde(default, skip_serializing_if = "MidiOutputIntentKind::is_note_on")]
    pub kind: MidiOutputIntentKind,
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
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub window_length_frames: u32,
}
```

Add helper:

```rust
fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}
```

In `midi_output_intent_for_playback` in `crates/mpc_core/src/state.rs`, include:

```rust
            kind: MidiOutputIntentKind::NoteOn,
            window_length_frames: intent.window_length_frames,
```

Add `MidiOutputIntentKind` to the existing `crate::events` import list at the top of `crates/mpc_core/src/state.rs`, then use `MidiOutputIntentKind::NoteOn` in the `midi_output_intent_for_playback` struct literal.

No project snapshot JSON validator is required for `MidiOutputIntent`, because it is a runtime `MachineOutput` type rather than project-persisted state.

In `crates/mpc_core/src/lib.rs`, add `MidiOutputIntentKind` to the existing `pub use events` list so downstream crates can use `mpc_core::MidiOutputIntentKind`.

- [ ] **Step 4: Add MIDI message kind and encoding**

In `crates/mpc_midi/src/lib.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiMessageKind {
    #[default]
    NoteOn,
    NoteOff,
}
```

Update `MidiMessage`:

```rust
pub struct MidiMessage {
    #[serde(default)]
    pub kind: MidiMessageKind,
    pub channel: u8,
    pub note: u8,
    pub velocity: u8,
}
```

Replace `encode_note_on_message` with:

```rust
pub fn encode_midi_message(message: &MidiMessage) -> Result<[u8; 3], HostMidiError> {
    validate_range(
        "channel",
        message.channel,
        MIDI_MIN_CHANNEL,
        MIDI_MAX_CHANNEL,
        "must be in range 1..=16",
    )?;
    validate_range(
        "note",
        message.note,
        0,
        MIDI_MAX_NOTE,
        "must be in range 0..=127",
    )?;
    let min_velocity = match message.kind {
        MidiMessageKind::NoteOn => 1,
        MidiMessageKind::NoteOff => 0,
    };
    validate_range(
        "velocity",
        message.velocity,
        min_velocity,
        MIDI_MAX_VELOCITY,
        if message.kind == MidiMessageKind::NoteOn {
            "must be in range 1..=127"
        } else {
            "must be in range 0..=127"
        },
    )?;

    let status = match message.kind {
        MidiMessageKind::NoteOn => MIDI_NOTE_ON_STATUS,
        MidiMessageKind::NoteOff => MIDI_NOTE_OFF_STATUS,
    };
    Ok([status | (message.channel - 1), message.note, message.velocity])
}

pub fn encode_note_on_message(message: &MidiMessage) -> Result<[u8; 3], HostMidiError> {
    let mut message = message.clone();
    message.kind = MidiMessageKind::NoteOn;
    encode_midi_message(&message)
}
```

Update `DeviceMidiOutputBackend::send` to call `encode_midi_message(&message)`.

Update `message_from_intent`:

```rust
    let kind = match intent.kind {
        mpc_core::MidiOutputIntentKind::NoteOn => MidiMessageKind::NoteOn,
        mpc_core::MidiOutputIntentKind::NoteOff => MidiMessageKind::NoteOff,
    };
```

Use `kind` in the returned `MidiMessage`. Apply the same velocity min rule used by `encode_midi_message`.

- [ ] **Step 5: Add outbound note scheduler**

In `crates/mpc_midi/src/lib.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingOutboundMidiNote {
    pub intent: MidiOutputIntent,
    pub note_off_due_millis: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundMidiNoteScheduler {
    pending_notes: Vec<PendingOutboundMidiNote>,
}

impl OutboundMidiNoteScheduler {
    pub fn pending_count(&self) -> usize {
        self.pending_notes.len()
    }

    pub fn has_pending(&self) -> bool {
        !self.pending_notes.is_empty()
    }

    pub fn register_note_on(
        &mut self,
        intent: &MidiOutputIntent,
        now_millis: u64,
        duration_millis: u64,
    ) {
        if intent.kind != mpc_core::MidiOutputIntentKind::NoteOn {
            return;
        }
        let due = now_millis.saturating_add(duration_millis);
        self.pending_notes
            .retain(|pending| !same_outbound_note(&pending.intent, intent));
        self.pending_notes.push(PendingOutboundMidiNote {
            intent: intent.clone(),
            note_off_due_millis: due,
        });
    }

    pub fn drain_due_note_offs(&mut self, now_millis: u64) -> Vec<MidiOutputIntent> {
        let mut due = Vec::new();
        let mut pending = Vec::with_capacity(self.pending_notes.len());
        for note in self.pending_notes.drain(..) {
            if note.note_off_due_millis <= now_millis {
                due.push(note_off_intent(&note.intent));
            } else {
                pending.push(note);
            }
        }
        self.pending_notes = pending;
        due
    }

    pub fn release_matching_note(
        &mut self,
        intent: &MidiOutputIntent,
        _now_millis: u64,
    ) -> Vec<MidiOutputIntent> {
        let mut released = Vec::new();
        let mut pending = Vec::with_capacity(self.pending_notes.len());
        for note in self.pending_notes.drain(..) {
            if same_outbound_note(&note.intent, intent) {
                released.push(note_off_intent(&note.intent));
            } else {
                pending.push(note);
            }
        }
        self.pending_notes = pending;
        released
    }

    pub fn clear(&mut self) {
        self.pending_notes.clear();
    }
}

fn same_outbound_note(left: &MidiOutputIntent, right: &MidiOutputIntent) -> bool {
    left.channel == right.channel
        && left.note == right.note
        && left.bank == right.bank
        && left.pad_number == right.pad_number
        && left.source_sample_id == right.source_sample_id
        && left.selected_track == right.selected_track
}

fn note_off_intent(intent: &MidiOutputIntent) -> MidiOutputIntent {
    let mut note_off = intent.clone();
    note_off.kind = mpc_core::MidiOutputIntentKind::NoteOff;
    note_off.velocity = 0;
    note_off
}
```

- [ ] **Step 6: Update existing MIDI tests and constructors**

Update the `intent()` helper in `crates/mpc_midi/src/lib.rs` tests:

```rust
            kind: mpc_core::MidiOutputIntentKind::NoteOn,
            window_length_frames: 48_000,
```

Update all expected `MidiMessage` values in tests to include:

```rust
                kind: MidiMessageKind::NoteOn,
```

- [ ] **Step 7: Run MIDI and core tests**

Run:

```bash
cargo test -p mpc_midi -- --test-threads=1
cargo test -p mpc_core --test core_flow midi -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 8: Commit MIDI note-off foundation**

```bash
git add crates/mpc_core/src/events.rs crates/mpc_core/src/lib.rs crates/mpc_core/src/state.rs crates/mpc_core/tests/core_flow.rs crates/mpc_midi/src/lib.rs
git commit -m "feat: add outbound midi note off scheduler"
```

## Task 6: Integrate Desktop MIDI Note-Off Scheduling

**Files:**
- Modify: `apps/desktop/src/main.rs`

- [ ] **Step 1: Add scheduler imports and fields**

Update `mpc_midi` imports:

```rust
    DeviceMidiOutputBackend, DeviceMidiOutputStatus, HostMidiBackend, HostMidiEngine,
    HostMidiEvent, HostMidiOutputReport, HostMidiState, MidiInputEvent, MidiPortDescriptor,
    OutboundMidiNoteScheduler, list_device_midi_input_ports, list_device_midi_output_ports,
```

Add constants near the other desktop-level constants:

```rust
const DEFAULT_OUTBOUND_NOTE_DURATION_MILLIS: u64 = 250;
const MIN_OUTBOUND_NOTE_DURATION_MILLIS: u64 = 30;
const MAX_OUTBOUND_NOTE_DURATION_MILLIS: u64 = 4_000;
```

Add fields to `MpcDesktopApp`:

```rust
    outbound_midi_notes: OutboundMidiNoteScheduler,
    runtime_started_at: std::time::Instant,
    last_midi_note_off_status: String,
```

Initialize them:

```rust
            outbound_midi_notes: OutboundMidiNoteScheduler::default(),
            runtime_started_at: std::time::Instant::now(),
            last_midi_note_off_status: "MIDI note-off: none pending".to_string(),
```

- [ ] **Step 2: Add scheduler helper methods**

Add these methods inside `impl MpcDesktopApp`:

```rust
    fn runtime_millis(&self) -> u64 {
        self.runtime_started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn outbound_note_duration_millis(&self, intent: &mpc_core::MidiOutputIntent) -> u64 {
        if intent.window_length_frames == 0 {
            return DEFAULT_OUTBOUND_NOTE_DURATION_MILLIS;
        }
        let sample_rate_hz = u64::from(self.host_audio.render_settings().sample_rate_hz);
        let frames = u64::from(intent.window_length_frames);
        frames
            .saturating_mul(1_000)
            .checked_div(sample_rate_hz)
            .unwrap_or(DEFAULT_OUTBOUND_NOTE_DURATION_MILLIS)
            .clamp(MIN_OUTBOUND_NOTE_DURATION_MILLIS, MAX_OUTBOUND_NOTE_DURATION_MILLIS)
    }

    fn send_midi_intent(&mut self, intent: &mpc_core::MidiOutputIntent) -> Option<String> {
        let report = self.host_midi.send_intent(intent);
        let error = self.record_midi_report(report);
        if error.is_none() && intent.kind == mpc_core::MidiOutputIntentKind::NoteOff {
            self.last_midi_note_off_status = format!(
                "MIDI note-off sent ch {} note {} {:?}{:02}",
                intent.channel, intent.note, intent.bank, intent.pad_number
            );
        }
        error
    }

    fn flush_due_midi_note_offs(&mut self) -> Option<String> {
        let now = self.runtime_millis();
        let due = self.outbound_midi_notes.drain_due_note_offs(now);
        if due.is_empty() {
            return None;
        }

        let mut last_error = None;
        for intent in due {
            if let Some(error) = self.send_midi_intent(&intent) {
                last_error = Some(error);
            }
        }
        if last_error.is_none() {
            self.last_midi_note_off_status = "MIDI note-off: due notes flushed".to_string();
        }
        last_error
    }
```

- [ ] **Step 3: Register note-ons and send note-offs**

Replace `handle_midi_outputs` with:

```rust
    fn handle_midi_outputs(&mut self, outputs: &[MachineOutput]) -> Option<String> {
        let mut midi_error = None;

        for output in outputs {
            if let MachineOutput::MidiOutputIntent { intent } = output {
                if let Some(message) = self.send_midi_intent(intent) {
                    midi_error = Some(message);
                } else if intent.kind == mpc_core::MidiOutputIntentKind::NoteOn {
                    let duration = self.outbound_note_duration_millis(intent);
                    self.outbound_midi_notes
                        .register_note_on(intent, self.runtime_millis(), duration);
                    self.last_midi_note_off_status = format!(
                        "MIDI note-off scheduled ch {} note {} in {} ms",
                        intent.channel, intent.note, duration
                    );
                }
            }
        }

        midi_error
    }
```

At the start of `fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame)`, after `self.poll_host_midi_input();`, add:

```rust
        if let Some(error) = self.flush_due_midi_note_offs() {
            self.last_status = error;
        }
```

In the repaint request condition at the end of `ui`, include pending notes:

```rust
        if self.host_midi_input.is_some() || self.outbound_midi_notes.has_pending() {
```

- [ ] **Step 4: Add panic command to host MIDI UI**

In `draw_host_midi_status`, near the MIDI status labels, add:

```rust
            if ui.button("MIDI panic").clicked() {
                self.outbound_midi_notes.clear();
                self.last_midi_note_off_status = "MIDI panic: pending notes cleared".to_string();
                self.last_status = self.last_midi_note_off_status.clone();
            }
            ui.separator();
            ui.label(&self.last_midi_note_off_status);
```

- [ ] **Step 5: Run desktop compile**

Run:

```bash
cargo check -p mpc_desktop
```

Expected: PASS.

- [ ] **Step 6: Commit desktop MIDI scheduling**

```bash
git add apps/desktop/src/main.rs
git commit -m "feat: schedule outbound midi note offs"
```

## Task 7: Add Layered Pad Lighting

**Files:**
- Modify: `apps/desktop/src/main.rs`

- [ ] **Step 1: Add pure pad light tests**

At the bottom of `apps/desktop/src/main.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_light_layers_assignment_memory_and_pressure() {
        let mut memory = PadLightMemory::default();
        let pad = ProgramPad {
            bank: PadBank::A,
            pad_number: 1,
        };
        memory.record_hit(pad, 100, 1000);

        let visual = pad_visual_state(
            pad,
            true,
            false,
            false,
            Some(127),
            &memory,
            1100,
        );

        assert!(visual.assigned);
        assert!(visual.hit_memory > 0.0);
        assert_eq!(visual.active_pressure, 1.0);
        assert_eq!(visual.intensity, 1.0);
    }

    #[test]
    fn pad_light_memory_decays_to_zero() {
        let mut memory = PadLightMemory::default();
        let pad = ProgramPad {
            bank: PadBank::A,
            pad_number: 1,
        };
        memory.record_hit(pad, 100, 1000);

        let visual = pad_visual_state(pad, false, false, false, None, &memory, 2000);

        assert_eq!(visual.hit_memory, 0.0);
        assert_eq!(visual.intensity, 0.0);
    }
}
```

- [ ] **Step 2: Run failing desktop tests**

Run:

```bash
cargo test -p mpc_desktop pad_light -- --test-threads=1
```

Expected: FAIL because pad light types do not exist.

- [ ] **Step 3: Add pad light state types**

Add constants near the MIDI duration constants:

```rust
const PAD_HIT_MEMORY_MILLIS: u64 = 700;
const PAD_ASSIGNED_INTENSITY: f32 = 0.18;
const PAD_MISSING_INTENSITY: f32 = 0.35;
```

Add these types near `RuntimeSampleStatus`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
struct PadVisualState {
    assigned: bool,
    missing_runtime_sample: bool,
    selected: bool,
    active_pressure: f32,
    hit_memory: f32,
    intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PadHitLight {
    velocity: u8,
    hit_millis: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PadLightMemory {
    hits: BTreeMap<ProgramPad, PadHitLight>,
}

impl PadLightMemory {
    fn record_hit(&mut self, pad: ProgramPad, velocity: u8, now_millis: u64) {
        self.hits.insert(
            pad,
            PadHitLight {
                velocity,
                hit_millis: now_millis,
            },
        );
    }

    fn intensity_for(&self, pad: ProgramPad, now_millis: u64) -> f32 {
        let Some(hit) = self.hits.get(&pad) else {
            return 0.0;
        };
        let elapsed = now_millis.saturating_sub(hit.hit_millis);
        if elapsed >= PAD_HIT_MEMORY_MILLIS {
            return 0.0;
        }
        let velocity = f32::from(hit.velocity).clamp(1.0, 127.0) / 127.0;
        let remaining = 1.0 - (elapsed as f32 / PAD_HIT_MEMORY_MILLIS as f32);
        velocity * remaining
    }

    fn has_active_memory(&self, now_millis: u64) -> bool {
        self.hits
            .values()
            .any(|hit| now_millis.saturating_sub(hit.hit_millis) < PAD_HIT_MEMORY_MILLIS)
    }
}
```

Add this field to `MpcDesktopApp` and initialize it:

```rust
    pad_lights: PadLightMemory,
```

```rust
            pad_lights: PadLightMemory::default(),
```

- [ ] **Step 4: Add pad visual functions**

Add these free functions near `program_pad_label`:

```rust
fn pad_visual_state(
    pad: ProgramPad,
    assigned: bool,
    missing_runtime_sample: bool,
    selected: bool,
    active_velocity: Option<u8>,
    memory: &PadLightMemory,
    now_millis: u64,
) -> PadVisualState {
    let active_pressure = active_velocity
        .map(|velocity| f32::from(velocity).clamp(1.0, 127.0) / 127.0)
        .unwrap_or(0.0);
    let hit_memory = memory.intensity_for(pad, now_millis);
    let base = if missing_runtime_sample {
        PAD_MISSING_INTENSITY
    } else if assigned {
        PAD_ASSIGNED_INTENSITY
    } else {
        0.0
    };
    let intensity = active_pressure.max(hit_memory).max(base).min(1.0);
    PadVisualState {
        assigned,
        missing_runtime_sample,
        selected,
        active_pressure,
        hit_memory,
        intensity,
    }
}

fn pad_color_for_visual_state(visual: PadVisualState) -> egui::Color32 {
    if visual.missing_runtime_sample {
        let red = (160.0 + 80.0 * visual.intensity) as u8;
        return egui::Color32::from_rgb(red, 58, 42);
    }
    let green = (42.0 + 190.0 * visual.intensity) as u8;
    let blue = (44.0 + 110.0 * visual.hit_memory) as u8;
    let red = if visual.selected { 92 } else { 36 };
    egui::Color32::from_rgb(red, green, blue)
}
```

- [ ] **Step 5: Project playback outputs into pad light memory**

Add this method inside `impl MpcDesktopApp`:

```rust
    fn record_pad_lights_from_outputs(&mut self, outputs: &[MachineOutput]) {
        let now = self.runtime_millis();
        for output in outputs {
            match output {
                MachineOutput::PadTriggered {
                    bank,
                    pad,
                    velocity,
                } => self.pad_lights.record_hit(
                    ProgramPad {
                        bank: *bank,
                        pad_number: *pad,
                    },
                    *velocity,
                    now,
                ),
                MachineOutput::SamplePlaybackIntent { intent } => self.pad_lights.record_hit(
                    ProgramPad {
                        bank: intent.bank,
                        pad_number: intent.pad_number,
                    },
                    intent.velocity,
                    now,
                ),
                _ => {}
            }
        }
    }
```

In `dispatch_event`, after `let outputs = self.core.dispatch(event);`, add:

```rust
        self.record_pad_lights_from_outputs(&outputs);
```

- [ ] **Step 6: Render pad buttons with light state**

Replace the inner pad button block in `draw_pads` with:

```rust
                    let assigned = self
                        .core
                        .state()
                        .current_program
                        .pad_assignments
                        .iter()
                        .any(|assignment| assignment.pad == pad_address);
                    let missing_runtime_sample =
                        self.pad_has_missing_runtime_sample(pad_address);
                    let now = self.runtime_millis();
                    let visual = pad_visual_state(
                        pad_address,
                        assigned,
                        missing_runtime_sample,
                        selected,
                        None,
                        &self.pad_lights,
                        now,
                    );
                    let response = ui.add(
                        egui::Button::new(program_pad_label(pad_address))
                            .fill(pad_color_for_visual_state(visual))
                            .min_size(egui::vec2(72.0, 48.0)),
                    );
                    if response.clicked() {
                        self.dispatch_event(HardwareEvent::StrikePad {
                            bank: active_bank,
                            pad,
                            velocity: 100,
                        });
                    }
```

Add this helper method:

```rust
    fn pad_has_missing_runtime_sample(&self, pad: ProgramPad) -> bool {
        self.core
            .state()
            .current_program
            .pad_assignments
            .iter()
            .find(|assignment| assignment.pad == pad)
            .and_then(|assignment| {
                self.runtime_sample_statuses
                    .get(&assignment.sample.id)
                    .map(|status| !matches!(status, RuntimeSampleStatus::Loaded { .. }))
            })
            .unwrap_or(false)
    }
```

Update the repaint request condition:

```rust
        if self.host_midi_input.is_some()
            || self.outbound_midi_notes.has_pending()
            || self.pad_lights.has_active_memory(self.runtime_millis())
        {
```

- [ ] **Step 7: Run desktop tests and compile**

Run:

```bash
cargo test -p mpc_desktop pad_light -- --test-threads=1
cargo check -p mpc_desktop
```

Expected: PASS.

- [ ] **Step 8: Commit pad lighting**

```bash
git add apps/desktop/src/main.rs
git commit -m "feat: add layered pad lighting"
```

## Task 8: Update Docs And Run Full Verification

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README runtime sections**

In `README.md`, replace the final sentence under `Runtime WAV Import` with:

```markdown
Project files persist imported sample metadata plus rights-safe media references to user-owned local WAV paths. They do not embed WAV bytes. On project load, the desktop app tries to relink those paths and marks missing imported pads without deleting their assignments.
```

Add this section after `Runtime WAV Import`:

```markdown
## Runtime Host I/O

The desktop shell can refresh and select real host audio output devices through CPAL. Capture mode remains the deterministic test backend. MIDI output sends note-on and scheduled note-off messages through the selected MIDI backend so external synths do not hang on one-shot pad or sequence playback.

Pad lights are runtime UI state. Assigned pads are dim, recent hits glow briefly, missing imported WAV payloads are marked distinctly, and active strikes use velocity-derived brightness.
```

- [ ] **Step 2: Run formatting**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS. If it fails with formatting diffs, run:

```bash
cargo fmt --all
```

Then rerun `cargo fmt --all --check` and expect PASS.

- [ ] **Step 3: Run full automated verification**

Run:

```bash
cargo test -p mpc_core --test core_flow imported_media_reference -- --test-threads=1
cargo test -p mpc_core --test core_flow midi_output_intent -- --test-threads=1
cargo test -p mpc_storage -- --test-threads=1
cargo test -p mpc_audio -- --test-threads=1
cargo test -p mpc_midi -- --test-threads=1
cargo test -p mpc_desktop -- --test-threads=1
cargo check -p mpc_desktop
cargo test -p mpc_conformance --test fixtures all_json_fixtures_with_source_references_pass -- --exact --test-threads=1
cargo test --workspace
python3 tools/check_assets.py
git diff --check
```

Expected: all commands PASS.

- [ ] **Step 4: Commit docs update**

```bash
git add README.md
git commit -m "docs: document runtime io pad feedback"
```
