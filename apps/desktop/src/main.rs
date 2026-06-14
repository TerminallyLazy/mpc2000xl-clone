use eframe::egui;
use mpc_audio::{
    AudioRenderKind, AudioRenderSettings, AudioRenderSummary, CaptureAudioBackend,
    DeviceAudioBackend, DeviceAudioBackendConfig, DeviceAudioBackendStatus, HostAudioBackend,
    HostAudioBackendError, HostAudioEngine, HostAudioError, HostAudioEvent,
    HostAudioPlaybackReport, HostAudioState, MAX_RENDER_FRAMES, RuntimeSampleLibrary,
    load_wav_sample_payload,
};
use mpc_core::{
    CountInClickIntent, DiskOperation, HardwareEvent, MachineOutput, MainScreenField,
    MidiSettingsField, Mode, MpcCore, MpcState, PadAssignmentChange, PadBank, PanelControl,
    ProgramPad, SampleCatalogEntry, SamplePlaybackIntent, SamplePlaybackResolution,
    SetupPreferences, TimingCorrectSettings,
};
use mpc_midi::{
    CaptureMidiBackend, DeviceMidiInputConfig, DeviceMidiInputConnection, DeviceMidiInputStatus,
    DeviceMidiOutputBackend, DeviceMidiOutputStatus, HostMidiBackend, HostMidiEngine,
    HostMidiEvent, HostMidiOutputReport, HostMidiState, MidiInputEvent, MidiPortDescriptor,
    list_device_midi_input_ports, list_device_midi_output_ports,
};
use mpc_storage::{
    DEFAULT_PROJECT_FILE_PATH, load_project_file_with_report,
    save_project_file as save_project_file_to_path,
};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MPC2000XL")
            .with_inner_size([1320.0, 900.0])
            .with_min_inner_size([1060.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MPC2000XL",
        options,
        Box::new(|_cc| Ok(Box::new(MpcDesktopApp::default()))),
    )
}

struct MpcDesktopApp {
    core: MpcCore,
    host_audio: HostAudioEngine<DesktopAudioBackend>,
    host_midi: HostMidiEngine<DesktopMidiBackend>,
    host_midi_input: Option<DeviceMidiInputConnection>,
    midi_input_ports: Vec<MidiPortDescriptor>,
    midi_output_ports: Vec<MidiPortDescriptor>,
    selected_midi_input_port: usize,
    selected_midi_output_port: usize,
    last_status: String,
    last_audio_render: Option<AudioRenderSummary>,
    last_audio_render_error: Option<String>,
    runtime_samples: RuntimeSampleLibrary,
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
    full_level_enabled: bool,
    sixteen_levels_enabled: bool,
    numeric_entry: String,
    last_transport_tick: Instant,
    last_tap_tick: Option<Instant>,
}

enum DesktopAudioBackend {
    Capture(CaptureAudioBackend),
    Device(DeviceAudioBackend),
}

impl DesktopAudioBackend {
    fn capture() -> Self {
        Self::Capture(CaptureAudioBackend::new(8))
    }

    fn device_status(&self) -> Option<DeviceAudioBackendStatus> {
        match self {
            Self::Capture(_) => None,
            Self::Device(backend) => Some(backend.status()),
        }
    }
}

impl HostAudioBackend for DesktopAudioBackend {
    fn backend_name(&self) -> &str {
        match self {
            Self::Capture(backend) => backend.backend_name(),
            Self::Device(backend) => backend.backend_name(),
        }
    }

    fn enqueue_render(
        &mut self,
        rendered: &mpc_audio::RenderedAudio,
    ) -> Result<mpc_audio::HostAudioBackendReceipt, HostAudioBackendError> {
        match self {
            Self::Capture(backend) => backend.enqueue_render(rendered),
            Self::Device(backend) => backend.enqueue_render(rendered),
        }
    }
}

enum DesktopMidiBackend {
    Capture(CaptureMidiBackend),
    Device(DeviceMidiOutputBackend),
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
        let (host_audio, host_audio_status) = default_host_audio_engine();
        let mut runtime_samples = RuntimeSampleLibrary::default();
        let factory_kit_status = load_factory_808_909_samples(&mut runtime_samples);
        Self {
            core: MpcCore::new(),
            host_audio,
            host_midi: HostMidiEngine::enabled(DesktopMidiBackend::capture()),
            host_midi_input: None,
            midi_input_ports: Vec::new(),
            midi_output_ports: Vec::new(),
            selected_midi_input_port: 0,
            selected_midi_output_port: 0,
            last_status: format!("Ready; {host_audio_status}; {factory_kit_status}"),
            last_audio_render: None,
            last_audio_render_error: None,
            runtime_samples,
            sample_import_path: "local-assets/samples/import.wav".to_string(),
            last_runtime_sample_status: factory_kit_status,
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
            full_level_enabled: false,
            sixteen_levels_enabled: false,
            numeric_entry: String::new(),
            last_transport_tick: Instant::now(),
            last_tap_tick: None,
        }
    }
}

impl eframe::App for MpcDesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.tick_transport_clock();
        self.poll_host_midi_input();
        ui.ctx().set_visuals(egui::Visuals::dark());
        egui::Frame::central_panel(ui.style())
            .fill(mpc_color::BENCH)
            .show(ui, |ui| {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(1240.0, 800.0));
                        self.draw_mpc_faceplate(ui);
                    });
            });
        if self.core.state().playing || self.host_midi_input.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
    }
}

