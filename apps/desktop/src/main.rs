use eframe::egui;
use mpc_audio::{
    AudioRenderKind, AudioRenderSettings, AudioRenderSummary, CaptureAudioBackend, HostAudioEngine,
    HostAudioError, HostAudioEvent, HostAudioPlaybackReport, HostAudioState,
};
use mpc_core::{
    DiskOperation, HardwareEvent, MachineOutput, MidiSettingsField, Mode, MpcCore, MpcState,
    PadAssignmentChange, PadBank, PanelControl, ProgramPad, SampleCatalogEntry,
    SamplePlaybackIntent, SamplePlaybackResolution, SetupPreferences, TimingCorrectSettings,
};
use mpc_midi::{
    CaptureMidiBackend, HostMidiEngine, HostMidiEvent, HostMidiOutputReport, HostMidiState,
};
use mpc_storage::{
    DEFAULT_PROJECT_FILE_PATH, load_project_file_with_report,
    save_project_file as save_project_file_to_path,
};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MPC2000XL Clone Foundation")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([920.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MPC2000XL Clone Foundation",
        options,
        Box::new(|_cc| Ok(Box::new(MpcDesktopApp::default()))),
    )
}

struct MpcDesktopApp {
    core: MpcCore,
    host_audio: HostAudioEngine<CaptureAudioBackend>,
    host_midi: HostMidiEngine<CaptureMidiBackend>,
    last_status: String,
    last_synthetic_render: Option<AudioRenderSummary>,
    last_synthetic_render_error: Option<String>,
    last_project_snapshot_json: Option<String>,
    last_project_snapshot_status: String,
    last_project_snapshot_version: Option<u16>,
    last_project_snapshot_bytes: Option<usize>,
    project_file_path: String,
    last_project_file_status: String,
    last_project_file_version: Option<u16>,
    last_project_file_bytes: Option<usize>,
    midi_channel: u8,
    midi_note: u8,
    midi_velocity: u8,
}

impl Default for MpcDesktopApp {
    fn default() -> Self {
        Self {
            core: MpcCore::new(),
            host_audio: HostAudioEngine::new(
                CaptureAudioBackend::new(8),
                AudioRenderSettings::preview(),
            )
            .expect("desktop host audio preview settings should satisfy guardrails"),
            host_midi: HostMidiEngine::enabled(CaptureMidiBackend::new(16)),
            last_status: "Ready".to_string(),
            last_synthetic_render: None,
            last_synthetic_render_error: None,
            last_project_snapshot_json: None,
            last_project_snapshot_status: "Snapshot: none".to_string(),
            last_project_snapshot_version: None,
            last_project_snapshot_bytes: None,
            project_file_path: DEFAULT_PROJECT_FILE_PATH.to_string(),
            last_project_file_status: "Project file: none".to_string(),
            last_project_file_version: None,
            last_project_file_bytes: None,
            midi_channel: 1,
            midi_note: 36,
            midi_velocity: 100,
        }
    }
}

impl eframe::App for MpcDesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            ui.heading("MPC2000XL Clone Foundation");
            ui.label("Rights-safe desktop shell wired to deterministic machine core.");
            ui.separator();

            self.draw_lcd(ui);
            ui.add_space(16.0);
            self.draw_mode_buttons(ui);
            ui.add_space(16.0);
            self.draw_edit_controls(ui);
            ui.add_space(16.0);
            self.draw_project_snapshot_controls(ui);
            ui.add_space(16.0);
            self.draw_transport(ui);
            self.draw_midi_controls(ui);
            self.draw_timing_correct_controls(ui);
            self.draw_setup_status(ui);
            self.draw_sequence_status(ui);
            self.draw_program_status(ui);
            self.draw_sample_status(ui);
            self.draw_audio_render_status(ui);
            self.draw_host_audio_status(ui);
            self.draw_host_midi_status(ui);
            ui.add_space(16.0);
            self.draw_pads(ui);
            ui.add_space(16.0);
            ui.label(format!("Status: {}", self.last_status));
        });
    }
}

