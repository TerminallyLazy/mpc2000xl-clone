use eframe::egui;
use mpc_audio::{
    AudioOutputDeviceDescriptor, AudioRenderKind, AudioRenderSettings, AudioRenderSummary,
    CaptureAudioBackend, DeviceAudioBackend, DeviceAudioBackendConfig, DeviceAudioBackendStatus,
    HostAudioBackend, HostAudioBackendError, HostAudioEngine, HostAudioError, HostAudioEvent,
    HostAudioPlaybackReport, HostAudioState, RuntimeSampleLibrary, WavSampleLoadError,
    WavSamplePayload, list_output_devices, load_wav_sample_payload,
};
use mpc_core::{
    DiskOperation, HardwareEvent, MachineOutput, MidiSettingsField, Mode, MpcCore, MpcState,
    PadAssignmentChange, PadBank, PanelControl, ProgramPad, SampleCatalogEntry,
    SamplePlaybackIntent, SamplePlaybackResolution, SetupPreferences, TimingCorrectSettings,
};
use mpc_midi::{
    CaptureMidiBackend, DeviceMidiInputConfig, DeviceMidiInputConnection, DeviceMidiInputStatus,
    DeviceMidiOutputBackend, DeviceMidiOutputStatus, HostMidiBackend, HostMidiEngine,
    HostMidiEvent, HostMidiOutputReport, HostMidiState, MidiInputEvent, MidiPortDescriptor,
    OutboundMidiNoteScheduler, list_device_midi_input_ports, list_device_midi_output_ports,
};
use mpc_storage::{
    DEFAULT_PROJECT_FILE_PATH, load_project_file_with_report,
    save_project_file as save_project_file_to_path,
};
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_OUTBOUND_NOTE_DURATION_MILLIS: u64 = 250;
const MIN_OUTBOUND_NOTE_DURATION_MILLIS: u64 = 30;
const MAX_OUTBOUND_NOTE_DURATION_MILLIS: u64 = 4_000;

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
    host_audio: HostAudioEngine<DesktopAudioBackend>,
    host_midi: HostMidiEngine<DesktopMidiBackend>,
    outbound_midi_notes: OutboundMidiNoteScheduler,
    runtime_started_at: std::time::Instant,
    host_midi_input: Option<DeviceMidiInputConnection>,
    midi_input_ports: Vec<MidiPortDescriptor>,
    midi_output_ports: Vec<MidiPortDescriptor>,
    audio_output_devices: Vec<AudioOutputDeviceDescriptor>,
    selected_midi_input_port: usize,
    selected_midi_output_port: usize,
    selected_audio_output_device: usize,
    last_status: String,
    last_midi_note_off_status: String,
    last_audio_render: Option<AudioRenderSummary>,
    last_audio_render_error: Option<String>,
    runtime_samples: RuntimeSampleLibrary,
    runtime_sample_statuses: BTreeMap<String, RuntimeSampleStatus>,
    sample_import_path: String,
    last_runtime_sample_status: String,
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

enum DesktopAudioBackend {
    Capture(CaptureAudioBackend),
    Device {
        backend: DeviceAudioBackend,
        origin: DesktopAudioDeviceOrigin,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopAudioDeviceOrigin {
    Default,
    Selected,
}

impl DesktopAudioDeviceOrigin {
    fn label(self) -> &'static str {
        match self {
            Self::Default => "default device",
            Self::Selected => "selected device",
        }
    }
}

impl DesktopAudioBackend {
    fn capture() -> Self {
        Self::Capture(CaptureAudioBackend::new(8))
    }

    fn device_status(&self) -> Option<DeviceAudioBackendStatus> {
        match self {
            Self::Capture(_) => None,
            Self::Device { backend, .. } => Some(backend.status()),
        }
    }

    fn device_origin(&self) -> Option<DesktopAudioDeviceOrigin> {
        match self {
            Self::Capture(_) => None,
            Self::Device { origin, .. } => Some(*origin),
        }
    }

    fn is_default_device(&self) -> bool {
        matches!(
            self,
            Self::Device {
                origin: DesktopAudioDeviceOrigin::Default,
                ..
            }
        )
    }

    fn is_selected_device(&self) -> bool {
        matches!(
            self,
            Self::Device {
                origin: DesktopAudioDeviceOrigin::Selected,
                ..
            }
        )
    }

    fn is_device(&self) -> bool {
        match self {
            Self::Capture(_) => false,
            Self::Device { .. } => true,
        }
    }
}

impl HostAudioBackend for DesktopAudioBackend {
    fn backend_name(&self) -> &str {
        match self {
            Self::Capture(backend) => backend.backend_name(),
            Self::Device { backend, .. } => backend.backend_name(),
        }
    }

    fn enqueue_render(
        &mut self,
        rendered: &mpc_audio::RenderedAudio,
    ) -> Result<mpc_audio::HostAudioBackendReceipt, HostAudioBackendError> {
        match self {
            Self::Capture(backend) => backend.enqueue_render(rendered),
            Self::Device { backend, .. } => backend.enqueue_render(rendered),
        }
    }
}

enum DesktopMidiBackend {
    Capture(CaptureMidiBackend),
    Device(DeviceMidiOutputBackend),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DesktopMidiSendResult {
    Queued,
    Ignored { message: String },
    Failed { message: String },
}

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

impl DesktopMidiBackend {
    fn capture() -> Self {
        Self::Capture(CaptureMidiBackend::new(16))
    }

