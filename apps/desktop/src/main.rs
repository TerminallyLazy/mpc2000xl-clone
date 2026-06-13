use eframe::egui;
use mpc_core::{HardwareEvent, MachineOutput, Mode, MpcCore, MpcState, PadBank, PanelControl};

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
    last_status: String,
}

impl Default for MpcDesktopApp {
    fn default() -> Self {
        Self {
            core: MpcCore::new(),
            last_status: "Ready".to_string(),
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
            self.draw_transport(ui);
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

    fn dispatch_event(&mut self, event: HardwareEvent) {
        let outputs = self.core.dispatch(event);
        self.last_status = Self::status_from_outputs(&outputs, self.core.state());
    }

    fn status_from_outputs(outputs: &[MachineOutput], state: &MpcState) -> String {
        if let Some(MachineOutput::Ignored { reason }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::Ignored { .. }))
        {
            return format!("Ignored: {reason}");
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
        });
    }

    fn draw_pads(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("pads")
            .num_columns(4)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                for pad in 1..=16 {
                    if ui.button(format!("PAD {pad:02}")).clicked() {
                        self.dispatch_event(HardwareEvent::StrikePad {
                            bank: PadBank::A,
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
}

fn main_screen_status(state: &MpcState) -> String {
    match state.mode {
        Mode::Main => format!(
            "LCD updated: {} focus, Seq {:02}, Trk {:02}, Tempo {}, Bars {:03}",
            state.selected_main_field.label(),
            state.sequence_index,
            state.selected_track,
            tempo_text(state.tempo_bpm_x100),
            state.bar_count
        ),
        mode => format!("LCD updated: {mode:?}"),
    }
}

fn tempo_text(tempo_bpm_x100: u32) -> String {
    format!("{}.{:02} BPM", tempo_bpm_x100 / 100, tempo_bpm_x100 % 100)
}