impl MpcDesktopApp {
    fn draw_lcd(&mut self, ui: &mut egui::Ui) {
        let lcd = self.core.state().lcd.clone();
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(720.0);
            ui.heading(lcd.title);
            for line in &lcd.lines {
                ui.monospace(line);
            }
            ui.horizontal_wrapped(|ui| {
                for (index, soft_key) in lcd.soft_keys.iter().enumerate() {
                    let soft_key_number = index as u8 + 1;
                    if ui
                        .button(format!("F{soft_key_number} {soft_key}"))
                        .clicked()
                    {
                        self.dispatch_event(HardwareEvent::Press {
                            control: PanelControl::SoftKey(soft_key_number),
                        });
                    }
                }
            });
        });
    }

    fn draw_mode_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            self.mode_button(ui, "MAIN", PanelControl::MainScreen, Mode::Main);
            self.mode_button(ui, "PROGRAM", PanelControl::Program, Mode::Program);
            self.mode_button(ui, "SAMPLE", PanelControl::Sample, Mode::Sample);
            self.mode_button(ui, "TRIM", PanelControl::Trim, Mode::Trim);
            self.mode_button(ui, "SONG", PanelControl::Song, Mode::Song);
            self.mode_button(ui, "MIDI", PanelControl::Midi, Mode::Midi);
            self.mode_button(
                ui,
                "TIMING",
                PanelControl::TimingCorrect,
                Mode::TimingCorrect,
            );
            self.mode_button(ui, "DISK", PanelControl::Disk, Mode::Disk);
            self.mode_button(ui, "SETUP", PanelControl::Setup, Mode::Setup);
        });
    }

    fn mode_button(&mut self, ui: &mut egui::Ui, label: &str, control: PanelControl, mode: Mode) {
        let selected = self.core.state().mode == mode;
        if ui.selectable_label(selected, label).clicked() {
            self.dispatch_event(HardwareEvent::Press { control });
        }
    }

    fn draw_edit_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Cursor ^").clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorUp,
                });
            }
            if ui.button("Cursor v").clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorDown,
                });
            }
            if ui.button("Cursor <").clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorLeft,
                });
            }
            if ui.button("Cursor >").clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorRight,
                });
            }
            ui.separator();
            if ui.button("Wheel -").clicked() {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: -1 });
            }
            if ui.button("Wheel +").clicked() {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: 1 });
            }
        });
    }

    fn draw_project_snapshot_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Save Snapshot").clicked() {
                self.save_project_snapshot();
            }

            let has_snapshot = self.last_project_snapshot_json.is_some();
            if ui
                .add_enabled(has_snapshot, egui::Button::new("Load Last Snapshot"))
                .clicked()
            {
                self.load_last_project_snapshot();
            }

            ui.separator();
            ui.label(project_snapshot_status_text(
                &self.last_project_snapshot_status,
                self.last_project_snapshot_version,
                self.last_project_snapshot_bytes,
            ));
        });

        ui.horizontal_wrapped(|ui| {
            ui.label("Project file");
            ui.add(egui::TextEdit::singleline(&mut self.project_file_path).desired_width(460.0));

            if ui.button("Save File").clicked() {
                self.save_project_file();
            }
            if ui.button("Load File").clicked() {
                self.load_project_file();
            }

            ui.separator();
            ui.label(project_file_status_text(
                &self.last_project_file_status,
                self.last_project_file_version,
                self.last_project_file_bytes,
            ));
        });
    }

    fn save_project_snapshot(&mut self) {
        let snapshot = self.core.export_project_snapshot();
        match self.core.to_project_json() {
            Ok(json) => {
                let byte_count = json.len();
                self.last_project_snapshot_json = Some(json);
                self.last_project_snapshot_version = Some(snapshot.version);
                self.last_project_snapshot_bytes = Some(byte_count);
                self.last_project_snapshot_status = format!(
                    "saved metadata snapshot v{} ({byte_count} bytes)",
                    snapshot.version
                );
                self.last_status = format!(
                    "Saved project snapshot v{} ({byte_count} bytes)",
                    snapshot.version
                );
            }
            Err(error) => {
                let message = format!("Snapshot save failed: {error}");
                self.last_project_snapshot_status = message.clone();
                self.last_status = message;
            }
        }
    }

    fn load_last_project_snapshot(&mut self) {
        let Some(json) = self.last_project_snapshot_json.clone() else {
            self.last_project_snapshot_status = "Snapshot: none saved".to_string();
            self.last_status = "No project snapshot saved".to_string();
            return;
        };

        let byte_count = json.len();
        match MpcCore::from_project_json(&json) {
            Ok(snapshot) => {
                let version = snapshot.version;
                match self.core.restore_project_snapshot(snapshot) {
                    Ok(()) => {
                        self.last_project_snapshot_version = Some(version);
                        self.last_project_snapshot_bytes = Some(byte_count);
                        self.last_project_snapshot_status = format!(
                            "loaded metadata snapshot v{version} ({byte_count} bytes); transport stopped"
                        );
                        self.last_status = format!(
                            "Loaded project snapshot v{version}; transport stopped/disarmed"
                        );
                    }
                    Err(error) => {
                        let message = format!("Snapshot load failed: {error}");
                        self.last_project_snapshot_status = message.clone();
                        self.last_status = message;
                    }
                }
            }
            Err(error) => {
                let message = format!("Snapshot load failed: {error}");
                self.last_project_snapshot_status = message.clone();
                self.last_status = message;
            }
        }
    }

    fn save_project_file(&mut self) {
        let path = self.project_file_path.trim();
        match save_project_file_to_path(&self.core, path) {
            Ok(report) => {
                self.last_project_file_version = Some(report.snapshot_version);
                self.last_project_file_bytes = Some(report.byte_count);
                self.last_project_file_status =
                    format!("saved metadata JSON to {}", report.path.display());
                self.last_status = format!(
                    "Saved project file v{} ({} bytes)",
                    report.snapshot_version, report.byte_count
                );
            }
            Err(error) => {
                let message = format!("Project file save failed: {error}");
                self.last_project_file_version = None;
                self.last_project_file_bytes = None;
                self.last_project_file_status = message.clone();
                self.last_status = message;
            }
        }
    }

    fn load_project_file(&mut self) {
        let path = self.project_file_path.trim();
        match load_project_file_with_report(path) {
            Ok(loaded) => {
                let report = loaded.report;
                match self.core.restore_project_snapshot(loaded.snapshot) {
                    Ok(()) => {
                        self.last_project_file_version = Some(report.snapshot_version);
                        self.last_project_file_bytes = Some(report.byte_count);
                        self.last_project_file_status = format!(
                            "loaded metadata JSON from {}; transport stopped",
                            report.path.display()
                        );
                        self.last_status = format!(
                            "Loaded project file v{}; transport stopped/disarmed",
                            report.snapshot_version
                        );
                    }
                    Err(error) => {
                        let message = format!("Project file load failed: {error}");
                        self.last_project_file_version = None;
                        self.last_project_file_bytes = None;
                        self.last_project_file_status = message.clone();
                        self.last_status = message;
                    }
                }
            }
            Err(error) => {
                let message = format!("Project file load failed: {error}");
                self.last_project_file_version = None;
                self.last_project_file_bytes = None;
                self.last_project_file_status = message.clone();
                self.last_status = message;
            }
        }
    }

    fn dispatch_event(&mut self, event: HardwareEvent) {
        let outputs = self.core.dispatch(event);
        let render_or_host_error = self.handle_audio_outputs(&outputs);
        let midi_host_error = self.handle_midi_outputs(&outputs);
        let disk_operation_status = self.handle_disk_operation_request(&outputs);
        self.last_status = disk_operation_status
            .or(midi_host_error)
            .or(render_or_host_error)
            .unwrap_or_else(|| Self::status_from_outputs(&outputs, self.core.state()));
    }

    fn handle_disk_operation_request(&mut self, outputs: &[MachineOutput]) -> Option<String> {
        let Some(operation) = outputs.iter().find_map(|output| match output {
            MachineOutput::DiskOperationRequested { operation } => Some(*operation),
            _ => None,
        }) else {
            return None;
        };

        match operation {
            DiskOperation::SaveProject => self.save_project_file(),
            DiskOperation::LoadProject => self.load_project_file(),
        }
        Some(self.last_status.clone())
    }

    fn handle_audio_outputs(&mut self, outputs: &[MachineOutput]) -> Option<String> {
        let mut playback_error = None;

        for output in outputs {
            match output {
                MachineOutput::SamplePlaybackIntent { intent } => {
                    let report = self.host_audio.play_intent_with_render_summary(intent);
                    if let Some(message) =
                        self.record_audio_report(report, "Synthetic render failed")
                    {
                        playback_error = Some(message);
                    }
                }
                MachineOutput::MetronomeClick { intent } => {
                    let report = self
                        .host_audio
                        .play_count_in_click_with_render_summary(intent);
                    if let Some(message) =
                        self.record_audio_report(report, "Count-in click render failed")
                    {
                        playback_error = Some(message);
                    }
                }
                _ => {}
            }
        }

        playback_error
    }

    fn record_audio_report(
        &mut self,
        report: HostAudioPlaybackReport,
        render_error_prefix: &str,
    ) -> Option<String> {
        self.last_synthetic_render = report.render_summary;
        self.last_synthetic_render_error = None;

        if let HostAudioEvent::Failed {
            error: HostAudioError::Render { error },
            ..
        } = &report.event
        {
            let message = format!("{render_error_prefix}: {error}");
            self.last_synthetic_render_error = Some(message.clone());
            return Some(message);
        }

        host_audio_error_message(&report.event)
    }

    fn handle_midi_outputs(&mut self, outputs: &[MachineOutput]) -> Option<String> {
        let mut midi_error = None;

        for output in outputs {
            if let MachineOutput::MidiOutputIntent { intent } = output {
                let report = self.host_midi.send_intent(intent);
                if let Some(message) = self.record_midi_report(report) {
                    midi_error = Some(message);
                }
            }
        }

        midi_error
    }

    fn record_midi_report(&mut self, report: HostMidiOutputReport) -> Option<String> {
        host_midi_error_message(&report.event)
    }

    fn status_from_outputs(outputs: &[MachineOutput], state: &MpcState) -> String {
        if let Some(MachineOutput::CountInStarted { total_ticks, bars }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::CountInStarted { .. }))
        {
            return format!("Count-in started: {bars} bar(s), {total_ticks} ticks");
        }

        if let Some(MachineOutput::CountInCompleted { total_ticks }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::CountInCompleted { .. }))
        {
            return format!("Count-in completed: {total_ticks} ticks");
        }

        if let Some(MachineOutput::MetronomeClick { intent }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::MetronomeClick { .. }))
        {
            let accent = if intent.accent { "accent" } else { "click" };
            return format!(
                "Count-in {accent}: bar {} beat {} tick {}",
                intent.bar_index, intent.beat_index, intent.count_in_tick
            );
        }

        if let Some(MachineOutput::MidiSettingsChanged {
            input_channel,
            base_note,
            selected_field,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::MidiSettingsChanged { .. }))
        {
            let selected_value = match selected_field {
                MidiSettingsField::InputChannel => midi_input_channel_text(*input_channel),
                MidiSettingsField::BaseNote => base_note.to_string(),
            };
            return format!(
                "MIDI settings {}={} base {} range {}",
                selected_field.label(),
                selected_value,
                base_note,
                midi_note_range_text(*base_note)
            );
        }

        if let Some(MachineOutput::MidiInputIgnored { reason }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::MidiInputIgnored { .. }))
        {
            return format!("MIDI ignored: {reason}");
        }

        if let Some(MachineOutput::MidiOutputIntent { intent }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::MidiOutputIntent { .. }))
        {
            return format!(
                "MIDI out ch {} note {} vel {} from {:?}{:02} {}",
                intent.channel,
                intent.note,
                intent.velocity,
                intent.bank,
                intent.pad_number,
                intent.source_sample_name
            );
        }

        if let Some(MachineOutput::TimingCorrectChanged {
            settings,
            selected_field,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::TimingCorrectChanged { .. }))
        {
            return format!(
                "TIMING {} selected: {}",
                selected_field.label(),
                timing_correct_settings_text(*settings)
            );
        }

        if let Some(MachineOutput::SetupPreferencesChanged {
            preferences,
            selected_field,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SetupPreferencesChanged { .. }))
        {
            return format!(
                "SETUP {} selected: {}",
                selected_field.label(),
                setup_preferences_text(*preferences)
            );
        }

        if let Some(MachineOutput::DiskOperationSelected { operation }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::DiskOperationSelected { .. }))
        {
            return format!("DISK selected {}", operation.display_label());
        }

        if let Some(MachineOutput::DiskOperationRequested { operation }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::DiskOperationRequested { .. }))
        {
            return format!("DISK requested {}", operation.display_label());
        }

        if let Some(MachineOutput::SongStepChanged { index, field, step }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SongStepChanged { .. }))
        {
            return format!(
                "SONG step {:02} {} -> Seq {:02} repeats {:02}",
                index + 1,
                field.label(),
                u16::from(step.sequence_index) + 1,
                step.repeats
            );
        }

        if let Some(MachineOutput::SongStepInserted { index, step }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SongStepInserted { .. }))
        {
            return format!(
                "SONG inserted step {:02}: Seq {:02} repeats {:02}",
                index + 1,
                u16::from(step.sequence_index) + 1,
                step.repeats
            );
        }

        if let Some(MachineOutput::SongStepDeleted { index, step }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SongStepDeleted { .. }))
        {
            return format!(
                "SONG deleted step {:02}: Seq {:02} repeats {:02}",
                index + 1,
                u16::from(step.sequence_index) + 1,
                step.repeats
            );
        }

        if let Some(MachineOutput::SongStepSelected { index, step }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SongStepSelected { .. }))
        {
            return format!(
                "SONG selected step {:02}/{:02}: Seq {:02} repeats {:02}",
                index + 1,
                state.song_steps.len(),
                u16::from(step.sequence_index) + 1,
                step.repeats
            );
        }

        if let Some(MachineOutput::TimingCorrectApplied {
            original_tick,
            quantized_tick,
            division,
            swing_percent,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::TimingCorrectApplied { .. }))
        {
            return format!(
                "Timing Correct applied: tick {original_tick} -> {quantized_tick} ({} swing {}%)",
                division.label(),
                swing_percent
            );
        }

        if let Some(MachineOutput::MidiNoteMapped {
            channel,
            note,
            bank,
            pad,
            velocity,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::MidiNoteMapped { .. }))
        {
            let playback = playback_intent_from_outputs(outputs)
                .map(|intent| format!(" {}", playback_intent_status_text(intent)))
                .unwrap_or_default();
            return format!(
                "MIDI ch {channel} note {note} -> {bank:?}{pad:02} velocity {velocity}{playback}"
            );
        }

        if let Some(MachineOutput::Ignored { reason }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::Ignored { .. }))
        {
            return format!("Ignored: {reason}");
        }

        if let Some(MachineOutput::SampleMetadataCreated {
            sample,
            source_kind,
            target_pad,
            length_frames,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SampleMetadataCreated { .. }))
        {
            return format!(
                "{} sample metadata {} created for {} ({} frames, assigned)",
                source_kind.label(),
                sample.name,
                program_pad_label(*target_pad),
                length_frames
            );
        }

        if let Some(MachineOutput::SampleSelected { entry }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SampleSelected { .. }))
        {
            return format!(
                "Selected sample {:02}/{:02} {} {} ({}, {} frames, metadata only)",
                entry.index.min(99),
                entry.count.min(99),
                entry.sample.name,
                entry.source_kind.label(),
                program_pad_label(entry.source_pad),
                entry.length_frames
            );
        }

        if let Some(MachineOutput::PadAssignmentChanged {
            bank,
            pad,
            action,
            assignment,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::PadAssignmentChanged { .. }))
        {
            return match assignment {
                Some(assignment) => format!(
                    "Program pad {bank:?}{pad:02} {} to {}",
                    assignment_action_text(*action),
                    assignment.sample.name
                ),
                None => format!(
                    "Program pad {bank:?}{pad:02} {}",
                    assignment_action_text(*action)
                ),
            };
        }

        if let Some(MachineOutput::PadParameterChanged {
            bank,
            pad,
            parameter,
            value,
            assignment,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::PadParameterChanged { .. }))
        {
            return format!(
                "Program pad {bank:?}{pad:02} {} set to {} ({})",
                parameter.label(),
                value,
                assignment.sample.name
            );
        }

        if let Some(MachineOutput::SampleTrimChanged {
            sample_id,
            start_frame,
            end_frame,
            window_length_frames,
            selected_field,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SampleTrimChanged { .. }))
        {
            return format!(
                "TRIM {} {}={}..{} window {} frames",
                sample_id,
                selected_field.label(),
                start_frame,
                end_frame,
                window_length_frames
            );
        }

        if let Some(MachineOutput::SequenceEventRecorded { event }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SequenceEventRecorded { .. }))
        {
            let sample = event
                .playback
                .as_ref()
                .map(|intent| format!(" {}", playback_intent_status_text(intent)))
                .unwrap_or_else(|| " unassigned".to_string());
            return format!(
                "Recorded Trk {:02} {:?}{:02} velocity {} at tick {}{} ({} events)",
                event.selected_track,
                event.pad_bank,
                event.pad_number,
                event.velocity,
                event.tick,
                sample,
                state.recorded_events.len()
            );
        }

        if let Some(MachineOutput::SequenceEventsErased {
            selected_track,
            count,
            ..
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SequenceEventsErased { .. }))
        {
            return format!(
                "Erased {count} event(s) from Trk {selected_track:02} ({} events remain)",
                state.recorded_events.len()
            );
        }

        if let Some(MachineOutput::PlayheadLocated { tick }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::PlayheadLocated { .. }))
        {
            return format!("Located playhead to tick {tick}");
        }

        if let Some(MachineOutput::LoopChanged { enabled }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::LoopChanged { .. }))
        {
            return format!(
                "Sequence loop {}",
                if *enabled { "enabled" } else { "disabled" }
            );
        }

        if let Some(MachineOutput::TrackMuteChanged {
            track,
            muted,
            muted_tracks,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::TrackMuteChanged { .. }))
        {
            return format!(
                "Track mute Trk {track:02} {} ({} muted)",
                if *muted { "on" } else { "off" },
                muted_tracks.len()
            );
        }

        if let Some(MachineOutput::BankChanged { bank }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::BankChanged { .. }))
        {
            return format!("Pad bank {} selected", bank.label());
        }

        if let Some(MachineOutput::SequenceEventPlayed { event }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SequenceEventPlayed { .. }))
        {
            let sample = event
                .playback
                .as_ref()
                .map(|intent| format!(" {}", playback_intent_status_text(intent)))
                .unwrap_or_else(|| " unassigned".to_string());
            return format!(
                "Played Trk {:02} {:?}{:02} velocity {} at tick {}{}",
                event.selected_track,
                event.pad_bank,
                event.pad_number,
                event.velocity,
                event.tick,
                sample
            );
        }

        if let Some(MachineOutput::SamplePlaybackIntent { intent }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SamplePlaybackIntent { .. }))
        {
            return format!(
                "Playback intent Trk {:02} Pgm {:02} {}",
                intent.selected_track,
                intent.program_index,
                playback_intent_status_text(intent)
            );
        }

        if let Some(MachineOutput::SamplePlaybackMiss { miss }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SamplePlaybackMiss { .. }))
        {
            return format!(
                "Playback miss Trk {:02} Pgm {:02} {:?}{:02}: {:?}",
                miss.selected_track, miss.program_index, miss.bank, miss.pad_number, miss.reason
            );
        }

        if let Some(MachineOutput::PadTriggered {
            bank,
            pad,
            velocity,
        }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::PadTriggered { .. }))
        {
            return format!("Pad {bank:?}{pad:02} velocity {velocity}");
        }

        if let Some(MachineOutput::TransportChanged { playing, recording }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::TransportChanged { .. }))
        {
            return format!("Transport playing={playing} recording={recording}");
        }

        if let Some(MachineOutput::ModeChanged { mode }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::ModeChanged { .. }))
        {
            return format!("Mode changed to {mode:?}");
        }

        if outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::LcdChanged))
        {
            return main_screen_status(state);
        }

        "No machine output".to_string()
    }

    fn draw_transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("STOP").clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::Stop,
                });
            }
            if ui.button("PLAY").clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::Play,
                });
            }
            if ui.button("REC").clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::Rec,
                });
            }
            if ui.button("OVERDUB").clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::Overdub,
                });
            }
            if ui.button("LOCATE START").clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::LocateStart,
                });
            }
            let loop_label = if self.core.state().loop_enabled {
                "LOOP ON"
            } else {
                "LOOP OFF"
            };
            if ui.button(loop_label).clicked() {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::ToggleLoop,
                });
            }
            if ui.button("TICK +100ms").clicked() {
                self.dispatch_event(HardwareEvent::Tick { micros: 100_000 });
            }
        });
    }

    fn draw_midi_controls(&mut self, ui: &mut egui::Ui) {
        let state = self.core.state();
        let midi_mode = state.mode == Mode::Midi;
        let midi_input_channel = state.midi_input_channel;
        let midi_base_note = state.midi_base_note;
        let selected_midi_settings_field = state.selected_midi_settings_field;
        let top_note = midi_base_note.saturating_add(15);
        let fifth_note = midi_base_note.saturating_add(4);
        let ignored_note = if midi_base_note > 0 {
            midi_base_note - 1
        } else {
            top_note.saturating_add(1)
        };

        ui.horizontal_wrapped(|ui| {
            ui.label("MIDI sim");
            ui.add(egui::Slider::new(&mut self.midi_channel, 1..=16).text("Ch"));
            ui.add(egui::Slider::new(&mut self.midi_note, 0..=127).text("Note"));
            ui.add(egui::Slider::new(&mut self.midi_velocity, 1..=127).text("Vel"));

            if ui.button("Note On").clicked() {
                self.dispatch_event(HardwareEvent::MidiNoteOn {
                    channel: self.midi_channel,
                    note: self.midi_note,
                    velocity: self.midi_velocity,
                });
            }
            if ui.button("Note Off").clicked() {
                self.dispatch_event(HardwareEvent::MidiNoteOff {
                    channel: self.midi_channel,
                    note: self.midi_note,
                    velocity: 64,
                });
            }

            ui.separator();
            if ui.button(format!("{midi_base_note} -> A01")).clicked() {
                self.send_midi_note_on(midi_base_note);
            }
            if ui.button(format!("{fifth_note} -> A05")).clicked() {
                self.send_midi_note_on(fifth_note);
            }
            if ui.button(format!("{top_note} -> A16")).clicked() {
                self.send_midi_note_on(top_note);
            }
            if ui.button(format!("{ignored_note} ignored")).clicked() {
                self.send_midi_note_on(ignored_note);
            }
        });

        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "MIDI settings: input {} base {} range {} field {} host output capture",
                midi_input_channel_text(midi_input_channel),
                midi_base_note,
                midi_note_range_text(midi_base_note),
                selected_midi_settings_field.label()
            ));
            ui.separator();
            if ui
                .add_enabled(midi_mode, egui::Button::new("Setting <"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorLeft,
                });
            }
            if ui
                .add_enabled(midi_mode, egui::Button::new("Setting >"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorRight,
                });
            }
            if ui
                .add_enabled(midi_mode, egui::Button::new("Value -"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: -1 });
            }
            if ui
                .add_enabled(midi_mode, egui::Button::new("Value +"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: 1 });
            }
        });
    }

    fn send_midi_note_on(&mut self, note: u8) {
        self.midi_note = note;
        self.dispatch_event(HardwareEvent::MidiNoteOn {
            channel: self.midi_channel,
            note,
            velocity: self.midi_velocity,
        });
    }

    fn draw_timing_correct_controls(&mut self, ui: &mut egui::Ui) {
        let state = self.core.state();
        let timing_mode = state.mode == Mode::TimingCorrect;
        let settings = state.timing_correct;
        let selected_field = state.selected_timing_correct_field;

        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "TC: {} field {}",
                timing_correct_settings_text(settings),
                selected_field.label()
            ));
            ui.separator();
            if ui
                .add_enabled(timing_mode, egui::Button::new("TC <"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorLeft,
                });
            }
            if ui
                .add_enabled(timing_mode, egui::Button::new("TC >"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorRight,
                });
            }
            if ui
                .add_enabled(timing_mode, egui::Button::new("TC -"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: -1 });
            }
            if ui
                .add_enabled(timing_mode, egui::Button::new("TC +"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: 1 });
            }
        });
    }

    fn draw_setup_status(&mut self, ui: &mut egui::Ui) {
        let state = self.core.state();
        let setup_mode = state.mode == Mode::Setup;
        let preferences = state.setup_preferences;
        let selected_field = state.selected_setup_field;

        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "SETUP: {} field {}",
                setup_preferences_text(preferences),
                selected_field.label()
            ));
            ui.separator();
            if ui
                .add_enabled(setup_mode, egui::Button::new("Setup <"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorLeft,
                });
            }
            if ui
                .add_enabled(setup_mode, egui::Button::new("Setup >"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorRight,
                });
            }
            if ui
                .add_enabled(setup_mode, egui::Button::new("Setup -"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: -1 });
            }
            if ui
                .add_enabled(setup_mode, egui::Button::new("Setup +"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: 1 });
            }
        });
    }

    fn draw_sequence_status(&mut self, ui: &mut egui::Ui) {
        let state = self.core.state();
        let playing = state.playing;
        let recording = state.recording;
        let loop_enabled = state.loop_enabled;
        let sequence_length_ticks = state.sequence_length_ticks();
        let playhead_ticks = state.playhead_ticks;
        let recorded_event_count = state.recorded_events.len();
        let selected_track = state.selected_track;
        let selected_track_muted = state.is_track_muted(selected_track);
        let muted_track_count = state.muted_tracks.len();
        let can_erase = state.mode == Mode::Main;
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Transport: playing={} recording={}",
                playing, recording
            ));
            ui.separator();
            ui.label(format!(
                "Loop: {} length {} ticks",
                if loop_enabled { "on" } else { "off" },
                sequence_length_ticks
            ));
            ui.separator();
            ui.label(format!("Playhead: {} ticks", playhead_ticks));
            ui.separator();
            ui.label(format!("Recorded events: {}", recorded_event_count));
            ui.separator();
            ui.label(format!(
                "Track mute: Trk {selected_track:02} {} ({} muted)",
                if selected_track_muted {
                    "muted"
                } else {
                    "active"
                },
                muted_track_count
            ));
            ui.separator();
            if ui
                .add_enabled(can_erase, egui::Button::new("Erase last event"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::SoftKey(5),
                });
            }
        });
    }

    fn draw_program_status(&mut self, ui: &mut egui::Ui) {
        let state = self.core.state();
        let selected_pad = state.selected_program_pad;
        let program_text = format!(
            "Program: {:02} {}",
            state.current_program.index, state.current_program.name
        );
        let assignment_text = selected_assignment_text(state);
        let last_playback_text = last_playback_text(state);
        let show_program_actions = state.mode == Mode::Program;

        ui.horizontal_wrapped(|ui| {
            ui.label(program_text);
            ui.separator();
            ui.label(format!("Active bank: {}", state.pad_bank.label()));
            ui.separator();
            ui.label(format!("Selected pad: {}", program_pad_label(selected_pad)));
            ui.separator();
            ui.label(format!(
                "Edit field: {}",
                state.selected_program_edit_field.label()
            ));
            ui.separator();
            ui.label(assignment_text);
            ui.separator();
            ui.label(last_playback_text);
        });

        if show_program_actions {
            ui.horizontal(|ui| {
                if ui.button("F1 Clear selected pad").clicked() {
                    self.dispatch_event(HardwareEvent::Press {
                        control: PanelControl::SoftKey(1),
                    });
                }
                if ui.button("F2 Generate assignment").clicked() {
                    self.dispatch_event(HardwareEvent::Press {
                        control: PanelControl::SoftKey(2),
                    });
                }
            });
        }
    }

    fn draw_sample_status(&mut self, ui: &mut egui::Ui) {
        let state = self.core.state();
        let selected_sample = state.selected_sample();
        let sample_text = selected_sample_text(selected_sample.as_ref());
        let sample_mode = matches!(state.mode, Mode::Sample | Mode::Trim);
        let sample_create_mode = state.mode == Mode::Sample;
        let trim_mode = state.mode == Mode::Trim;
        let selected_trim_edit_field = state.selected_trim_edit_field;

        ui.horizontal_wrapped(|ui| {
            ui.label(sample_text);
            ui.separator();
            ui.label("Sample catalog: metadata only, no audio bytes");
            ui.separator();
            if ui
                .add_enabled(sample_mode, egui::Button::new("Prev sample"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::SoftKey(1),
                });
            }
            if ui
                .add_enabled(sample_mode, egui::Button::new("Next sample"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::SoftKey(2),
                });
            }
            if sample_create_mode {
                if ui.button("Record meta").clicked() {
                    self.dispatch_event(HardwareEvent::Press {
                        control: PanelControl::SoftKey(3),
                    });
                }
                if ui.button("Import meta").clicked() {
                    self.dispatch_event(HardwareEvent::Press {
                        control: PanelControl::SoftKey(4),
                    });
                }
            }
            ui.separator();
            ui.label(format!("TRIM field {}", selected_trim_edit_field.label()));
            if ui
                .add_enabled(trim_mode, egui::Button::new("Field <"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorLeft,
                });
            }
            if ui
                .add_enabled(trim_mode, egui::Button::new("Field >"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::CursorRight,
                });
            }
            if ui
                .add_enabled(trim_mode, egui::Button::new("Trim -"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: -1 });
            }
            if ui
                .add_enabled(trim_mode, egui::Button::new("Trim +"))
                .clicked()
            {
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta: 1 });
            }
        });
    }

    fn draw_audio_render_status(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(last_synthetic_render_text(
                self.last_synthetic_render.as_ref(),
                self.last_synthetic_render_error.as_deref(),
            ));
        });
    }

    fn draw_host_audio_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut enabled = self.host_audio.is_enabled();
            if ui.checkbox(&mut enabled, "Host audio").changed() {
                self.host_audio.set_enabled(enabled);
            }

            let state = self.host_audio.state();
            ui.separator();
            ui.label(host_audio_state_text(&state));
            ui.separator();
            ui.label(last_host_audio_event_text(state.last_event.as_ref()));
        });
    }

    fn draw_host_midi_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut enabled = self.host_midi.is_enabled();
            if ui.checkbox(&mut enabled, "Host MIDI").changed() {
                self.host_midi.set_enabled(enabled);
            }

            let state = self.host_midi.state();
            ui.separator();
            ui.label(host_midi_state_text(&state));
            ui.separator();
            ui.label(last_host_midi_event_text(state.last_event.as_ref()));
        });
    }

    fn draw_pads(&mut self, ui: &mut egui::Ui) {
        let active_bank = self.core.state().pad_bank;
        let selected_program_pad = self.core.state().selected_program_pad;
        let program_mode = self.core.state().mode == Mode::Program;
        ui.horizontal(|ui| {
            self.bank_button(ui, "A", PadBank::A, PanelControl::PadBankA);
            self.bank_button(ui, "B", PadBank::B, PanelControl::PadBankB);
            self.bank_button(ui, "C", PadBank::C, PanelControl::PadBankC);
            self.bank_button(ui, "D", PadBank::D, PanelControl::PadBankD);
        });
        ui.add_space(8.0);
        egui::Grid::new("pads")
            .num_columns(4)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                for pad in 1..=16 {
                    let pad_address = ProgramPad {
                        bank: active_bank,
                        pad_number: pad,
                    };
                    let selected = program_mode && selected_program_pad == pad_address;
                    if ui
                        .selectable_label(selected, program_pad_label(pad_address))
                        .clicked()
                    {
                        self.dispatch_event(HardwareEvent::StrikePad {
                            bank: active_bank,
                            pad,
                            velocity: 100,
                        });
                    }
                    if pad % 4 == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    fn bank_button(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        bank: PadBank,
        control: PanelControl,
    ) {
        let selected = self.core.state().pad_bank == bank;
        if ui.selectable_label(selected, label).clicked() {
            self.dispatch_event(HardwareEvent::Press { control });
        }
    }
}

fn main_screen_status(state: &MpcState) -> String {
    match state.mode {
        Mode::Main => format!(
            "LCD updated: {} focus, Seq {:02}, Trk {:02} {}, {} muted, {}, Tempo {}, Bars {:03}, Loop {}, Len {}, Tick {}, Events {}",
            state.selected_main_field.label(),
            state.sequence_index,
            state.selected_track,
            if state.is_track_muted(state.selected_track) {
                "muted"
            } else {
                "active"
            },
            state.muted_tracks.len(),
            state.current_program.name,
            tempo_text(state.tempo_bpm_x100),
            state.bar_count,
            if state.loop_enabled { "on" } else { "off" },
            state.sequence_length_ticks(),
            state.playhead_ticks,
            state.recorded_events.len()
        ),
        Mode::Program => format!(
            "LCD updated: PROGRAM bank {} {} field {}, {}",
            state.pad_bank.label(),
            program_pad_label(state.selected_program_pad),
            state.selected_program_edit_field.label(),
            selected_assignment_text(state)
        ),
        Mode::Sample | Mode::Trim => {
            let trim_field = if state.mode == Mode::Trim {
                format!(" field {}", state.selected_trim_edit_field.label())
            } else {
                String::new()
            };
            format!(
                "LCD updated: {:?}{}, {}",
                state.mode,
                trim_field,
                selected_sample_text(state.selected_sample().as_ref())
            )
        }
        Mode::Song => {
            let step = state.song_steps[state.selected_song_step_index];
            format!(
                "LCD updated: SONG step {:02}/{:02} field {}, Seq {:02}, repeats {:02}",
                state.selected_song_step_index + 1,
                state.song_steps.len(),
                state.selected_song_edit_field.label(),
                u16::from(step.sequence_index) + 1,
                step.repeats
            )
        }
        Mode::Midi => format!(
            "LCD updated: MIDI input {} base {} range {}",
            midi_input_channel_text(state.midi_input_channel),
            state.midi_base_note,
            midi_note_range_text(state.midi_base_note)
        ),
        Mode::TimingCorrect => format!(
            "LCD updated: TIMING field {}, {}",
            state.selected_timing_correct_field.label(),
            timing_correct_settings_text(state.timing_correct)
        ),
        Mode::Setup => format!(
            "LCD updated: SETUP field {}, {}",
            state.selected_setup_field.label(),
            setup_preferences_text(state.setup_preferences)
        ),
        mode => format!("LCD updated: {mode:?}"),
    }
}

fn tempo_text(tempo_bpm_x100: u32) -> String {
    format!("{}.{:02} BPM", tempo_bpm_x100 / 100, tempo_bpm_x100 % 100)
}

fn tune_text(tune_cents: i16) -> String {
    format!("{tune_cents:+04}c")
}

fn assignment_action_text(action: PadAssignmentChange) -> &'static str {
    match action {
        PadAssignmentChange::Cleared => "cleared",
        PadAssignmentChange::Restored => "assigned",
        PadAssignmentChange::Assigned => "assigned",
    }
}