    fn device_output_status(&self) -> Option<DeviceMidiOutputStatus> {
        match self {
            Self::Capture(_) => None,
            Self::Device(backend) => Some(backend.status()),
        }
    }
}

impl HostMidiBackend for DesktopMidiBackend {
    fn name(&self) -> &'static str {
        match self {
            Self::Capture(backend) => backend.name(),
            Self::Device(backend) => backend.name(),
        }
    }

    fn mode(&self) -> mpc_midi::HostMidiMode {
        match self {
            Self::Capture(backend) => backend.mode(),
            Self::Device(backend) => backend.mode(),
        }
    }

    fn send(
        &mut self,
        message: mpc_midi::MidiMessage,
    ) -> Result<mpc_midi::HostMidiBackendReceipt, mpc_midi::HostMidiError> {
        match self {
            Self::Capture(backend) => backend.send(message),
            Self::Device(backend) => backend.send(message),
        }
    }
}

impl Default for MpcDesktopApp {
    fn default() -> Self {
        Self {
            core: MpcCore::new(),
            host_audio: HostAudioEngine::new(
                DesktopAudioBackend::capture(),
                AudioRenderSettings::preview(),
            )
            .expect("desktop host audio preview settings should satisfy guardrails"),
            host_midi: HostMidiEngine::enabled(DesktopMidiBackend::capture()),
            outbound_midi_notes: OutboundMidiNoteScheduler::default(),
            runtime_started_at: std::time::Instant::now(),
            host_midi_input: None,
            midi_input_ports: Vec::new(),
            midi_output_ports: Vec::new(),
            audio_output_devices: Vec::new(),
            selected_midi_input_port: 0,
            selected_midi_output_port: 0,
            selected_audio_output_device: 0,
            last_status: "Ready".to_string(),
            last_midi_note_off_status: "MIDI note-off: none pending".to_string(),
            last_audio_render: None,
            last_audio_render_error: None,
            runtime_samples: RuntimeSampleLibrary::default(),
            runtime_sample_statuses: BTreeMap::new(),
            sample_import_path: "local-assets/samples/import.wav".to_string(),
            last_runtime_sample_status: "Runtime WAV: none".to_string(),
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
        self.poll_host_midi_input();
        if let Some(error) = self.flush_due_midi_note_offs() {
            self.last_status = error;
        }
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
        if self.host_midi_input.is_some()
            || (self.host_midi.is_enabled() && self.outbound_midi_notes.has_pending())
        {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(10));
        }
    }
}

impl MpcDesktopApp {
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
            .clamp(
                MIN_OUTBOUND_NOTE_DURATION_MILLIS,
                MAX_OUTBOUND_NOTE_DURATION_MILLIS,
            )
    }

    fn send_midi_intent(&mut self, intent: &mpc_core::MidiOutputIntent) -> DesktopMidiSendResult {
        let report = self.host_midi.send_intent(intent);
        let result = match &report.event {
            HostMidiEvent::Queued { .. } => DesktopMidiSendResult::Queued,
            HostMidiEvent::Ignored { reason, intent } => DesktopMidiSendResult::Ignored {
                message: format!(
                    "Host MIDI ignored {:?} ch {} note {}: {}",
                    intent.kind, intent.channel, intent.note, reason
                ),
            },
            HostMidiEvent::Failed { error, intent } => DesktopMidiSendResult::Failed {
                message: format!(
                    "Host MIDI failed {:?} ch {} note {}: {}",
                    intent.kind, intent.channel, intent.note, error
                ),
            },
        };
        let _ = self.record_midi_report(report);

        if intent.kind == mpc_core::MidiOutputIntentKind::NoteOff {
            self.last_midi_note_off_status = match &result {
                DesktopMidiSendResult::Queued => format!(
                    "MIDI note-off sent ch {} note {} {:?}{:02}",
                    intent.channel, intent.note, intent.bank, intent.pad_number
                ),
                DesktopMidiSendResult::Ignored { message }
                | DesktopMidiSendResult::Failed { message } => {
                    format!("MIDI note-off not sent; {message}")
                }
            };
        }

        result
    }

    fn flush_due_midi_note_offs(&mut self) -> Option<String> {
        let now = self.runtime_millis();
        if !self.host_midi.is_enabled() {
            return None;
        }

        self.flush_midi_note_offs_due_by(now, now, "due")
    }

    fn flush_all_pending_midi_note_offs(&mut self, reason: &str) -> Option<String> {
        if !self.host_midi.is_enabled() {
            return None;
        }

        let now = self.runtime_millis();
        self.flush_midi_note_offs_due_by(u64::MAX, now, reason)
    }

    fn flush_midi_note_offs_due_by(
        &mut self,
        due_by_millis: u64,
        retry_base_millis: u64,
        reason: &str,
    ) -> Option<String> {
        let due = self.outbound_midi_notes.drain_due_note_offs(due_by_millis);
        if due.is_empty() {
            return None;
        }

        let due_count = due.len();
        let mut queued_count = 0usize;
        let mut ignored_count = 0usize;
        let mut failed_count = 0usize;
        let mut last_failure = None;
        for intent in due {
            match self.send_midi_intent(&intent) {
                DesktopMidiSendResult::Queued => {
                    queued_count += 1;
                }
                DesktopMidiSendResult::Ignored { .. } => {
                    ignored_count += 1;
                    self.requeue_note_off_retry(&intent, retry_base_millis);
                }
                DesktopMidiSendResult::Failed { message } => {
                    failed_count += 1;
                    last_failure = Some(message);
                    self.requeue_note_off_retry(&intent, retry_base_millis);
                }
            }
        }

        if ignored_count == 0 && failed_count == 0 {
            self.last_midi_note_off_status =
                format!("MIDI note-off sent {queued_count} {reason} note-off(s)");
            return None;
        }

        self.last_midi_note_off_status = format!(
            "MIDI note-off pending: sent {queued_count}/{due_count} {reason} note-off(s); {ignored_count} ignored, {failed_count} failed"
        );
        last_failure.or_else(|| Some(self.last_midi_note_off_status.clone()))
    }