impl MpcDesktopApp {
    fn draw_mpc_faceplate(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(mpc_color::CASE)
            .stroke(egui::Stroke::new(2.0, mpc_color::CASE_EDGE))
            .corner_radius(18)
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(700.0);
                        self.draw_mpc_lcd_section(ui);
                        ui.add_space(12.0);
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(330.0);
                                self.draw_mpc_mode_section(ui);
                                ui.add_space(12.0);
                                self.draw_mpc_numeric_section(ui);
                                ui.add_space(12.0);
                                self.draw_mpc_transport_section(ui);
                            });
                            ui.add_space(18.0);
                            ui.vertical(|ui| {
                                ui.set_width(300.0);
                                self.draw_mpc_data_wheel_section(ui);
                                ui.add_space(12.0);
                                self.draw_mpc_cursor_section(ui);
                            });
                        });
                    });
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        ui.set_width(500.0);
                        self.draw_mpc_logo_control_panel(ui);
                        ui.add_space(14.0);
                        self.draw_mpc_pad_section(ui);
                    });
                });
                ui.add_space(14.0);
                self.draw_mpc_status_strip(ui);
                self.draw_mpc_advanced_service_panel(ui);
            });
    }

    fn draw_mpc_logo_control_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(mpc_color::RIGHT_BAY)
            .stroke(egui::Stroke::new(1.0, mpc_color::CASE_EDGE))
            .corner_radius(3)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("AKAI")
                                .size(30.0)
                                .strong()
                                .color(mpc_color::AKAI_RED),
                        );
                        ui.label(
                            egui::RichText::new("professional")
                                .size(12.0)
                                .italics()
                                .color(mpc_color::AKAI_RED),
                        );
                    });
                    ui.add_space(22.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("MPC2000XL")
                                .size(28.0)
                                .strong()
                                .color(mpc_color::INK),
                        );
                        ui.label(
                            egui::RichText::new("MIDI PRODUCTION CENTER")
                                .size(11.0)
                                .strong()
                                .color(mpc_color::FADED_INK),
                        );
                    });
                });
                ui.add_space(14.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if mpc_button(
                                ui,
                                "FULL\nLEVEL",
                                [58.0, 34.0],
                                mpc_color::CREAM_KEY,
                                self.full_level_enabled,
                            )
                            .clicked()
                            {
                                self.full_level_enabled = !self.full_level_enabled;
                                self.last_status = format!(
                                    "FULL LEVEL {}",
                                    if self.full_level_enabled { "on" } else { "off" }
                                );
                            }
                            if mpc_button(
                                ui,
                                "16\nLEVELS",
                                [58.0, 34.0],
                                mpc_color::CREAM_KEY,
                                self.sixteen_levels_enabled,
                            )
                            .clicked()
                            {
                                self.sixteen_levels_enabled = !self.sixteen_levels_enabled;
                                let selected = self.core.state().selected_program_pad;
                                self.last_status = format!(
                                    "16 LEVELS {} for {}",
                                    if self.sixteen_levels_enabled {
                                        "on"
                                    } else {
                                        "off"
                                    },
                                    program_pad_label(selected)
                                );
                            }
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if mpc_button(
                                ui,
                                "NEXT\nSEQ",
                                [58.0, 34.0],
                                mpc_color::CREAM_KEY,
                                self.core.state().mode == Mode::Song,
                            )
                            .clicked()
                            {
                                self.dispatch_event(HardwareEvent::Press {
                                    control: PanelControl::Song,
                                });
                                self.last_status =
                                    "NEXT SEQ: song chain editor selected".to_string();
                            }
                            let selected_track = self.core.state().selected_track;
                            let track_muted = self.core.state().is_track_muted(selected_track);
                            if mpc_button(
                                ui,
                                "TRACK\nMUTE",
                                [58.0, 34.0],
                                mpc_color::CREAM_KEY,
                                track_muted,
                            )
                            .clicked()
                            {
                                self.dispatch_event(HardwareEvent::Press {
                                    control: PanelControl::MainScreen,
                                });
                                self.dispatch_event(HardwareEvent::Press {
                                    control: PanelControl::SoftKey(4),
                                });
                            }
                        });
                    });
                    ui.add_space(26.0);
                    mpc_knob(ui, "REC GAIN");
                    ui.add_space(20.0);
                    mpc_knob(ui, "MAIN VOLUME");
                });
            });
    }

    fn draw_mpc_lcd_section(&mut self, ui: &mut egui::Ui) {
        let lcd = self.core.state().lcd.clone();
        egui::Frame::new()
            .fill(mpc_color::SCREEN_BEZEL)
            .stroke(egui::Stroke::new(2.0, mpc_color::BLACK))
            .corner_radius(6)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                egui::Frame::new()
                    .fill(mpc_color::LCD)
                    .stroke(egui::Stroke::new(2.0, mpc_color::LCD_EDGE))
                    .corner_radius(3)
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(610.0, 136.0));
                        ui.label(
                            egui::RichText::new(lcd.title)
                                .monospace()
                                .size(21.0)
                                .strong()
                                .color(mpc_color::LCD_TEXT),
                        );
                        ui.add_space(4.0);
                        for line in &lcd.lines {
                            ui.label(
                                egui::RichText::new(line)
                                    .monospace()
                                    .size(15.0)
                                    .color(mpc_color::LCD_TEXT),
                            );
                        }
                    });
            });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            for (index, soft_key) in lcd.soft_keys.iter().enumerate() {
                let soft_key_number = index as u8 + 1;
                let label = format!("F{soft_key_number}\n{soft_key}");
                if mpc_button(ui, &label, [96.0, 34.0], mpc_color::CREAM_KEY, false).clicked() {
                    self.dispatch_event(HardwareEvent::Press {
                        control: PanelControl::SoftKey(soft_key_number),
                    });
                }
            }
        });
    }

    fn draw_mpc_transport_section(&mut self, ui: &mut egui::Ui) {
        mpc_section(ui, "TRANSPORT", |ui| {
            ui.horizontal(|ui| {
                if mpc_button(
                    ui,
                    "REC",
                    [70.0, 38.0],
                    mpc_color::REC_RED,
                    self.core.state().recording,
                )
                .clicked()
                {
                    self.record_start();
                }
                let overdub_selected = self.core.state().playing && self.core.state().recording;
                if mpc_button(
                    ui,
                    "OVER\nDUB",
                    [70.0, 38.0],
                    mpc_color::BUTTON,
                    overdub_selected,
                )
                .clicked()
                {
                    self.dispatch_event(HardwareEvent::Press {
                        control: PanelControl::Overdub,
                    });
                }
                if mpc_button(
                    ui,
                    "STOP",
                    [70.0, 38.0],
                    mpc_color::BUTTON,
                    !self.core.state().playing,
                )
                .clicked()
                {
                    self.dispatch_event(HardwareEvent::Press {
                        control: PanelControl::Stop,
                    });
                }
                if mpc_button(
                    ui,
                    "PLAY",
                    [70.0, 38.0],
                    mpc_color::PLAY_GREEN,
                    self.core.state().playing,
                )
                .clicked()
                {
                    self.play();
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if mpc_button(ui, "PLAY\nSTART", [94.0, 36.0], mpc_color::BUTTON, false).clicked() {
                    self.play_start();
                }
                let loop_selected = self.core.state().loop_enabled;
                if mpc_button(ui, "LOOP", [70.0, 36.0], mpc_color::BUTTON, loop_selected).clicked()
                {
                    self.dispatch_event(HardwareEvent::Press {
                        control: PanelControl::ToggleLoop,
                    });
                }
                if mpc_button(ui, "TAP\nTICK", [70.0, 36.0], mpc_color::BUTTON, false).clicked() {
                    self.tap_tick();
                }
            });
        });
    }

    fn draw_mpc_mode_section(&mut self, ui: &mut egui::Ui) {
        let current_mode = self.core.state().mode;
        mpc_section(ui, "MODE", |ui| {
            egui::Grid::new("mpc_mode_keys")
                .num_columns(3)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    self.mpc_mode_key(
                        ui,
                        "MAIN\nSCREEN",
                        PanelControl::MainScreen,
                        Mode::Main,
                        current_mode,
                    );
                    self.mpc_mode_key(
                        ui,
                        "PROGRAM",
                        PanelControl::Program,
                        Mode::Program,
                        current_mode,
                    );
                    self.mpc_mode_key(
                        ui,
                        "SAMPLE",
                        PanelControl::Sample,
                        Mode::Sample,
                        current_mode,
                    );
                    ui.end_row();
                    self.mpc_mode_key(ui, "TRIM", PanelControl::Trim, Mode::Trim, current_mode);
                    self.mpc_mode_key(ui, "SONG", PanelControl::Song, Mode::Song, current_mode);
                    self.mpc_mode_key(
                        ui,
                        "MIDI\nSYNC",
                        PanelControl::Midi,
                        Mode::Midi,
                        current_mode,
                    );
                    ui.end_row();
                    self.mpc_mode_key(
                        ui,
                        "TIMING\nCORRECT",
                        PanelControl::TimingCorrect,
                        Mode::TimingCorrect,
                        current_mode,
                    );
                    self.mpc_mode_key(ui, "DISK", PanelControl::Disk, Mode::Disk, current_mode);
                    self.mpc_mode_key(ui, "SETUP", PanelControl::Setup, Mode::Setup, current_mode);
                });
        });
    }

    fn mpc_mode_key(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        control: PanelControl,
        mode: Mode,
        current_mode: Mode,
    ) {
        if mpc_button(
            ui,
            label,
            [98.0, 42.0],
            mpc_color::BUTTON,
            current_mode == mode,
        )
        .clicked()
        {
            self.dispatch_event(HardwareEvent::Press { control });
        }
    }

    fn draw_mpc_io_section(&mut self, ui: &mut egui::Ui) {
        mpc_section(ui, "DISK / SAMPLE / MIDI", |ui| {
            ui.horizontal(|ui| {
                if mpc_button(ui, "SAVE", [78.0, 34.0], mpc_color::BUTTON, false).clicked() {
                    self.save_project_file();
                }
                if mpc_button(ui, "LOAD", [78.0, 34.0], mpc_color::BUTTON, false).clicked() {
                    self.load_project_file();
                }
                if mpc_button(ui, "SNAP", [78.0, 34.0], mpc_color::BUTTON, false).clicked() {
                    self.save_project_snapshot();
                }
            });
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.project_file_path)
                    .desired_width(318.0)
                    .font(egui::TextStyle::Monospace),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if mpc_button(ui, "WAV\nLOAD", [78.0, 42.0], mpc_color::BUTTON, false).clicked() {
                    self.load_wav_to_selected_pad();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.sample_import_path)
                        .desired_width(224.0)
                        .font(egui::TextStyle::Monospace),
                );
            });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let mut audio_enabled = self.host_audio.is_enabled();
                if ui.checkbox(&mut audio_enabled, "AUDIO").changed() {
                    self.host_audio.set_enabled(audio_enabled);
                }
                let mut midi_enabled = self.host_midi.is_enabled();
                if ui.checkbox(&mut midi_enabled, "MIDI").changed() {
                    self.host_midi.set_enabled(midi_enabled);
                }
                if mpc_button(ui, "MIDI\nPORTS", [78.0, 36.0], mpc_color::BUTTON, false).clicked() {
                    self.refresh_midi_device_ports();
                }
            });
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(compact_io_text(
                    self.host_audio.state(),
                    self.host_midi.state(),
                    &self.last_runtime_sample_status,
                ))
                .size(11.0)
                .color(mpc_color::FADED_INK),
            );
        });
    }

    fn draw_mpc_cursor_section(&mut self, ui: &mut egui::Ui) {
        mpc_section(ui, "CURSOR", |ui| {
            egui::Grid::new("mpc_cursor")
                .num_columns(3)
                .spacing([6.0, 6.0])
                .show(ui, |ui| {
                    ui.allocate_space(egui::vec2(44.0, 36.0));
                    if mpc_button(ui, "^", [44.0, 36.0], mpc_color::BUTTON, false).clicked() {
                        self.dispatch_event(HardwareEvent::Press {
                            control: PanelControl::CursorUp,
                        });
                    }
                    ui.allocate_space(egui::vec2(44.0, 36.0));
                    ui.end_row();
                    if mpc_button(ui, "<", [44.0, 36.0], mpc_color::BUTTON, false).clicked() {
                        self.dispatch_event(HardwareEvent::Press {
                            control: PanelControl::CursorLeft,
                        });
                    }
                    if mpc_button(ui, "DO IT", [44.0, 36.0], mpc_color::DARK_BUTTON, false)
                        .clicked()
                    {
                        self.do_it();
                    }
                    if mpc_button(ui, ">", [44.0, 36.0], mpc_color::BUTTON, false).clicked() {
                        self.dispatch_event(HardwareEvent::Press {
                            control: PanelControl::CursorRight,
                        });
                    }
                    ui.end_row();
                    ui.allocate_space(egui::vec2(44.0, 36.0));
                    if mpc_button(ui, "v", [44.0, 36.0], mpc_color::BUTTON, false).clicked() {
                        self.dispatch_event(HardwareEvent::Press {
                            control: PanelControl::CursorDown,
                        });
                    }
                    ui.allocate_space(egui::vec2(44.0, 36.0));
                });
        });
    }

    fn draw_mpc_data_wheel_section(&mut self, ui: &mut egui::Ui) {
        mpc_section(ui, "DATA", |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(118.0, 118.0), egui::Sense::click());
            let painter = ui.painter();
            painter.circle_filled(rect.center(), 56.0, mpc_color::WHEEL);
            painter.circle_stroke(
                rect.center(),
                56.0,
                egui::Stroke::new(3.0, mpc_color::BLACK),
            );
            painter.circle_stroke(
                rect.center(),
                38.0,
                egui::Stroke::new(1.0, mpc_color::WHEEL_RING),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "DATA",
                egui::FontId::proportional(15.0),
                mpc_color::WHEEL_TEXT,
            );
            if response.clicked() {
                let delta = response
                    .interact_pointer_pos()
                    .map(|position| if position.x < rect.center().x { -1 } else { 1 })
                    .unwrap_or(1);
                self.dispatch_event(HardwareEvent::TurnDataWheel { delta });
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if mpc_button(ui, "-", [52.0, 30.0], mpc_color::BUTTON, false).clicked() {
                    self.dispatch_event(HardwareEvent::TurnDataWheel { delta: -1 });
                }
                if mpc_button(ui, "+", [52.0, 30.0], mpc_color::BUTTON, false).clicked() {
                    self.dispatch_event(HardwareEvent::TurnDataWheel { delta: 1 });
                }
            });
        });
    }

    fn draw_mpc_numeric_section(&mut self, ui: &mut egui::Ui) {
        mpc_section(ui, "NUMERIC", |ui| {
            let keys = [[7, 8, 9], [4, 5, 6], [1, 2, 3]];
            egui::Grid::new("mpc_numeric")
                .num_columns(3)
                .spacing([6.0, 6.0])
                .show(ui, |ui| {
                    for row in keys {
                        for number in row {
                            if mpc_button(
                                ui,
                                &number.to_string(),
                                [38.0, 30.0],
                                mpc_color::BUTTON,
                                false,
                            )
                            .clicked()
                            {
                                self.push_numeric_digit(number);
                            }
                        }
                        ui.end_row();
                    }
                    if mpc_button(ui, "-", [38.0, 30.0], mpc_color::BUTTON, false).clicked() {
                        self.toggle_numeric_sign();
                    }
                    if mpc_button(ui, "0", [38.0, 30.0], mpc_color::BUTTON, false).clicked() {
                        self.push_numeric_digit(0);
                    }
                    if mpc_button(ui, ".", [38.0, 30.0], mpc_color::BUTTON, false).clicked() {
                        self.push_numeric_decimal();
                    }
                });
            ui.add_space(6.0);
            let entry = if self.numeric_entry.is_empty() {
                "ENTRY --".to_string()
            } else {
                format!("ENTRY {}", self.numeric_entry)
            };
            ui.label(
                egui::RichText::new(entry)
                    .monospace()
                    .size(11.0)
                    .color(mpc_color::INK),
            );
        });
    }

    fn draw_mpc_pad_section(&mut self, ui: &mut egui::Ui) {
        let active_bank = self.core.state().pad_bank;
        let selected_program_pad = self.core.state().selected_program_pad;
        let program_mode = self.core.state().mode == Mode::Program;

        mpc_section(ui, "DRUM PADS", |ui| {
            ui.horizontal(|ui| {
                self.mpc_bank_button(ui, "A", PadBank::A, PanelControl::PadBankA, active_bank);
                self.mpc_bank_button(ui, "B", PadBank::B, PanelControl::PadBankB, active_bank);
                self.mpc_bank_button(ui, "C", PadBank::C, PanelControl::PadBankC, active_bank);
                self.mpc_bank_button(ui, "D", PadBank::D, PanelControl::PadBankD, active_bank);
            });
            ui.add_space(12.0);
            let rows = [
                [13, 14, 15, 16],
                [9, 10, 11, 12],
                [5, 6, 7, 8],
                [1, 2, 3, 4],
            ];
            egui::Grid::new("mpc_large_pads")
                .num_columns(4)
                .spacing([12.0, 12.0])
                .show(ui, |ui| {
                    for row in rows {
                        for pad in row {
                            let pad_address = ProgramPad {
                                bank: active_bank,
                                pad_number: pad,
                            };
                            let selected = program_mode && selected_program_pad == pad_address;
                            let label = factory_pad_label(active_bank, pad)
                                .map(str::to_string)
                                .unwrap_or_else(|| program_pad_label(pad_address));
                            let label = if self.sixteen_levels_enabled {
                                let selected_pad = self.core.state().selected_program_pad;
                                format!(
                                    "V{:03}\n{}",
                                    sixteen_levels_velocity(pad),
                                    program_pad_label(selected_pad)
                                )
                            } else {
                                label
                            };
                            if mpc_pad(ui, &label, selected).clicked() {
                                self.strike_panel_pad(active_bank, pad);
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    }

    fn mpc_bank_button(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        bank: PadBank,
        control: PanelControl,
        active_bank: PadBank,
    ) {
        if mpc_button(
            ui,
            label,
            [56.0, 34.0],
            mpc_color::BUTTON,
            active_bank == bank,
        )
        .clicked()
        {
            self.dispatch_event(HardwareEvent::Press { control });
        }
    }

    fn draw_mpc_status_strip(&self, ui: &mut egui::Ui) {
        let state = self.core.state();
        egui::Frame::new()
            .fill(mpc_color::STATUS)
            .stroke(egui::Stroke::new(1.0, mpc_color::CASE_EDGE))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "SEQ {:02}   TRK {:02}   {}   BAR {:03}   TICK {:06}   {}   {}",
                            state.sequence_index,
                            state.selected_track,
                            tempo_text(state.tempo_bpm_x100),
                            state.bar_count,
                            state.playhead_ticks.min(999_999),
                            self.last_status,
                            main_host_audio_text(
                                self.host_audio.state(),
                                self.host_audio.backend()
                            )
                        ))
                        .monospace()
                        .size(13.0)
                        .color(mpc_color::STATUS_TEXT),
                    );
                });
            });
    }

    fn draw_mpc_advanced_service_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        egui::CollapsingHeader::new("ADVANCED / SERVICE")
            .default_open(false)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Diagnostic controls")
                        .size(12.0)
                        .color(mpc_color::FADED_INK),
                );
                self.draw_mpc_io_section(ui);
                ui.add_space(8.0);
                self.draw_lcd(ui);
                ui.add_space(8.0);
                self.draw_mode_buttons(ui);
                ui.add_space(8.0);
                self.draw_edit_controls(ui);
                ui.add_space(8.0);
                self.draw_project_snapshot_controls(ui);
                ui.add_space(8.0);
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
                ui.add_space(8.0);
                self.draw_pads(ui);
            });
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
                        self.clear_runtime_sample_payloads("snapshot load");
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
                        self.clear_runtime_sample_payloads("project file load");
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
        let resets_transport_clock = matches!(
            &event,
            HardwareEvent::Press {
                control: PanelControl::Play
                    | PanelControl::Overdub
                    | PanelControl::Stop
                    | PanelControl::LocateStart
            }
        );
        let outputs = self.core.dispatch(event);
        self.handle_machine_outputs(outputs, false);
        if resets_transport_clock {
            self.last_transport_tick = Instant::now();
        }
    }

    fn dispatch_transport_tick(&mut self, micros: u64) {
        let outputs = self.core.dispatch(HardwareEvent::Tick { micros });
        self.handle_machine_outputs(outputs, true);
    }

    fn handle_machine_outputs(
        &mut self,
        outputs: Vec<MachineOutput>,
        preserve_status_when_empty: bool,
    ) {
        if preserve_status_when_empty
            && (outputs.is_empty()
                || outputs
                    .iter()
                    .all(|output| matches!(output, MachineOutput::LcdChanged)))
        {
            return;
        }

        let render_or_host_error = self.handle_audio_outputs(&outputs);
        let midi_host_error = self.handle_midi_outputs(&outputs);
        let disk_operation_status = self.handle_disk_operation_request(&outputs);
        self.prune_runtime_samples_to_project_metadata();
        self.last_status = disk_operation_status
            .or(midi_host_error)
            .or(render_or_host_error)
            .unwrap_or_else(|| Self::status_from_outputs(&outputs, self.core.state()));
    }

    fn tick_transport_clock(&mut self) {
        let now = Instant::now();
        if !self.core.state().playing {
            self.last_transport_tick = now;
            return;
        }

        let elapsed = now.saturating_duration_since(self.last_transport_tick);
        self.last_transport_tick = now;
        let micros = elapsed.as_micros().min(50_000) as u64;
        if micros > 0 {
            self.dispatch_transport_tick(micros);
        }
    }

    fn record_start(&mut self) {
        if !self.core.state().playing {
            self.dispatch_event(HardwareEvent::Press {
                control: PanelControl::LocateStart,
            });
        }
        self.dispatch_event(HardwareEvent::Press {
            control: PanelControl::Rec,
        });
        if !self.core.state().playing {
            self.dispatch_event(HardwareEvent::Press {
                control: PanelControl::Play,
            });
        }
    }

    fn play(&mut self) {
        if self.should_restart_playback_from_start() {
            self.dispatch_event(HardwareEvent::Press {
                control: PanelControl::LocateStart,
            });
        }
        self.dispatch_event(HardwareEvent::Press {
            control: PanelControl::Play,
        });
    }

    fn play_start(&mut self) {
        self.dispatch_event(HardwareEvent::Press {
            control: PanelControl::LocateStart,
        });
        self.dispatch_event(HardwareEvent::Press {
            control: PanelControl::Play,
        });
    }

    fn should_restart_playback_from_start(&self) -> bool {
        let state = self.core.state();
        if state.playing || state.recorded_events.is_empty() {
            return false;
        }

        !state
            .recorded_events
            .iter()
            .any(|event| event.tick > state.playhead_ticks)
    }

    fn tap_tick(&mut self) {
        let now = Instant::now();
        let click_error = self.play_tap_tick_click();
        let Some(previous_tap) = self.last_tap_tick.replace(now) else {
            self.last_status =
                click_error.unwrap_or_else(|| "Tap tick: waiting for next tap".to_string());
            return;
        };

        let elapsed = now.saturating_duration_since(previous_tap);
        let millis = elapsed.as_millis();
        if !(200..=2_000).contains(&millis) {
            self.last_status =
                click_error.unwrap_or_else(|| "Tap tick: tap again in time".to_string());
            return;
        }

        let bpm_x100 = ((60_000_u128 * 100) / millis).clamp(3_000, 30_000) as u32;
        match self.core.set_tempo_bpm_x100(bpm_x100) {
            Ok(outputs) => {
                self.handle_machine_outputs(outputs, true);
                self.last_status = click_error
                    .unwrap_or_else(|| format!("Tap tempo set {}", tempo_text(bpm_x100)));
            }
            Err(error) => {
                self.last_status = format!("Tap tempo rejected: {error}");
            }
        }
    }

    fn play_tap_tick_click(&mut self) -> Option<String> {
        let intent = CountInClickIntent {
            count_in_tick: self.core.state().playhead_ticks,
            bar_index: 1,
            beat_index: 1,
            accent: true,
        };
        let report = self
            .host_audio
            .play_count_in_click_with_render_summary(&intent);
        self.record_audio_report(report, "Tap tick render failed")
    }

    fn strike_panel_pad(&mut self, active_bank: PadBank, pad: u8) {
        let (bank, pad, velocity) = self.panel_pad_strike(active_bank, pad);
        self.dispatch_event(HardwareEvent::StrikePad {
            bank,
            pad,
            velocity,
        });
    }

    fn panel_pad_strike(&self, active_bank: PadBank, pad: u8) -> (PadBank, u8, u8) {
        if self.sixteen_levels_enabled {
            let selected = self.core.state().selected_program_pad;
            return (
                selected.bank,
                selected.pad_number,
                sixteen_levels_velocity(pad),
            );
        }

        let velocity = if self.full_level_enabled { 127 } else { 100 };
        (active_bank, pad, velocity)
    }

    fn push_numeric_digit(&mut self, digit: u8) {
        if digit > 9 || self.numeric_entry.len() >= 8 {
            return;
        }
        if self.numeric_entry == "0" {
            self.numeric_entry.clear();
        }
        self.numeric_entry.push(char::from(b'0' + digit));
        self.last_status = format!("Numeric entry {}", self.numeric_entry);
    }

    fn toggle_numeric_sign(&mut self) {
        if self.numeric_entry.starts_with('-') {
            self.numeric_entry.remove(0);
        } else if self.numeric_entry.len() < 8 {
            self.numeric_entry.insert(0, '-');
        }
        self.last_status = format!(
            "Numeric entry {}",
            if self.numeric_entry.is_empty() {
                "--"
            } else {
                &self.numeric_entry
            }
        );
    }

    fn push_numeric_decimal(&mut self) {
        if self.numeric_entry.len() >= 8 || self.numeric_entry.contains('.') {
            return;
        }
        if self.numeric_entry.is_empty() || self.numeric_entry == "-" {
            self.numeric_entry.push('0');
        }
        self.numeric_entry.push('.');
        self.last_status = format!("Numeric entry {}", self.numeric_entry);
    }

    fn do_it(&mut self) {
        if !self.numeric_entry.is_empty() {
            self.apply_numeric_entry();
            return;
        }

        match self.core.state().mode {
            Mode::Disk => {
                let soft_key = match self.core.state().selected_disk_operation {
                    DiskOperation::SaveProject => 2,
                    DiskOperation::LoadProject => 3,
                };
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::SoftKey(soft_key),
                });
            }
            Mode::Program => {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::SoftKey(2),
                });
            }
            Mode::Sample | Mode::Trim => {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::SoftKey(6),
                });
            }
            Mode::Song => {
                self.dispatch_event(HardwareEvent::Press {
                    control: PanelControl::SoftKey(2),
                });
            }
            Mode::Main => {
                self.last_status =
                    "DO IT: enter a number first, then press DO IT to set the selected MAIN field"
                        .to_string();
            }
            Mode::Midi | Mode::TimingCorrect | Mode::Setup => {
                self.last_status =
                    "DO IT: use cursor/data controls or enter a MAIN numeric value".to_string();
            }
        }
    }

    fn apply_numeric_entry(&mut self) {
        if self.core.state().playing {
            self.last_status =
                "Numeric entry held: stop transport before applying MAIN values".to_string();
            return;
        }

        let entry = std::mem::take(&mut self.numeric_entry);
        let mode = self.core.state().mode;
        let result = match mode {
            Mode::Main => self.apply_main_numeric_entry(&entry),
            Mode::Program => self.apply_program_numeric_entry(&entry),
            _ => Err(format!(
                "Numeric entry {entry} is not mapped on {:?}; use MAIN or PROGRAM",
                mode
            )),
        };

        self.last_status = match result {
            Ok(message) => message,
            Err(message) => format!("Numeric entry rejected: {message}"),
        };
    }

    fn apply_main_numeric_entry(&mut self, entry: &str) -> Result<String, String> {
        let mut snapshot = self.core.export_project_snapshot();
        let message = match self.core.state().selected_main_field {
            MainScreenField::Sequence => {
                let sequence = parse_bounded_u8(entry, 1, 99, "sequence")?;
                snapshot.sequence.index = sequence;
                snapshot.sequence.name = format!("Sequence{sequence:02}");
                format!("MAIN sequence set to {sequence:02}")
            }
            MainScreenField::Track => {
                let track = parse_bounded_u8(entry, 1, 64, "track")?;
                snapshot.sequence.selected_track = track;
                format!("MAIN track set to {track:02}")
            }
            MainScreenField::Tempo => {
                let tempo_bpm_x100 = parse_tempo_bpm_x100(entry)?;
                snapshot.sequence.tempo_bpm_x100 = tempo_bpm_x100;
                format!("MAIN tempo set to {}", tempo_text(tempo_bpm_x100))
            }
            MainScreenField::Bars => {
                let bars = parse_bounded_u16(entry, 1, 999, "bars")?;
                snapshot.sequence.bar_count = bars;
                format!("MAIN bars set to {bars:03}")
            }
        };

        self.core
            .restore_project_snapshot(snapshot)
            .map_err(|error| error.to_string())?;
        Ok(message)
    }

    fn apply_program_numeric_entry(&mut self, entry: &str) -> Result<String, String> {
        let pad = parse_bounded_u8(entry, 1, 16, "program pad")?;
        let mut snapshot = self.core.export_project_snapshot();
        let bank = self.core.state().pad_bank;
        snapshot.machine.selected_program_pad = ProgramPad {
            bank,
            pad_number: pad,
        };
        self.core
            .restore_project_snapshot(snapshot)
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "PROGRAM selected {}",
            program_pad_label(ProgramPad {
                bank,
                pad_number: pad
            })
        ))
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
        self.last_audio_render = None;
        self.last_audio_render_error = None;
        self.last_runtime_sample_status = format!("Runtime WAV: cleared after {reason}");
    }

    fn prune_runtime_samples_to_project_metadata(&mut self) {
        if self.runtime_samples.is_empty() {
            return;
        }

        let retained_sample_ids = runtime_sample_ids_referenced_by_project(self.core.state());
        self.runtime_samples
            .retain(|sample_id, _| retained_sample_ids.contains(sample_id));
        if self.runtime_samples.is_empty() {
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
            if *playing && *recording {
                return "Recording: transport running".to_string();
            }
            if !*playing && *recording {
                return "REC armed: press PLAY START or OVERDUB to record".to_string();
            }
            if *playing {
                return "Playback running".to_string();
            }
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
            let capture_selected =
                matches!(self.host_audio.backend(), DesktopAudioBackend::Capture(_));
            if ui.selectable_label(capture_selected, "Capture").clicked() {
                self.switch_host_audio_to_capture();
            }
            let device_selected =
                matches!(self.host_audio.backend(), DesktopAudioBackend::Device(_));
            if ui
                .selectable_label(device_selected, "Default device")
                .clicked()
            {
                self.switch_host_audio_to_default_device();
            }

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
                self.host_midi.set_enabled(enabled);
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
        if matches!(self.host_audio.backend(), DesktopAudioBackend::Device(_)) {
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
                    DesktopAudioBackend::Device(backend),
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

#[derive(Debug, Clone, Copy)]
struct FactoryPadWav {
    bank: PadBank,
    pad: u8,
    pad_label: &'static str,
    sample_name: &'static str,
    relative_path: &'static str,
}

const FACTORY_SAMPLE_ROOT: &str = "local-assets/samples";

const FACTORY_PAD_WAVS: &[FactoryPadWav] = &[
    FactoryPadWav {
        bank: PadBank::A,
        pad: 1,
        pad_label: "808\nKICK",
        sample_name: "808 KICK",
        relative_path: "roland-tr-808/Roland TR-808/BD/BD7575.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 2,
        pad_label: "808\nSNARE",
        sample_name: "808 SNARE",
        relative_path: "roland-tr-808/Roland TR-808/SD/SD7575.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 3,
        pad_label: "808\nCLAP",
        sample_name: "808 CLAP",
        relative_path: "roland-tr-808/Roland TR-808/CP/CP.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 4,
        pad_label: "808\nRIM",
        sample_name: "808 RIMSHOT",
        relative_path: "roland-tr-808/Roland TR-808/RS/RS.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 5,
        pad_label: "808\nCHH",
        sample_name: "808 CLOSED HAT",
        relative_path: "roland-tr-808/Roland TR-808/CH/CH.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 6,
        pad_label: "808\nOHH",
        sample_name: "808 OPEN HAT",
        relative_path: "roland-tr-808/Roland TR-808/OH/OH75.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 7,
        pad_label: "808\nCOW",
        sample_name: "808 COWBELL",
        relative_path: "roland-tr-808/Roland TR-808/CB/CB.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 8,
        pad_label: "808\nCRASH",
        sample_name: "808 CRASH",
        relative_path: "roland-tr-808/Roland TR-808/CY/CY7575.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 9,
        pad_label: "808\nTOM L",
        sample_name: "808 LOW TOM",
        relative_path: "roland-tr-808/Roland TR-808/LT/LT50.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 10,
        pad_label: "808\nTOM M",
        sample_name: "808 MID TOM",
        relative_path: "roland-tr-808/Roland TR-808/MT/MT50.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 11,
        pad_label: "808\nTOM H",
        sample_name: "808 HIGH TOM",
        relative_path: "roland-tr-808/Roland TR-808/HT/HT50.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 12,
        pad_label: "808\nCLAVE",
        sample_name: "808 CLAVES",
        relative_path: "roland-tr-808/Roland TR-808/CL/CL.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 13,
        pad_label: "808\nKICK 2",
        sample_name: "808 KICK 2",
        relative_path: "roland-tr-808/Roland TR-808/BD/BD5010.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 14,
        pad_label: "808\nKICK 3",
        sample_name: "808 KICK 3",
        relative_path: "roland-tr-808/Roland TR-808/BD/BD1010.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 15,
        pad_label: "808\nCABASA",
        sample_name: "808 CABASA",
        relative_path: "roland-tr-808/Roland TR-808/MA/MA.WAV",
    },
    FactoryPadWav {
        bank: PadBank::A,
        pad: 16,
        pad_label: "808\nCRASH 2",
        sample_name: "808 CRASH 2",
        relative_path: "roland-tr-808/Roland TR-808/CY/CY5010.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 1,
        pad_label: "909\nKICK",
        sample_name: "909 KICK",
        relative_path: "roland-tr-909/Roland TR-909/BTAA0DA.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 2,
        pad_label: "909\nSNARE",
        sample_name: "909 SNARE",
        relative_path: "roland-tr-909/Roland TR-909/STATASA.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 3,
        pad_label: "909\nCLAP",
        sample_name: "909 CLAP",
        relative_path: "roland-tr-909/Roland TR-909/HANDCLP1.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 4,
        pad_label: "909\nRIM",
        sample_name: "909 RIMSHOT",
        relative_path: "roland-tr-909/Roland TR-909/RIM127.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 5,
        pad_label: "909\nCHH",
        sample_name: "909 CLOSED HAT",
        relative_path: "roland-tr-909/Roland TR-909/HHCD8.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 6,
        pad_label: "909\nOHH",
        sample_name: "909 OPEN HAT",
        relative_path: "roland-tr-909/Roland TR-909/HHODA.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 7,
        pad_label: "909\nRIDE",
        sample_name: "909 RIDE",
        relative_path: "roland-tr-909/Roland TR-909/RIDED0.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 8,
        pad_label: "909\nCRASH",
        sample_name: "909 CRASH",
        relative_path: "roland-tr-909/Roland TR-909/CSHD8.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 9,
        pad_label: "909\nTOM L",
        sample_name: "909 LOW TOM",
        relative_path: "roland-tr-909/Roland TR-909/LTADA.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 10,
        pad_label: "909\nTOM M",
        sample_name: "909 MID TOM",
        relative_path: "roland-tr-909/Roland TR-909/MTADA.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 11,
        pad_label: "909\nTOM H",
        sample_name: "909 HIGH TOM",
        relative_path: "roland-tr-909/Roland TR-909/HTADA.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 12,
        pad_label: "909\nBD 2",
        sample_name: "909 KICK 2",
        relative_path: "roland-tr-909/Roland TR-909/BT0A0D0.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 13,
        pad_label: "909\nBD 3",
        sample_name: "909 KICK 3",
        relative_path: "roland-tr-909/Roland TR-909/BT0A0DA.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 14,
        pad_label: "909\nBD 4",
        sample_name: "909 KICK 4",
        relative_path: "roland-tr-909/Roland TR-909/BTAA0D3.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 15,
        pad_label: "909\nCLAP 2",
        sample_name: "909 CLAP 2",
        relative_path: "roland-tr-909/Roland TR-909/HANDCLP2.WAV",
    },
    FactoryPadWav {
        bank: PadBank::B,
        pad: 16,
        pad_label: "909\nOP/CL",
        sample_name: "909 OPEN/CLOSED HAT",
        relative_path: "roland-tr-909/Roland TR-909/OPCL1.WAV",
    },
];

fn default_host_audio_engine() -> (HostAudioEngine<DesktopAudioBackend>, String) {
    match DeviceAudioBackend::open_default(DeviceAudioBackendConfig::default()) {
        Ok(backend) => {
            let status = backend.status();
            let settings = desktop_audio_settings(status.sample_rate_hz);
            let engine = HostAudioEngine::enabled(DesktopAudioBackend::Device(backend), settings)
                .expect("desktop device audio settings should satisfy guardrails");
            (
                engine,
                format!(
                    "audio device {} {} Hz",
                    status.device_name, status.sample_rate_hz
                ),
            )
        }
        Err(error) => {
            let settings = desktop_audio_settings(44_100);
            let engine = HostAudioEngine::enabled(DesktopAudioBackend::capture(), settings)
                .expect("desktop capture audio settings should satisfy guardrails");
            (engine, format!("audio capture fallback: {error}"))
        }
    }
}

fn desktop_audio_settings(sample_rate_hz: u32) -> AudioRenderSettings {
    let frame_count = (sample_rate_hz as usize)
        .saturating_mul(2)
        .min(MAX_RENDER_FRAMES);
    AudioRenderSettings::new(sample_rate_hz, frame_count)
        .expect("desktop audio settings should satisfy guardrails")
}

fn load_factory_808_909_samples(runtime_samples: &mut RuntimeSampleLibrary) -> String {
    let root = Path::new(FACTORY_SAMPLE_ROOT);
    let mut loaded = 0_usize;
    let mut missing = 0_usize;
    let mut failed = 0_usize;

    for sample in FACTORY_PAD_WAVS {
        let path = root.join(sample.relative_path);
        if !path.exists() {
            missing = missing.saturating_add(1);
            continue;
        }
        match load_wav_sample_payload(&path) {
            Ok(payload) => {
                runtime_samples.insert(
                    factory_sample_id(sample.bank, sample.pad),
                    sample.sample_name,
                    payload,
                );
                loaded = loaded.saturating_add(1);
            }
            Err(_) => {
                failed = failed.saturating_add(1);
            }
        }
    }

    if loaded == 0 {
        format!("Factory 808/909 WAVs: none loaded ({missing} missing, {failed} failed)")
    } else {
        format!("Factory 808/909 WAVs: {loaded} loaded ({missing} missing, {failed} failed)")
    }
}

fn factory_sample_id(bank: PadBank, pad: u8) -> String {
    format!("synthetic_{}_{pad:02}", bank.label().to_ascii_lowercase())
}

fn factory_pad_label(bank: PadBank, pad: u8) -> Option<&'static str> {
    FACTORY_PAD_WAVS
        .iter()
        .find(|sample| sample.bank == bank && sample.pad == pad)
        .map(|sample| sample.pad_label)
}

fn sixteen_levels_velocity(pad: u8) -> u8 {
    let pad = pad.clamp(1, 16);
    let velocity = 8_u16 + (u16::from(pad - 1) * 119 / 15);
    velocity.min(127) as u8
}

fn parse_bounded_u8(entry: &str, min: u8, max: u8, label: &str) -> Result<u8, String> {
    if entry.contains('.') || entry.starts_with('-') {
        return Err(format!("{label} must be a whole number {min}..={max}"));
    }
    let value = entry
        .parse::<u16>()
        .map_err(|_| format!("{label} must be a whole number {min}..={max}"))?;
    if value < u16::from(min) || value > u16::from(max) {
        return Err(format!("{label} must be in range {min}..={max}"));
    }
    Ok(value as u8)
}

fn parse_bounded_u16(entry: &str, min: u16, max: u16, label: &str) -> Result<u16, String> {
    if entry.contains('.') || entry.starts_with('-') {
        return Err(format!("{label} must be a whole number {min}..={max}"));
    }
    let value = entry
        .parse::<u32>()
        .map_err(|_| format!("{label} must be a whole number {min}..={max}"))?;
    if value < u32::from(min) || value > u32::from(max) {
        return Err(format!("{label} must be in range {min}..={max}"));
    }
    Ok(value as u16)
}

fn parse_tempo_bpm_x100(entry: &str) -> Result<u32, String> {
    if entry.starts_with('-') {
        return Err("tempo must be in range 30.00..=300.00 BPM".to_string());
    }

    let (whole, cents) = match entry.split_once('.') {
        Some((whole, cents)) => {
            if cents.len() > 2 {
                return Err("tempo supports at most two decimal places".to_string());
            }
            let padded = format!("{cents:0<2}");
            (whole, padded)
        }
        None => (entry, "00".to_string()),
    };

    let bpm = whole
        .parse::<u32>()
        .map_err(|_| "tempo must be in range 30.00..=300.00 BPM".to_string())?;
    let cents = cents
        .parse::<u32>()
        .map_err(|_| "tempo must be in range 30.00..=300.00 BPM".to_string())?;
    let tempo = bpm.saturating_mul(100).saturating_add(cents);
    if !(3_000..=30_000).contains(&tempo) {
        return Err("tempo must be in range 30.00..=300.00 BPM".to_string());
    }
    Ok(tempo)
}

mod mpc_color {
    use eframe::egui::Color32;

    pub const BENCH: Color32 = Color32::from_rgb(20, 22, 24);
    pub const CASE: Color32 = Color32::from_rgb(188, 184, 171);
    pub const CASE_PANEL: Color32 = Color32::from_rgb(160, 157, 146);
    pub const RIGHT_BAY: Color32 = Color32::from_rgb(177, 181, 178);
    pub const CASE_EDGE: Color32 = Color32::from_rgb(78, 76, 70);
    pub const INK: Color32 = Color32::from_rgb(30, 30, 28);
    pub const FADED_INK: Color32 = Color32::from_rgb(75, 73, 68);
    pub const AKAI_RED: Color32 = Color32::from_rgb(185, 32, 34);
    pub const BLACK: Color32 = Color32::from_rgb(15, 15, 14);
    pub const SCREEN_BEZEL: Color32 = Color32::from_rgb(37, 40, 38);
    pub const LCD: Color32 = Color32::from_rgb(166, 188, 126);
    pub const LCD_EDGE: Color32 = Color32::from_rgb(91, 108, 76);
    pub const LCD_TEXT: Color32 = Color32::from_rgb(28, 47, 29);
    pub const BUTTON: Color32 = Color32::from_rgb(77, 78, 75);
    pub const DARK_BUTTON: Color32 = Color32::from_rgb(42, 43, 42);
    pub const CREAM_KEY: Color32 = Color32::from_rgb(218, 215, 193);
    pub const BUTTON_TEXT: Color32 = Color32::from_rgb(236, 236, 228);
    pub const ACTIVE: Color32 = Color32::from_rgb(47, 114, 141);
    pub const PAD: Color32 = Color32::from_rgb(33, 34, 34);
    pub const PAD_SELECTED: Color32 = Color32::from_rgb(55, 68, 70);
    pub const PAD_TEXT: Color32 = Color32::from_rgb(232, 231, 220);
    pub const REC_RED: Color32 = Color32::from_rgb(143, 40, 39);
    pub const PLAY_GREEN: Color32 = Color32::from_rgb(58, 110, 62);
    pub const WHEEL: Color32 = Color32::from_rgb(30, 31, 31);
    pub const WHEEL_RING: Color32 = Color32::from_rgb(93, 95, 92);
    pub const WHEEL_TEXT: Color32 = Color32::from_rgb(207, 208, 198);
    pub const STATUS: Color32 = Color32::from_rgb(49, 50, 48);
    pub const STATUS_TEXT: Color32 = Color32::from_rgb(222, 223, 211);
}

fn mpc_section<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(mpc_color::CASE_PANEL)
        .stroke(egui::Stroke::new(1.0, mpc_color::CASE_EDGE))
        .corner_radius(5)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(title)
                    .size(11.0)
                    .strong()
                    .color(mpc_color::FADED_INK),
            );
            ui.add_space(8.0);
            add_contents(ui)
        })
        .inner
}