fn program_pad_label(pad: ProgramPad) -> String {
    format!("{}{:02}", pad.bank.label(), pad.pad_number)
}

fn selected_assignment_text(state: &MpcState) -> String {
    let selected_pad = state.selected_program_pad;
    match state
        .current_program
        .pad_assignments
        .iter()
        .find(|assignment| assignment.pad == selected_pad)
    {
        Some(assignment) => format!(
            "Assignment: {} level {} pan {} tune {}",
            assignment.sample.name,
            assignment.level,
            assignment.pan,
            tune_text(assignment.tune_cents)
        ),
        None => "Assignment: unassigned".to_string(),
    }
}

fn selected_sample_text(selected_sample: Option<&SampleCatalogEntry>) -> String {
    match selected_sample {
        Some(entry) => format!(
            "Sample: {:02}/{:02} {} {} {} len {} frames trim {}..{} window {}",
            entry.index.min(99),
            entry.count.min(99),
            entry.sample.name,
            entry.source_kind.label(),
            program_pad_label(entry.source_pad),
            entry.length_frames,
            entry.start_frame,
            entry.end_frame,
            entry.window_length_frames
        ),
        None => "Sample: empty catalog".to_string(),
    }
}

fn midi_input_channel_text(input_channel: Option<u8>) -> String {
    match input_channel {
        Some(channel) => format!("Ch {channel:02}"),
        None => "Omni".to_string(),
    }
}