    fn requeue_note_off_retry(&mut self, intent: &mpc_core::MidiOutputIntent, now_millis: u64) {
        let mut retry_intent = intent.clone();
        retry_intent.kind = mpc_core::MidiOutputIntentKind::NoteOn;
        let _ = self.outbound_midi_notes.register_note_on(
            &retry_intent,
            now_millis,
            MIN_OUTBOUND_NOTE_DURATION_MILLIS,
        );
    }

    fn flush_pending_before_midi_backend_change(&mut self) -> Result<(), String> {
        if !self.outbound_midi_notes.has_pending() {
            return Ok(());
        }

        if !self.host_midi.is_enabled() {
            let message =
                "MIDI output switch blocked: pending note-offs remain while Host MIDI is disabled"
                    .to_string();
            self.last_midi_note_off_status = message.clone();
            return Err(message);
        }

        if let Some(message) = self.flush_all_pending_midi_note_offs("before switching MIDI output")
        {
            return Err(message);
        }

        if self.outbound_midi_notes.has_pending() {
            let message = "MIDI output switch blocked: pending note-offs were not sent".to_string();
            self.last_midi_note_off_status = message.clone();
            return Err(message);
        }

        Ok(())
    }

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
                        self.relink_runtime_samples_from_project("snapshot load");
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
                        self.relink_runtime_samples_from_project("project file load");
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
        self.prune_runtime_samples_to_project_metadata();
        self.last_status = disk_operation_status
            .or(midi_host_error)
            .or(render_or_host_error)
            .unwrap_or_else(|| Self::status_from_outputs(&outputs, self.core.state()));
    }

    fn poll_host_midi_input(&mut self) {
        let Some(input) = self.host_midi_input.as_mut() else {
            return;
        };
        let events = match input.drain_events() {
            Ok(events) => events,
            Err(error) => {
                self.last_status = format!("Host MIDI input failed: {error}");
                return;
            }
        };

        for event in events {
            match event {
                MidiInputEvent::NoteOn {
                    channel,
                    note,
                    velocity,
                } => {
                    self.dispatch_event(HardwareEvent::MidiNoteOn {
                        channel,
                        note,
                        velocity,
                    });
                }
                MidiInputEvent::NoteOff {
                    channel,
                    note,
                    velocity,
                } => {
                    self.dispatch_event(HardwareEvent::MidiNoteOff {
                        channel,
                        note,
                        velocity,
                    });
                }
            }
        }
    }

    fn refresh_midi_device_ports(&mut self) {
        let input_result = list_device_midi_input_ports();
        let output_result = list_device_midi_output_ports();

        let mut status_parts = Vec::new();
        match input_result {
            Ok(ports) => {
                self.midi_input_ports = ports;
                self.selected_midi_input_port =
                    clamp_port_index(self.selected_midi_input_port, self.midi_input_ports.len());
                status_parts.push(format!("{} MIDI input(s)", self.midi_input_ports.len()));
            }
            Err(error) => {
                self.midi_input_ports.clear();
                self.selected_midi_input_port = 0;
                status_parts.push(format!("MIDI input refresh failed: {error}"));
            }
        }

        match output_result {
            Ok(ports) => {
                self.midi_output_ports = ports;
                self.selected_midi_output_port =
                    clamp_port_index(self.selected_midi_output_port, self.midi_output_ports.len());
                status_parts.push(format!("{} MIDI output(s)", self.midi_output_ports.len()));
            }
            Err(error) => {
                self.midi_output_ports.clear();
                self.selected_midi_output_port = 0;
                status_parts.push(format!("MIDI output refresh failed: {error}"));
            }
        }

        self.last_status = format!("MIDI devices: {}", status_parts.join(", "));
    }

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

    fn switch_host_midi_to_capture(&mut self) {
        if matches!(self.host_midi.backend(), DesktopMidiBackend::Capture(_)) {
            return;
        }
        self.replace_host_midi_backend(
            DesktopMidiBackend::capture(),
            "Host MIDI output backend: capture".to_string(),
        );
    }

    fn switch_host_midi_to_device_output(&mut self) {
        let Some(port) = self
            .midi_output_ports
            .get(self.selected_midi_output_port)
            .cloned()
        else {
            self.last_status = "No MIDI output port selected".to_string();
            return;
        };

        match DeviceMidiOutputBackend::connect_output_port_id(&port.id) {
            Ok(backend) => {
                let status = backend.status();
                self.replace_host_midi_backend(
                    DesktopMidiBackend::Device(backend),
                    format!(
                        "Host MIDI output backend: device {}",
                        midi_port_label(&status.output_port)
                    ),
                );
            }
            Err(error) => {
                self.last_status = format!("Host MIDI output unavailable: {error}");
            }
        }
    }

    fn replace_host_midi_backend(&mut self, backend: DesktopMidiBackend, status: String) {
        if let Err(message) = self.flush_pending_before_midi_backend_change() {
            self.last_status = message;
            return;
        }

        let enabled = self.host_midi.is_enabled();
        let mut host_midi = HostMidiEngine::new(backend);
        host_midi.set_enabled(enabled);
        self.host_midi = host_midi;
        self.last_status = status;
    }

    fn connect_host_midi_input(&mut self) {
        let Some(port) = self
            .midi_input_ports
            .get(self.selected_midi_input_port)
            .cloned()
        else {
            self.last_status = "No MIDI input port selected".to_string();
            return;
        };

        match DeviceMidiInputConnection::connect_input_port_id(
            &port.id,
            DeviceMidiInputConfig::default(),
        ) {
            Ok(connection) => {
                let status = connection.status();
                self.host_midi_input = Some(connection);
                self.last_status = format!(
                    "Host MIDI input connected: {}",
                    midi_port_label(&status.input_port)
                );
            }
            Err(error) => {
                self.last_status = format!("Host MIDI input unavailable: {error}");
            }
        }
    }

    fn disconnect_host_midi_input(&mut self) {
        self.host_midi_input = None;
        self.last_status = "Host MIDI input disconnected".to_string();
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
                    let report = self
                        .host_audio
                        .play_intent_with_runtime_samples_and_render_summary(
                            intent,
                            &self.runtime_samples,
                        );
                    if let Some(message) = self.record_audio_report(report, "Sample render failed")
                    {
                        playback_error = Some(message);
                    }
                }
                MachineOutput::SampleReleaseIntent { intent } => {
                    let event = self.host_audio.release_intent(intent);
                    if let Some(message) = host_audio_error_message(&event) {
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
        self.last_audio_render = report.render_summary;
        self.last_audio_render_error = None;

        if let HostAudioEvent::Failed {
            error: HostAudioError::Render { error },
            ..
        } = &report.event
        {
            let message = format!("{render_error_prefix}: {error}");
            self.last_audio_render_error = Some(message.clone());
            return Some(message);
        }

        host_audio_error_message(&report.event)
    }

    fn handle_midi_outputs(&mut self, outputs: &[MachineOutput]) -> Option<String> {
        let mut midi_error = None;

        for output in outputs {
            if let MachineOutput::MidiOutputIntent { intent } = output {
                match self.send_midi_intent(intent) {
                    DesktopMidiSendResult::Queued
                        if intent.kind == mpc_core::MidiOutputIntentKind::NoteOn =>
                    {
                        let duration = self.outbound_note_duration_millis(intent);
                        let displaced_note_offs = self.outbound_midi_notes.register_note_on(
                            intent,
                            self.runtime_millis(),
                            duration,
                        );
                        let displaced_count = displaced_note_offs.len();
                        let mut displaced_queued_count = 0usize;
                        let mut displaced_ignored_count = 0usize;
                        let mut displaced_failed_count = 0usize;
                        for note_off in displaced_note_offs {
                            match self.send_midi_intent(&note_off) {
                                DesktopMidiSendResult::Queued => {
                                    displaced_queued_count += 1;
                                }
                                DesktopMidiSendResult::Ignored { .. } => {
                                    displaced_ignored_count += 1;
                                }
                                DesktopMidiSendResult::Failed { message } => {
                                    displaced_failed_count += 1;
                                    midi_error = Some(message);
                                }
                            }
                        }

                        self.last_midi_note_off_status = match (
                            displaced_count,
                            displaced_ignored_count,
                            displaced_failed_count,
                        ) {
                            (0, _, _) => format!(
                                "MIDI note-off scheduled ch {} note {} in {} ms",
                                intent.channel, intent.note, duration
                            ),
                            (_, 0, 0) => format!(
                                "MIDI note-off scheduled ch {} note {} in {} ms; sent {} displaced note-off(s)",
                                intent.channel, intent.note, duration, displaced_queued_count
                            ),
                            _ => format!(
                                "MIDI note-off scheduled ch {} note {} in {} ms; sent {}/{} displaced note-off(s); {} ignored, {} failed",
                                intent.channel,
                                intent.note,
                                duration,
                                displaced_queued_count,
                                displaced_count,
                                displaced_ignored_count,
                                displaced_failed_count
                            ),
                        };
                    }
                    DesktopMidiSendResult::Failed { message } => {
                        midi_error = Some(message);
                    }
                    DesktopMidiSendResult::Queued | DesktopMidiSendResult::Ignored { .. } => {}
                }
            }
        }

        midi_error
    }

    fn record_midi_report(&mut self, report: HostMidiOutputReport) -> Option<String> {
        host_midi_error_message(&report.event)
    }

    fn relink_runtime_samples_from_project(&mut self, reason: &str) {
        self.clear_runtime_sample_payloads(reason);
        let references = self.core.state().imported_media_references.clone();

        for reference in references {
            let _ = self.load_runtime_sample_reference(&reference);
        }

        self.last_audio_render = None;
        self.last_audio_render_error = None;
        self.last_runtime_sample_status =
            runtime_sample_relink_status_text(reason, &self.runtime_sample_statuses);
    }

    fn load_runtime_sample_reference(
        &mut self,
        reference: &mpc_core::ProjectImportedMediaReference,
    ) -> Result<(), ()> {
        let mut attempted_paths = Vec::new();
        let mut load_failed_status = None;
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
                    if !wav_sample_load_error_is_not_found(&error) {
                        load_failed_status = Some(RuntimeSampleStatus::LoadFailed {
                            path,
                            message: error.to_string(),
                        });
                    }
                }
            }
        }

        let status = load_failed_status.unwrap_or(RuntimeSampleStatus::Missing { attempted_paths });
        self.runtime_sample_statuses
            .insert(reference.sample_id.clone(), status);
        Err(())
    }

    fn load_wav_to_selected_pad(&mut self) {
        let path = self.sample_import_path.trim();
        if path.is_empty() {
            let message = "Runtime WAV import failed: path is empty".to_string();
            self.last_runtime_sample_status = message.clone();
            self.last_status = message;
            return;
        }

        let payload = match load_wav_sample_payload(path) {
            Ok(payload) => payload,
            Err(error) => {
                let message = format!("Runtime WAV import failed: {error}");
                self.last_runtime_sample_status = message.clone();
                self.last_status = message;
                return;
            }
        };
        let sample_name = sample_name_from_path(path);
        let outputs = self
            .core
            .import_sample_metadata_for_selected_pad(sample_name, payload.length_frames_u32());
        let created = outputs.iter().find_map(|output| match output {
            MachineOutput::SampleMetadataCreated {
                sample,
                source_kind,
                length_frames,
                ..
            } if *source_kind == mpc_core::SampleSourceKind::Imported => {
                Some((sample.clone(), *length_frames))
            }
            _ => None,
        });

        if let Some((sample, length_frames)) = created {
            let byte_count = payload.byte_count;
            let sample_rate_hz = payload.sample_rate_hz;
            self.runtime_samples
                .insert(sample.id.clone(), sample.name.clone(), payload);
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
            self.prune_runtime_samples_to_project_metadata();
            self.last_runtime_sample_status = format!(
                "Runtime WAV loaded: {} {} frames @ {} Hz ({} bytes)",
                sample.name, length_frames, sample_rate_hz, byte_count
            );
            self.last_status = self.last_runtime_sample_status.clone();
        } else {
            self.last_status = Self::status_from_outputs(&outputs, self.core.state());
            self.last_runtime_sample_status = self.last_status.clone();
        }
    }

    fn clear_runtime_sample_payloads(&mut self, reason: &str) {
        self.runtime_samples.clear();
        self.runtime_sample_statuses.clear();
        self.last_audio_render = None;
        self.last_audio_render_error = None;
        self.last_runtime_sample_status = format!("Runtime WAV: cleared after {reason}");
    }

    fn prune_runtime_samples_to_project_metadata(&mut self) {
        let retained_sample_ids = runtime_sample_ids_referenced_by_project(self.core.state());
        self.runtime_sample_statuses
            .retain(|sample_id, _| retained_sample_ids.contains(sample_id));
        self.runtime_samples
            .retain(|sample_id, _| retained_sample_ids.contains(sample_id));
        if self.runtime_samples.is_empty() && self.runtime_sample_statuses.is_empty() {
            self.last_runtime_sample_status =
                "Runtime WAV: none referenced by current project metadata".to_string();
        }
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

        if let Some(MachineOutput::SampleReleaseIntent { intent }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SampleReleaseIntent { .. }))
        {
            return format!(
                "Release intent Trk {:02} {:?}{:02} {} vel {}",
                intent.selected_track,
                intent.bank,
                intent.pad_number,
                intent.sample_name,
                intent.release_velocity
            );
        }

        if let Some(MachineOutput::MidiOutputIntent { intent }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::MidiOutputIntent { .. }))
        {
            return format!(
                "MIDI out {:?} ch {} note {} vel {} from {:?}{:02} {}",
                intent.kind,
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
        let host_midi_backend_name = self.host_midi.state().backend_name;
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
                "MIDI settings: input {} base {} range {} field {} host output {}",
                midi_input_channel_text(midi_input_channel),
                midi_base_note,
                midi_note_range_text(midi_base_note),
                selected_midi_settings_field.label(),
                host_midi_backend_name
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
            ui.label("Samples: project metadata only; WAV payloads stay runtime-only");
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
                ui.separator();
                ui.label("WAV");
                ui.add(
                    egui::TextEdit::singleline(&mut self.sample_import_path).desired_width(300.0),
                );
                if ui.button("Load WAV to pad").clicked() {
                    self.load_wav_to_selected_pad();
                }
            }
            ui.separator();
            ui.label(&self.last_runtime_sample_status);
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
            ui.label(last_audio_render_text(
                self.last_audio_render.as_ref(),
                self.last_audio_render_error.as_deref(),
            ));
        });
    }

    fn draw_host_audio_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut enabled = self.host_audio.is_enabled();
            if ui.checkbox(&mut enabled, "Host audio").changed() {
                self.host_audio.set_enabled(enabled);
            }

            ui.separator();
            let capture_selected = !self.host_audio.backend().is_device();
            if ui.selectable_label(capture_selected, "Capture").clicked() {
                self.switch_host_audio_to_capture();
            }
            if ui
                .selectable_label(
                    self.host_audio.backend().is_default_device(),
                    "Default device",
                )
                .clicked()
            {
                self.switch_host_audio_to_default_device();
            }
            if ui.button("Refresh audio").clicked() {
                self.refresh_audio_output_devices();
            }
            audio_output_device_combo(
                ui,
                &self.audio_output_devices,
                &mut self.selected_audio_output_device,
            );
            ui.add_enabled_ui(!self.audio_output_devices.is_empty(), |ui| {
                if ui
                    .selectable_label(
                        self.host_audio.backend().is_selected_device(),
                        "Selected device",
                    )
                    .clicked()
                {
                    self.switch_host_audio_to_selected_device();
                }
            });

            let state = self.host_audio.state();
            ui.separator();
            ui.label(host_audio_state_text(&state));
            ui.separator();
            ui.label(last_host_audio_event_text(state.last_event.as_ref()));
            ui.separator();
            ui.label(host_audio_backend_detail_text(self.host_audio.backend()));
        });
    }

    fn draw_host_midi_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut enabled = self.host_midi.is_enabled();
            if ui.checkbox(&mut enabled, "Host MIDI").changed() {
                if self.host_midi.is_enabled() && !enabled && self.outbound_midi_notes.has_pending()
                {
                    if let Some(message) =
                        self.flush_all_pending_midi_note_offs("before disabling Host MIDI")
                    {
                        self.last_status = message;
                    }
                }
                self.host_midi.set_enabled(enabled);
                if !enabled && self.outbound_midi_notes.has_pending() {
                    self.last_midi_note_off_status =
                        "MIDI note-off pending: Host MIDI disabled before all note-offs were sent"
                            .to_string();
                }
            }

            ui.separator();
            let capture_selected =
                matches!(self.host_midi.backend(), DesktopMidiBackend::Capture(_));
            if ui
                .selectable_label(capture_selected, "Capture out")
                .clicked()
            {
                self.switch_host_midi_to_capture();
            }
            let device_selected = matches!(self.host_midi.backend(), DesktopMidiBackend::Device(_));
            ui.add_enabled_ui(!self.midi_output_ports.is_empty(), |ui| {
                if ui.selectable_label(device_selected, "Device out").clicked() {
                    self.switch_host_midi_to_device_output();
                }
            });
            if ui.button("Refresh MIDI").clicked() {
                self.refresh_midi_device_ports();
            }

            ui.separator();
            midi_port_combo(
                ui,
                "MIDI in",
                &self.midi_input_ports,
                &mut self.selected_midi_input_port,
            );
            if ui
                .add_enabled(
                    !self.midi_input_ports.is_empty(),
                    egui::Button::new("Connect in"),
                )
                .clicked()
            {
                self.connect_host_midi_input();
            }
            if ui
                .add_enabled(
                    self.host_midi_input.is_some(),
                    egui::Button::new("Disconnect in"),
                )
                .clicked()
            {
                self.disconnect_host_midi_input();
            }

            midi_port_combo(
                ui,
                "MIDI out",
                &self.midi_output_ports,
                &mut self.selected_midi_output_port,
            );

            let state = self.host_midi.state();
            ui.separator();
            ui.label(host_midi_state_text(&state));
            ui.separator();
            if ui.button("MIDI panic").clicked() {
                self.outbound_midi_notes.clear();
                self.last_midi_note_off_status = "MIDI panic: pending notes cleared".to_string();
                self.last_status = self.last_midi_note_off_status.clone();
            }
            ui.separator();
            ui.label(&self.last_midi_note_off_status);
            ui.separator();
            ui.label(last_host_midi_event_text(state.last_event.as_ref()));
            ui.separator();
            ui.label(host_midi_backend_detail_text(self.host_midi.backend()));
            ui.separator();
            ui.label(host_midi_input_status_text(self.host_midi_input.as_ref()));
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

    fn switch_host_audio_to_capture(&mut self) {
        if matches!(self.host_audio.backend(), DesktopAudioBackend::Capture(_)) {
            return;
        }

        self.replace_host_audio_backend(
            DesktopAudioBackend::capture(),
            self.host_audio.render_settings(),
            "Host audio backend: capture".to_string(),
        );
    }

    fn switch_host_audio_to_default_device(&mut self) {
        if self.host_audio.backend().is_default_device() {
            return;
        }

        match DeviceAudioBackend::open_default(DeviceAudioBackendConfig::default()) {
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
                    DesktopAudioBackend::Device {
                        backend,
                        origin: DesktopAudioDeviceOrigin::Default,
                    },
                    device_render_settings,
                    format!(
                        "Host audio backend: default device {} {} Hz {} ch {}",
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
                    DesktopAudioBackend::Device {
                        backend,
                        origin: DesktopAudioDeviceOrigin::Selected,
                    },
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

    fn replace_host_audio_backend(
        &mut self,
        backend: DesktopAudioBackend,
        render_settings: AudioRenderSettings,
        status: String,
    ) {
        let enabled = self.host_audio.is_enabled();
        let mut host_audio = HostAudioEngine::new(backend, render_settings)
            .expect("desktop host audio render settings should remain valid");
        host_audio.set_enabled(enabled);
        self.host_audio = host_audio;
        self.last_status = status;
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

fn mute_group_text(mute_group: u8) -> String {
    if mute_group == 0 {
        "off".to_string()
    } else {
        format!("{mute_group:02}")
    }
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
            "Assignment: {} level {} pan {} tune {} mute group {}",
            assignment.sample.name,
            assignment.level,
            assignment.pan,
            tune_text(assignment.tune_cents),
            mute_group_text(assignment.mute_group)
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
            "Last playback: {} {} vel {} tune {} mute group {} trim {}..{} window {}",
            program_pad_label(ProgramPad {
                bank: intent.bank,
                pad_number: intent.pad_number,
            }),
            intent.sample_name,
            intent.velocity,
            tune_text(intent.tune_cents),
            mute_group_text(intent.mute_group),
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
        "{:?}{:02} {} velocity {} tune {} mute group {} trim {}..{} ({} frames)",
        intent.bank,
        intent.pad_number,
        intent.sample_name,
        intent.velocity,
        tune_text(intent.tune_cents),
        mute_group_text(intent.mute_group),
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

fn last_audio_render_text(summary: Option<&AudioRenderSummary>, error: Option<&str>) -> String {
    match (summary, error) {
        (_, Some(error)) => error.to_string(),
        (Some(summary), None) => match summary.render_kind {
            AudioRenderKind::SamplePlayback => format!(
                "{}: {} {} frames @ {} Hz trim {}..{} window {} tune {} mute group {} peak L{} R{} balance {:?} loaded {} bytes",
                audio_source_label(summary),
                summary.source_sample_name,
                summary.frame_count,
                summary.sample_rate_hz,
                summary.start_frame,
                summary.end_frame,
                summary.window_length_frames,
                tune_text(summary.tune_cents),
                mute_group_text(summary.mute_group),
                summary.peak_left,
                summary.peak_right,
                summary.channel_balance,
                summary.loaded_audio_byte_count
            ),
            AudioRenderKind::CountInClick => format!(
                "{}: {} {} frames @ {} Hz peak L{} R{}",
                audio_source_label(summary),
                count_in_click_summary_label(summary),
                summary.frame_count,
                summary.sample_rate_hz,
                summary.peak_left,
                summary.peak_right
            ),
        },
        (None, None) => "Audio render: none".to_string(),
    }
}

fn audio_source_label(summary: &AudioRenderSummary) -> &'static str {
    match summary.source_kind {
        mpc_audio::AudioSourceKind::RightsSafeGenerated => "Generated render",
        mpc_audio::AudioSourceKind::RuntimeUserWav => "Runtime WAV render",
    }
}

fn sample_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("USER-WAV")
        .to_string()
}

fn runtime_sample_ids_referenced_by_project(state: &MpcState) -> BTreeSet<String> {
    let mut sample_ids = BTreeSet::new();
    sample_ids.extend(
        state
            .current_program
            .pad_assignments
            .iter()
            .map(|assignment| assignment.sample.id.clone()),
    );
    sample_ids.extend(state.sample_trims.iter().map(|trim| trim.sample_id.clone()));
    if let Some(sample_id) = &state.selected_sample_id {
        sample_ids.insert(sample_id.clone());
    }
    for event in &state.recorded_events {
        if let Some(playback) = &event.playback {
            sample_ids.insert(playback.sample_id.clone());
        }
    }
    if let Some(SamplePlaybackResolution::Intent { intent }) = &state.last_playback {
        sample_ids.insert(intent.sample_id.clone());
    }
    sample_ids
}

fn media_reference_candidate_paths(
    reference: &mpc_core::ProjectImportedMediaReference,
) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(path) = &reference.managed_copy_path {
        paths.push(path.clone());
    }
    if !reference.source_path.is_empty() && !paths.iter().any(|path| path == &reference.source_path)
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

fn runtime_sample_relink_status_text(
    reason: &str,
    statuses: &BTreeMap<String, RuntimeSampleStatus>,
) -> String {
    let mut loaded_count = 0_usize;
    let mut missing_count = 0_usize;
    let mut mismatch_count = 0_usize;
    let mut failed_count = 0_usize;
    let mut first_issue = None;

    for (sample_id, status) in statuses {
        match status {
            RuntimeSampleStatus::Loaded { .. } => {
                loaded_count = loaded_count.saturating_add(1);
            }
            RuntimeSampleStatus::Missing { .. } => {
                missing_count = missing_count.saturating_add(1);
            }
            RuntimeSampleStatus::MetadataMismatch { .. } => {
                mismatch_count = mismatch_count.saturating_add(1);
            }
            RuntimeSampleStatus::LoadFailed { .. } => {
                failed_count = failed_count.saturating_add(1);
            }
        }

        if first_issue.is_none() {
            first_issue = runtime_sample_actionable_issue_text(sample_id, status);
        }
    }

    let mut text = format!(
        "Runtime WAV: relink after {reason}: {loaded_count} loaded, {missing_count} missing, {mismatch_count} mismatch, {failed_count} failed"
    );
    if let Some(issue) = first_issue {
        text.push_str("; ");
        text.push_str(&issue);
    }
    text
}

fn runtime_sample_actionable_issue_text(
    sample_id: &str,
    status: &RuntimeSampleStatus,
) -> Option<String> {
    match status {
        RuntimeSampleStatus::MetadataMismatch {
            path,
            expected_sample_rate_hz,
            actual_sample_rate_hz,
            expected_frame_count,
            actual_frame_count,
        } => Some(format!(
            "first issue: {sample_id} metadata mismatch at {path}: expected {expected_sample_rate_hz} Hz/{expected_frame_count} frames, got {actual_sample_rate_hz} Hz/{actual_frame_count} frames"
        )),
        RuntimeSampleStatus::LoadFailed { path, message } => Some(format!(
            "first issue: {sample_id} load failed at {path}: {message}"
        )),
        RuntimeSampleStatus::Loaded { .. } | RuntimeSampleStatus::Missing { .. } => None,
    }
}

fn wav_sample_load_error_is_not_found(error: &WavSampleLoadError) -> bool {
    let message = match error {
        WavSampleLoadError::Metadata { message, .. } | WavSampleLoadError::Open { message, .. } => {
            message
        }
        _ => return false,
    };
    let message = message.to_ascii_lowercase();
    message.contains("os error 2")
        || message.contains("no such file")
        || message.contains("not found")
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
        HostAudioEvent::Ignored { .. }
        | HostAudioEvent::Enqueued { .. }
        | HostAudioEvent::Released { .. } => None,
    }
}

fn host_midi_error_message(event: &HostMidiEvent) -> Option<String> {
    match event {
        HostMidiEvent::Failed { error, intent } => Some(format!(
            "Host MIDI failed {:?} ch {} note {}: {error}",
            intent.kind, intent.channel, intent.note
        )),
        HostMidiEvent::Ignored { .. } | HostMidiEvent::Queued { .. } => None,
    }
}

fn host_audio_state_text(state: &HostAudioState) -> String {
    format!(
        "Host audio: {:?} backend {} queued {} played {} voices {}/{} done {} released {} choked {} stolen {}",
        state.mode,
        state.backend_name,
        state.queued_render_count,
        state.played_render_count,
        state.active_voice_count,
        state.voice_limit,
        state.completed_voice_count,
        state.released_voice_count,
        state.choked_voice_count,
        state.stolen_voice_count
    )
}

fn host_audio_backend_detail_text(backend: &DesktopAudioBackend) -> String {
    match (backend.device_origin(), backend.device_status()) {
        (Some(origin), Some(status)) => device_audio_backend_status_text(origin, &status),
        _ => "Host audio backend detail: capture retains summaries only".to_string(),
    }
}

fn device_audio_backend_status_text(
    origin: DesktopAudioDeviceOrigin,
    status: &DeviceAudioBackendStatus,
) -> String {
    let stream_errors = status.recent_stream_errors.len();
    format!(
        "Host audio {}: {} {} Hz {} ch {} queued {}/{} cb {} underrun {} errors {}",
        origin.label(),
        status.device_name,
        status.sample_rate_hz,
        status.channels,
        status.sample_format,
        status.queued_frame_count,
        status.max_queued_frame_count,
        status.total_callback_frame_count,
        status.underrun_frame_count,
        stream_errors
    )
}

fn midi_port_combo(
    ui: &mut egui::Ui,
    label: &str,
    ports: &[MidiPortDescriptor],
    selected_index: &mut usize,
) {
    *selected_index = clamp_port_index(*selected_index, ports.len());
    egui::ComboBox::from_label(label)
        .selected_text(selected_midi_port_text(ports, *selected_index))
        .show_ui(ui, |ui| {
            for (index, port) in ports.iter().enumerate() {
                ui.selectable_value(selected_index, index, midi_port_label(port));
            }
        });
}

fn selected_midi_port_text(ports: &[MidiPortDescriptor], selected_index: usize) -> String {
    ports
        .get(selected_index)
        .map(midi_port_label)
        .unwrap_or_else(|| "none".to_string())
}

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

fn midi_port_label(port: &MidiPortDescriptor) -> String {
    format!("{}: {}", port.index + 1, port.name)
}

fn clamp_port_index(index: usize, port_count: usize) -> usize {
    if port_count == 0 {
        0
    } else {
        index.min(port_count - 1)
    }
}

fn host_midi_backend_detail_text(backend: &DesktopMidiBackend) -> String {
    match backend.device_output_status() {
        Some(status) => device_midi_output_status_text(&status),
        None => "Host MIDI output: capture retains note-on and note-off messages".to_string(),
    }
}

fn device_midi_output_status_text(status: &DeviceMidiOutputStatus) -> String {
    match &status.last_sent_message {
        Some(message) => format!(
            "Host MIDI output device: {} sent {} last {:?} ch {} note {} vel {}",
            midi_port_label(&status.output_port),
            status.total_sent_message_count,
            message.kind,
            message.channel,
            message.note,
            message.velocity
        ),
        None => format!(
            "Host MIDI output device: {} sent 0",
            midi_port_label(&status.output_port)
        ),
    }
}

fn host_midi_input_status_text(input: Option<&DeviceMidiInputConnection>) -> String {
    match input {
        Some(input) => device_midi_input_status_text(&input.status()),
        None => "Host MIDI input: disconnected".to_string(),
    }
}

fn device_midi_input_status_text(status: &DeviceMidiInputStatus) -> String {
    format!(
        "Host MIDI input: {} queued {}/{} received {} decoded {} ignored {} dropped {}",
        midi_port_label(&status.input_port),
        status.queued_event_count,
        status.max_queued_event_count,
        status.total_received_message_count,
        status.total_decoded_event_count,
        status.total_ignored_message_count,
        status.dropped_event_count
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
                "Host audio event: {} {} frames tune {} mute group {} queued={} played={} choked={}",
                receipt.summary.source_sample_name,
                receipt.frame_count,
                tune_text(receipt.summary.tune_cents),
                mute_group_text(receipt.summary.mute_group),
                receipt.queued,
                receipt.played,
                receipt
                    .voice_allocation
                    .as_ref()
                    .map(|allocation| allocation.choked_voice_count)
                    .unwrap_or(0)
            ),
            AudioRenderKind::CountInClick => format!(
                "Host audio event: {} {} frames queued={} played={}",
                count_in_click_summary_label(&receipt.summary),
                receipt.frame_count,
                receipt.queued,
                receipt.played
            ),
        },
        Some(HostAudioEvent::Released { receipt, .. }) => format!(
            "Host audio event: released {} voice(s) for {:?}{:02} {} active={}",
            receipt.released_voice_count,
            receipt.intent.bank,
            receipt.intent.pad_number,
            receipt.intent.sample_name,
            receipt.active_voice_count
        ),
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
            "Host MIDI event: {:?} ch {} note {} vel {} from {:?}{:02}",
            receipt.message.kind,
            receipt.message.channel,
            receipt.message.note,
            receipt.message.velocity,
            receipt.intent.bank,
            receipt.intent.pad_number
        ),
        Some(HostMidiEvent::Ignored { reason, intent }) => format!(
            "Host MIDI event: ignored {reason:?} {:?} ch {} note {}",
            intent.kind, intent.channel, intent.note
        ),
        Some(HostMidiEvent::Failed { error, intent }) => {
            format!(
                "Host MIDI event: failed {:?} ch {} note {}: {}",
                intent.kind, intent.channel, intent.note, error
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