fn mpc_button(
    ui: &mut egui::Ui,
    label: &str,
    size: [f32; 2],
    fill: egui::Color32,
    selected: bool,
) -> egui::Response {
    let base_fill = fill;
    let fill = if selected { mpc_color::ACTIVE } else { fill };
    let text_color = if selected || !is_light_button_fill(base_fill) {
        mpc_color::BUTTON_TEXT
    } else {
        mpc_color::INK
    };
    let stroke = if selected {
        egui::Stroke::new(2.0, mpc_color::LCD)
    } else {
        egui::Stroke::new(1.25, mpc_color::BLACK)
    };
    ui.add_sized(
        size,
        egui::Button::new(
            egui::RichText::new(label)
                .size(12.0)
                .strong()
                .color(text_color),
        )
        .fill(fill)
        .stroke(stroke)
        .corner_radius(3),
    )
}

fn is_light_button_fill(fill: egui::Color32) -> bool {
    fill == mpc_color::CREAM_KEY || fill == mpc_color::RIGHT_BAY || fill == mpc_color::CASE_PANEL
}

fn mpc_pad(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let fill = if selected {
        mpc_color::PAD_SELECTED
    } else {
        mpc_color::PAD
    };
    ui.add_sized(
        [92.0, 72.0],
        egui::Button::new(
            egui::RichText::new(label)
                .size(16.0)
                .strong()
                .color(mpc_color::PAD_TEXT),
        )
        .fill(fill)
        .stroke(egui::Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected {
                mpc_color::LCD
            } else {
                mpc_color::BLACK
            },
        ))
        .corner_radius(7),
    )
}