fn midi_note_range_text(base_note: u8) -> String {
    format!("{}..={}", base_note, base_note.saturating_add(15))
}

fn setup_preferences_text(preferences: SetupPreferences) -> String {
    format!(
        "metronome {} count-in {} bars contrast {}",
        if preferences.metronome_enabled {
            "on"
        } else {
            "off"
        },
        preferences.count_in_bars,
        preferences.lcd_contrast
    )
}

fn timing_correct_settings_text(settings: TimingCorrectSettings) -> String {
    let swing_scope = if settings.division.uses_swing() {
        "swing active"
    } else if settings.division.grid_ticks().is_some() {
        "triplet swing ignored"
    } else {
        "off"
    };
    format!(
        "division {} swing {}% ({swing_scope})",
        settings.division.label(),
        settings.swing_percent
    )
}

fn last_playback_text(state: &MpcState) -> String {
    match &state.last_playback {
        Some(resolution) => playback_resolution_text(resolution),
        None => "Last playback: none".to_string(),
    }
}

fn playback_resolution_text(resolution: &SamplePlaybackResolution) -> String {
    match resolution {
        SamplePlaybackResolution::Intent { intent } => format!(
            "Last playback: {} {} vel {} tune {} trim {}..{} window {}",
            program_pad_label(ProgramPad {
                bank: intent.bank,
                pad_number: intent.pad_number,
            }),
            intent.sample_name,
            intent.velocity,
            tune_text(intent.tune_cents),
            intent.start_frame,
            intent.end_frame,
            intent.window_length_frames
        ),
        SamplePlaybackResolution::Miss { miss } => format!(
            "Last playback: {} {:?}",
            program_pad_label(ProgramPad {
                bank: miss.bank,
                pad_number: miss.pad_number,
            }),
            miss.reason
        ),
    }
}

