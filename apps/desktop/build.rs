use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=src/main.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    let source_path = Path::new(&manifest_dir).join("src/main.rs");
    let mut source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));

    replace_once(
        &mut source,
        r#"    SamplePlaybackResolution, SampleSourceKind, SetupPreferences, SyntheticSample,
    TimingCorrectSettings,
};"#,
        r#"    SamplePlaybackResolution, SampleSourceKind, SetupPreferences, SyntheticSample,
    apply_sample_flip_plan_to_project_snapshot, build_pad_bank_sample_flip_plan, SampleFlipRegion,
    SampleFlipSource, TimingCorrectSettings,
};"#,
    );
    replace_once(
        &mut source,
        r#"const FACTORY_808_909_ASSET_ROOT: &str = "local-assets/samples/808s-909s/808s+909s";"#,
        r#"const FACTORY_808_909_ASSET_ROOT: &str = "local-assets/samples/808s-909s/808s+909s";
const SAMPLE_FLIP_DEFAULT_SOURCE_PATH: &str = "local-assets/samples/flip-source.wav";
const SAMPLE_FLIP_LOAD_REASON: &str = "sample flip bank load";"#,
    );
    replace_once(
        &mut source,
        r#"    sample_import_path: String,"#,
        r#"    sample_import_path: String,
    sample_flip_source_path: String,
    sample_flip_target_bank: PadBank,
    sample_flip_start_frame: u32,
    sample_flip_end_frame: u32,
    last_sample_flip_status: String,"#,
    );
    replace_once(
        &mut source,
        r#"            sample_import_path: "local-assets/samples/import.wav".to_string(),"#,
        r#"            sample_import_path: "local-assets/samples/import.wav".to_string(),
            sample_flip_source_path: SAMPLE_FLIP_DEFAULT_SOURCE_PATH.to_string(),
            sample_flip_target_bank: PadBank::B,
            sample_flip_start_frame: 0,
            sample_flip_end_frame: 0,
            last_sample_flip_status: "Sample flip: none".to_string(),"#,
    );
    replace_once(
        &mut source,
        r#"                    ui.add(
                        egui::TextEdit::singleline(&mut self.sample_import_path)
                            .desired_width(280.0),
                    );
                    if ui.button("Load WAV to pad").clicked() {
                        self.load_wav_to_selected_pad();
                    }"#,
        r#"                    ui.add(
                        egui::TextEdit::singleline(&mut self.sample_import_path)
                            .desired_width(280.0),
                    );
                    if ui.button("Load WAV to pad").clicked() {
                        self.load_wav_to_selected_pad();
                    }
                    ui.separator();
                    ui.label("Flip");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.sample_flip_source_path)
                            .desired_width(260.0),
                    );
                    sample_flip_bank_combo(ui, &mut self.sample_flip_target_bank);
                    ui.add(
                        egui::DragValue::new(&mut self.sample_flip_start_frame)
                            .speed(1.0)
                            .prefix("Start "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.sample_flip_end_frame)
                            .speed(1.0)
                            .prefix("End "),
                    );
                    if ui.button("Flip WAV to bank").clicked() {
                        self.flip_wav_to_selected_bank();
                    }
                    ui.label(
                        egui::RichText::new(&self.last_sample_flip_status).color(muted_text()),
                    );"#,
    );
    replace_once(
        &mut source,
        r#"    fn clear_runtime_sample_payloads(&mut self, reason: &str) {"#,
        r#"    fn flip_wav_to_selected_bank(&mut self) {
        let path = self.sample_flip_source_path.trim().to_string();
        if path.is_empty() {
            let message = "Sample flip failed: source path is empty".to_string();
            self.last_sample_flip_status = message.clone();
            self.last_status = message;
            return;
        }

        let payload = match load_wav_sample_payload(&path) {
            Ok(payload) => payload,
            Err(error) => {
                let message = format!("Sample flip failed: {error}");
                self.last_sample_flip_status = message.clone();
                self.last_status = message;
                return;
            }
        };
        let sample_name = sample_name_from_path(&path);
        let frame_count = payload.length_frames_u32();
        let region = match sample_flip_region_from_ui(
            self.sample_flip_start_frame,
            self.sample_flip_end_frame,
            frame_count,
        ) {
            Ok(region) => region,
            Err(message) => {
                self.last_sample_flip_status = message.clone();
                self.last_status = message;
                return;
            }
        };
        let source = SampleFlipSource {
            source_id: sample_name.clone(),
            source_title: sample_name.clone(),
            source_path: path.clone(),
            managed_copy_path: None,
            sample_rate_hz: payload.sample_rate_hz,
            frame_count,
            byte_count: payload.byte_count,
        };
        let plan = match build_pad_bank_sample_flip_plan(
            source,
            self.sample_flip_target_bank,
            Some(region),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                let message = format!("Sample flip failed: {error}");
                self.last_sample_flip_status = message.clone();
                self.last_status = message;
                return;
            }
        };

        let mut snapshot = self.core.export_project_snapshot();
        if let Err(error) = apply_sample_flip_plan_to_project_snapshot(&mut snapshot, &plan) {
            let message = format!("Sample flip metadata failed: {error}");
            self.last_sample_flip_status = message.clone();
            self.last_status = message;
            return;
        }
        if let Err(error) = self.core.restore_project_snapshot(snapshot) {
            let message = format!("Sample flip restore failed: {error}");
            self.last_sample_flip_status = message.clone();
            self.last_status = message;
            return;
        }

        for slice in &plan.slices {
            self.runtime_samples.insert(
                slice.sample_id.clone(),
                slice.sample_name.clone(),
                payload.clone(),
            );
            self.runtime_sample_statuses.insert(
                slice.sample_id.clone(),
                RuntimeSampleStatus::Loaded {
                    path: path.clone(),
                    frame_count: plan.source.frame_count as usize,
                    sample_rate_hz: plan.source.sample_rate_hz,
                    byte_count: plan.source.byte_count,
                },
            );
        }
        self.prune_runtime_samples_to_project_metadata();
        self.last_audio_render = None;
        self.last_audio_render_error = None;
        self.last_runtime_sample_status =
            runtime_sample_relink_status_text(SAMPLE_FLIP_LOAD_REASON, &self.runtime_sample_statuses);
        self.last_project_file_status =
            "Sample flip metadata changed; save project file to persist".to_string();
        self.sample_flip_start_frame = plan.region.start_frame;
        self.sample_flip_end_frame = plan.region.end_frame;
        self.last_sample_flip_status = format!(
            "Sample flip: {} -> Bank {} ({} chops, region {}..{} / {} frames)",
            sample_name,
            plan.bank.label(),
            plan.slices.len(),
            plan.region.start_frame,
            plan.region.end_frame,
            plan.region.window_length_frames()
        );
        self.last_status = self.last_sample_flip_status.clone();
    }

    fn clear_runtime_sample_payloads(&mut self, reason: &str) {"#,
    );
    replace_once(
        &mut source,
        r#"fn runtime_sample_ids_referenced_by_project(state: &MpcState) -> BTreeSet<String> {"#,
        r#"fn sample_flip_region_from_ui(
    requested_start_frame: u32,
    requested_end_frame: u32,
    frame_count: u32,
) -> Result<SampleFlipRegion, String> {
    if frame_count == 0 {
        return Err("Sample flip failed: source contains no frames".to_string());
    }

    let max_frame = frame_count.saturating_sub(1);
    let start_frame = requested_start_frame.min(max_frame);
    let end_frame = if requested_end_frame == 0 {
        max_frame
    } else {
        requested_end_frame.min(max_frame)
    };
    if end_frame < start_frame {
        return Err(format!(
            "Sample flip failed: start frame {start_frame} is after end frame {end_frame}"
        ));
    }

    Ok(SampleFlipRegion {
        start_frame,
        end_frame,
    })
}