fn mpc_knob(ui: &mut egui::Ui, label: &str) {
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.circle_filled(rect.center(), 29.0, mpc_color::WHEEL);
        painter.circle_stroke(
            rect.center(),
            29.0,
            egui::Stroke::new(2.0, mpc_color::BLACK),
        );
        painter.circle_stroke(
            rect.center(),
            18.0,
            egui::Stroke::new(1.0, mpc_color::WHEEL_RING),
        );
        painter.line_segment(
            [
                rect.center() + egui::vec2(0.0, -5.0),
                rect.center() + egui::vec2(0.0, -22.0),
            ],
            egui::Stroke::new(2.0, mpc_color::BUTTON_TEXT),
        );
        ui.label(
            egui::RichText::new(label)
                .size(10.0)
                .strong()
                .color(mpc_color::FADED_INK),
        );
    });
}

fn compact_io_text(
    audio: HostAudioState,
    midi: HostMidiState,
    runtime_sample_status: &str,
) -> String {
    format!(
        "audio {} voices {}/{} | midi {} queued {} | {}",
        audio.backend_name,
        audio.active_voice_count,
        audio.voice_limit,
        midi.backend_name,
        midi.queued_message_count,
        runtime_sample_status
    )
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
        HostMidiEvent::Failed { error, .. } => Some(format!("Host MIDI failed: {error}")),
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

fn main_host_audio_text(state: HostAudioState, backend: &DesktopAudioBackend) -> String {
    if !matches!(state.mode, mpc_audio::HostAudioMode::Enabled) {
        return "AUDIO off".to_string();
    }

    match backend.device_status() {
        Some(status) => format!(
            "AUDIO device q {}/{} cb {} err {}",
            status.queued_frame_count,
            status.max_queued_frame_count,
            status.total_callback_frame_count,
            status.recent_stream_errors.len()
        ),
        None => format!(
            "AUDIO capture/silent queued {} played {}",
            state.queued_render_count, state.played_render_count
        ),
    }
}

fn host_audio_backend_detail_text(backend: &DesktopAudioBackend) -> String {
    match backend.device_status() {
        Some(status) => device_audio_backend_status_text(&status),
        None => "Host audio backend detail: capture retains summaries only".to_string(),
    }
}

fn device_audio_backend_status_text(status: &DeviceAudioBackendStatus) -> String {
    let stream_errors = status.recent_stream_errors.len();
    format!(
        "Host audio device: {} {} Hz {} ch {} queued {}/{} cb {} underrun {} errors {}",
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
        None => "Host MIDI output: capture retains note-on messages only".to_string(),
    }
}

fn device_midi_output_status_text(status: &DeviceMidiOutputStatus) -> String {
    match &status.last_sent_message {
        Some(message) => format!(
            "Host MIDI output device: {} sent {} last ch {} note {} vel {}",
            midi_port_label(&status.output_port),
            status.total_sent_message_count,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> MpcDesktopApp {
        MpcDesktopApp {
            core: MpcCore::new(),
            host_audio: HostAudioEngine::enabled(
                DesktopAudioBackend::capture(),
                AudioRenderSettings::new(44_100, 4_096)
                    .expect("test render settings should be valid"),
            )
            .expect("test host audio should construct"),
            host_midi: HostMidiEngine::enabled(DesktopMidiBackend::capture()),
            host_midi_input: None,
            midi_input_ports: Vec::new(),
            midi_output_ports: Vec::new(),
            selected_midi_input_port: 0,
            selected_midi_output_port: 0,
            last_status: "Ready".to_string(),
            last_audio_render: None,
            last_audio_render_error: None,
            runtime_samples: RuntimeSampleLibrary::default(),
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
            full_level_enabled: false,
            sixteen_levels_enabled: false,
            numeric_entry: String::new(),
            last_transport_tick: Instant::now(),
            last_tap_tick: None,
        }
    }

    fn captured_render_summaries(app: &MpcDesktopApp) -> Vec<AudioRenderSummary> {
        match app.host_audio.backend() {
            DesktopAudioBackend::Capture(backend) => backend
                .captured_renders()
                .iter()
                .map(|capture| capture.summary.clone())
                .collect(),
            DesktopAudioBackend::Device(_) => panic!("test app should use capture audio"),
        }
    }

    #[test]
    fn desktop_recorded_sequence_reaches_host_audio_on_playback() {
        let mut app = test_app();

        app.record_start();
        app.strike_panel_pad(PadBank::A, 1);

        assert_eq!(app.core.state().recorded_events.len(), 1);
        assert!(app.core.state().playing);
        assert!(app.core.state().recording);

        let live_sample_renders = captured_render_summaries(&app)
            .into_iter()
            .filter(|summary| summary.render_kind == AudioRenderKind::SamplePlayback)
            .collect::<Vec<_>>();
        assert_eq!(live_sample_renders.len(), 1);
        assert_eq!(live_sample_renders[0].source_sample_id, "synthetic_a_01");

        app.dispatch_event(HardwareEvent::Press {
            control: PanelControl::Stop,
        });
        app.play_start();
        app.dispatch_transport_tick(250_000);

        let sample_renders = captured_render_summaries(&app)
            .into_iter()
            .filter(|summary| summary.render_kind == AudioRenderKind::SamplePlayback)
            .collect::<Vec<_>>();
        assert_eq!(sample_renders.len(), 2);
        assert_eq!(sample_renders[1].source_sample_id, "synthetic_a_01");
        assert!(app.last_status.starts_with("Played Trk 01 A01"));
    }

    #[test]
    fn desktop_tap_tick_outputs_click_and_sets_tempo_on_second_tap() {
        let mut app = test_app();

        app.tap_tick();
        let first_renders = captured_render_summaries(&app);
        assert_eq!(first_renders.len(), 1);
        assert_eq!(first_renders[0].render_kind, AudioRenderKind::CountInClick);
        assert_eq!(app.last_status, "Tap tick: waiting for next tap");

        app.last_tap_tick = Some(
            Instant::now()
                .checked_sub(Duration::from_millis(500))
                .expect("test tap instant should be representable"),
        );
        app.tap_tick();

        let renders = captured_render_summaries(&app);
        assert_eq!(renders.len(), 2);
        assert_eq!(renders[1].render_kind, AudioRenderKind::CountInClick);
        assert_eq!(app.core.state().tempo_bpm_x100, 12_000);
        assert_eq!(app.last_status, "Tap tempo set 120.00 BPM");
    }
}