fn playback_intent_status_text(intent: &SamplePlaybackIntent) -> String {
    format!(
        "{:?}{:02} {} velocity {} tune {} trim {}..{} ({} frames)",
        intent.bank,
        intent.pad_number,
        intent.sample_name,
        intent.velocity,
        tune_text(intent.tune_cents),
        intent.start_frame,
        intent.end_frame,
        intent.window_length_frames
    )
}

fn playback_intent_from_outputs(outputs: &[MachineOutput]) -> Option<&SamplePlaybackIntent> {
    outputs.iter().find_map(|output| match output {
        MachineOutput::SamplePlaybackIntent { intent } => Some(intent),
        _ => None,
    })
}

fn last_synthetic_render_text(summary: Option<&AudioRenderSummary>, error: Option<&str>) -> String {
    match (summary, error) {
        (_, Some(error)) => error.to_string(),
        (Some(summary), None) => match summary.render_kind {
            AudioRenderKind::SamplePlayback => format!(
                "Synthetic render: {} {} frames @ {} Hz trim {}..{} window {} tune {} peak L{} R{} balance {:?}",
                summary.source_sample_name,
                summary.frame_count,
                summary.sample_rate_hz,
                summary.start_frame,
                summary.end_frame,
                summary.window_length_frames,
                tune_text(summary.tune_cents),
                summary.peak_left,
                summary.peak_right,
                summary.channel_balance
            ),
            AudioRenderKind::CountInClick => format!(
                "Synthetic render: {} {} frames @ {} Hz peak L{} R{}",
                count_in_click_summary_label(summary),
                summary.frame_count,
                summary.sample_rate_hz,
                summary.peak_left,
                summary.peak_right
            ),
        },
        (None, None) => "Synthetic render: none".to_string(),
    }
}

