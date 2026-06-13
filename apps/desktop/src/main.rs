use eframe::egui;
use mpc_core::{HardwareEvent, Mode, MpcCore, PadBank, PanelControl};

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
            self.draw_transport(ui);
            ui.add_space(16.0);
            self.draw_pads(ui);
            ui.add_space(16.0);
            ui.label(format!("Status: {}", self.last_status));
        });
    }
}

impl MpcDesktopApp {
    fn draw_lcd(&self, ui: &mut egui::Ui) {
        let lcd = &self.core.state().lcd;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(720.0);
            ui.heading(&lcd.title);
            for line in &lcd.lines {
                ui.monospace(line);
            }
            ui.horizontal_wrapped(|ui| {
                for soft_key in &lcd.soft_keys {
                    ui.label(format!("[{soft_key}]"));
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
            self.core.dispatch(HardwareEvent::Press { control });
            self.last_status = format!("Mode changed to {mode:?}");
        }
    }

    fn draw_transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("STOP").clicked() {
                self.core.dispatch(HardwareEvent::Press {
                    control: PanelControl::Stop,
                });
                self.last_status = "Transport stopped".to_string();
            }
            if ui.button("PLAY").clicked() {
                self.core.dispatch(HardwareEvent::Press {
                    control: PanelControl::Play,
                });
                self.last_status = "Transport playing".to_string();
            }
            if ui.button("REC").clicked() {
                self.core.dispatch(HardwareEvent::Press {
                    control: PanelControl::Rec,
                });
                self.last_status = "Record armed".to_string();
            }
            if ui.button("OVERDUB").clicked() {
                self.core.dispatch(HardwareEvent::Press {
                    control: PanelControl::Overdub,
                });
                self.last_status = "Overdub active".to_string();
            }
            if ui.button("Tempo -").clicked() {
                self.core
                    .dispatch(HardwareEvent::TurnDataWheel { delta: -1 });
            }
            if ui.button("Tempo +").clicked() {
                self.core
                    .dispatch(HardwareEvent::TurnDataWheel { delta: 1 });
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
                        self.core.dispatch(HardwareEvent::StrikePad {
                            bank: PadBank::A,
                            pad,
                            velocity: 100,
                        });
                        self.last_status = format!("Pad A{pad:02} triggered");
                    }
                    if pad % 4 == 0 {
                        ui.end_row();
                    }
                }
            });
    }
}