fn sample_flip_bank_combo(ui: &mut egui::Ui, selected_bank: &mut PadBank) {
    egui::ComboBox::from_label("Flip bank")
        .selected_text(selected_bank.label())
        .show_ui(ui, |ui| {
            for bank in [PadBank::A, PadBank::B, PadBank::C, PadBank::D] {
                ui.selectable_value(selected_bank, bank, bank.label());
            }
        });
}

fn runtime_sample_ids_referenced_by_project(state: &MpcState) -> BTreeSet<String> {"#,
    );
    replace_once(
        &mut source,
        r#"    fn test_midi_output_intent() -> mpc_core::MidiOutputIntent {"#,
        r#"    #[test]
    fn sample_flip_region_defaults_zero_end_to_full_source() {
        let region = sample_flip_region_from_ui(0, 0, 100)
            .expect("zero end should mean full source");

        assert_eq!(
            region,
            SampleFlipRegion {
                start_frame: 0,
                end_frame: 99,
            }
        );
    }

    #[test]
    fn sample_flip_region_clamps_requested_frames_to_source_bounds() {
        let region = sample_flip_region_from_ui(10, 500, 128)
            .expect("end should clamp to the loaded source");

        assert_eq!(
            region,
            SampleFlipRegion {
                start_frame: 10,
                end_frame: 127,
            }
        );
    }

    fn test_midi_output_intent() -> mpc_core::MidiOutputIntent {"#,
    );

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR should be set");
    let generated_path = Path::new(&out_dir).join("desktop_sample_flip_main.rs");
    fs::write(&generated_path, source)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", generated_path.display()));
}

fn replace_once(source: &mut String, needle: &str, replacement: &str) {
    let match_count = source.matches(needle).count();
    assert_eq!(
        match_count, 1,
        "expected exactly one generated desktop source patch match for: {needle:?}"
    );
    *source = source.replacen(needle, replacement, 1);
}