fn project_snapshot_status_text(
    status: &str,
    version: Option<u16>,
    byte_count: Option<usize>,
) -> String {
    match (version, byte_count) {
        (Some(version), Some(byte_count)) => {
            format!("Snapshot: v{version}, {byte_count} bytes, {status}")
        }
        _ => status.to_string(),
    }
}

fn project_file_status_text(
    status: &str,
    version: Option<u16>,
    byte_count: Option<usize>,
) -> String {
    match (version, byte_count) {
        (Some(version), Some(byte_count)) => {
            format!("Project file: v{version}, {byte_count} bytes, {status}")
        }
        _ => status.to_string(),
    }
}

fn host_audio_error_message(event: &HostAudioEvent) -> Option<String> {
    match event {
        HostAudioEvent::Failed { error, .. } => Some(format!("Host audio failed: {error}")),
        HostAudioEvent::Ignored { .. } | HostAudioEvent::Enqueued { .. } => None,
    }
}

fn host_midi_error_message(event: &HostMidiEvent) -> Option<String> {
    match event {
        HostMidiEvent::Failed { error, .. } => Some(format!("Host MIDI failed: {error}")),
        HostMidiEvent::Ignored { .. } | HostMidiEvent::Queued { .. } => None,
    }
}

fn host_audio_state_text(state: &HostAudioState) -> String {
    format!(
        "Host audio: {:?} backend {} queued {} played {} voices {}/{} done {} stolen {}",
        state.mode,
        state.backend_name,
        state.queued_render_count,
        state.played_render_count,
        state.active_voice_count,
        state.voice_limit,
        state.completed_voice_count,
        state.stolen_voice_count
    )
}

