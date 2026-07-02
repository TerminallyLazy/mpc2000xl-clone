use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/main.rs");

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set by Cargo"),
    );
    let source_path = manifest_dir.join("src/main.rs");
    let template = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source_path.display()));
    let generated = inject_sample_flip_ui(&template);
    let output_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR should be set by Cargo"))
        .join("main_with_sample_flip_ui.rs");

    fs::write(&output_path, generated)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", output_path.display()));
}

fn inject_sample_flip_ui(template: &str) -> String {
    let load_wav_button = r#"                    if ui.button("Load WAV to pad").clicked() {
                        self.load_wav_to_selected_pad();
                    }
"#;
    let flip_button = r#"                    if ui.button("Flip WAV to active bank").clicked() {
                        self.flip_wav_to_active_bank();
                    }
"#;
    let load_wav_button_count = template.matches(load_wav_button).count();
    assert_eq!(
        load_wav_button_count, 2,
        "expected to find exactly two desktop Load WAV button sites"
    );

    let generated = template.replace(
        load_wav_button,
        &format!("{load_wav_button}{flip_button}"),
    );

    let method_anchor = "    fn clear_runtime_sample_payloads(&mut self, reason: &str) {\n";
    assert!(
        generated.contains(method_anchor),
        "expected desktop runtime sample method anchor"
    );

    generated.replace(
        method_anchor,
        &format!("{}{method_anchor}", sample_flip_method()),
    )
}

fn sample_flip_method() -> &'static str {
    r#"    fn flip_wav_to_active_bank(&mut self) {
        let path = self.sample_import_path.trim().to_string();
        if path.is_empty() {
            let message = "Sample flip failed: path is empty".to_string();
            self.last_runtime_sample_status = message.clone();
            self.last_status = message;
            return;
        }

        let payload = match load_wav_sample_payload(&path) {
            Ok(payload) => payload,
            Err(error) => {
                let message = format!("Sample flip failed: {error}");
                self.last_runtime_sample_status = message.clone();
                self.last_status = message;
                return;
            }
        };

        let bank = self.core.state().pad_bank;
        let sample_name = sample_name_from_path(&path);
        let source = mpc_core::SampleFlipSource {
            source_id: sample_name.clone(),
            source_title: sample_name.clone(),
            source_path: path.clone(),
            managed_copy_path: None,
            sample_rate_hz: payload.sample_rate_hz,
            frame_count: payload.length_frames_u32(),
            byte_count: payload.byte_count,
        };
        let plan = match mpc_core::build_pad_bank_sample_flip_plan(source, bank, None) {
            Ok(plan) => plan,
            Err(error) => {
                let message = format!("Sample flip plan failed: {error}");
                self.last_runtime_sample_status = message.clone();
                self.last_status = message;
                return;
            }
        };

        let mut snapshot = self.core.export_project_snapshot();
        if let Err(error) =
            mpc_core::apply_sample_flip_plan_to_project_snapshot(&mut snapshot, &plan)
        {
            let message = format!("Sample flip metadata failed: {error}");
            self.last_runtime_sample_status = message.clone();
            self.last_status = message;
            return;
        }
        if let Err(error) = self.core.restore_project_snapshot(snapshot) {
            let message = format!("Sample flip restore failed: {error}");
            self.last_runtime_sample_status = message.clone();
            self.last_status = message;
            return;
        }

        self.clear_runtime_sample_payloads("sample flip");
        let frame_count = payload.frame_count;
        let sample_rate_hz = payload.sample_rate_hz;
        let byte_count = payload.byte_count;
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
                    frame_count,
                    sample_rate_hz,
                    byte_count,
                },
            );
        }

        self.last_audio_render = None;
        self.last_audio_render_error = None;
        self.last_runtime_sample_status = format!(
            "Sample flip: {} -> bank {} as {} pads ({} frames @ {} Hz)",
            sample_name,
            bank.label(),
            plan.slices.len(),
            frame_count,
            sample_rate_hz
        );
        self.last_status = self.last_runtime_sample_status.clone();
    }

"#
}