fn host_midi_state_text(state: &HostMidiState) -> String {
    format!(
        "Host MIDI: {:?} backend {} queued {} ignored {} failed {}",
        state.mode,
        state.backend_name,
        state.queued_message_count,
        state.ignored_message_count,
        state.failed_message_count
    )
}

fn last_host_audio_event_text(event: Option<&HostAudioEvent>) -> String {
    match event {
        Some(HostAudioEvent::Ignored { reason, .. }) => {
            format!("Host audio event: ignored {reason:?}")
        }
        Some(HostAudioEvent::Enqueued { receipt, .. }) => match receipt.summary.render_kind {
            AudioRenderKind::SamplePlayback => format!(
                "Host audio event: {} {} frames tune {} queued={} played={}",
                receipt.summary.source_sample_name,
                receipt.frame_count,
                tune_text(receipt.summary.tune_cents),
                receipt.queued,
                receipt.played
            ),
            AudioRenderKind::CountInClick => format!(
                "Host audio event: {} {} frames queued={} played={}",
                count_in_click_summary_label(&receipt.summary),
                receipt.frame_count,
                receipt.queued,
                receipt.played
            ),
        },
        Some(HostAudioEvent::Failed { error, summary, .. }) => match summary {
            Some(summary) => match summary.render_kind {
                AudioRenderKind::SamplePlayback => {
                    format!(
                        "Host audio event: failed {}: {error}",
                        summary.source_sample_name
                    )
                }
                AudioRenderKind::CountInClick => format!(
                    "Host audio event: failed {}: {error}",
                    count_in_click_summary_label(summary)
                ),
            },
            None => format!("Host audio event: failed no render: {error}"),
        },
        None => "Host audio event: none".to_string(),
    }
}

fn last_host_midi_event_text(event: Option<&HostMidiEvent>) -> String {
    match event {
        Some(HostMidiEvent::Queued { receipt }) => format!(
            "Host MIDI event: ch {} note {} vel {} from {:?}{:02}",
            receipt.message.channel,
            receipt.message.note,
            receipt.message.velocity,
            receipt.intent.bank,
            receipt.intent.pad_number
        ),
        Some(HostMidiEvent::Ignored { reason, intent }) => format!(
            "Host MIDI event: ignored {reason:?} ch {} note {}",
            intent.channel, intent.note
        ),
        Some(HostMidiEvent::Failed { error, intent }) => {
            format!(
                "Host MIDI event: failed ch {} note {}: {}",
                intent.channel, intent.note, error
            )
        }
        None => "Host MIDI event: none".to_string(),
    }
}

fn count_in_click_summary_label(summary: &AudioRenderSummary) -> String {
    let accent = if summary.accent.unwrap_or(false) {
        "count-in accent"
    } else {
        "count-in click"
    };
    format!(
        "{accent} bar {} beat {} tick {}",
        optional_u8_text(summary.bar_index),
        optional_u8_text(summary.beat_index),
        optional_u64_text(summary.count_in_tick)
    )
}

fn optional_u8_text(value: Option<u8>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn optional_u64_text(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string())
}
