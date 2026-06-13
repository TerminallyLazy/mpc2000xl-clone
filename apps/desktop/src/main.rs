use eframe::egui;
use mpc_audio::{
    AudioRenderSettings, AudioRenderSummary, CaptureAudioBackend, HostAudioEngine, HostAudioError,
    HostAudioEvent, HostAudioState,
};
use mpc_core::{
    HardwareEvent, MachineOutput, Mode, MpcCore, MpcState, PadAssignmentChange, PadBank,
    PanelControl, ProgramPad, SamplePlaybackResolution,
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
    last_status: String,
    last_synthetic_render: Option<AudioRenderSummary>,
    last_synthetic_render_error: Option<String>,
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
            last_status: "Ready".to_string(),
            last_synthetic_render: None,
            last_synthetic_render_error: None,
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
            self.draw_sequence_status(ui);
            self.draw_program_status(ui);
            self.draw_audio_render_status(ui);
            self.draw_host_audio_status(ui);
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
        let render_or_host_error = self.handle_last_playback_intent(&outputs);
        self.last_status = render_or_host_error
            .unwrap_or_else(|| Self::status_from_outputs(&outputs, self.core.state()));
    }

    fn handle_last_playback_intent(&mut self, outputs: &[MachineOutput]) -> Option<String> {
        if let Some(MachineOutput::SamplePlaybackIntent { intent }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SamplePlaybackIntent { .. }))
        {
            let report = self.host_audio.play_intent_with_render_summary(intent);
            self.last_synthetic_render = report.render_summary;
            self.last_synthetic_render_error = None;

            if let HostAudioEvent::Failed {
                error: HostAudioError::Render { error },
                ..
            } = &report.event
            {
                let message = format!("Synthetic render failed: {error}");
                self.last_synthetic_render_error = Some(message.clone());
                return Some(message);
            }

            host_audio_error_message(&report.event)
        } else {
            None
        }
    }

    fn status_from_outputs(outputs: &[MachineOutput], state: &MpcState) -> String {
        if let Some(MachineOutput::Ignored { reason }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::Ignored { .. }))
        {
            return format!("Ignored: {reason}");
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

        if let Some(MachineOutput::SequenceEventRecorded { event }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SequenceEventRecorded { .. }))
        {
            let sample = event
                .playback
                .as_ref()
                .map(|intent| format!(" sample {}", intent.sample_name))
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

        if let Some(MachineOutput::SamplePlaybackIntent { intent }) = outputs
            .iter()
            .find(|output| matches!(output, MachineOutput::SamplePlaybackIntent { .. }))
        {
            return format!(
                "Playback intent Trk {:02} Pgm {:02} {:?}{:02} {} velocity {}",
                intent.selected_track,
                intent.program_index,
                intent.bank,
                intent.pad_number,
                intent.sample_name,
                intent.velocity
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
            if ui.button("TICK +100ms").clicked() {
                self.dispatch_event(HardwareEvent::Tick { micros: 100_000 });
            }
        });
    }

    fn draw_sequence_status(&self, ui: &mut egui::Ui) {
        let state = self.core.state();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "Transport: playing={} recording={}",
                state.playing, state.recording
            ));
            ui.separator();
            ui.label(format!("Playhead: {} ticks", state.playhead_ticks));
            ui.separator();
            ui.label(format!("Recorded events: {}", state.recorded_events.len()));
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
            ui.label(format!("Selected pad: {}", program_pad_label(selected_pad)));
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

    fn draw_pads(&mut self, ui: &mut egui::Ui) {
        let selected_program_pad = self.core.state().selected_program_pad;
        let program_mode = self.core.state().mode == Mode::Program;
        egui::Grid::new("pads")
            .num_columns(4)
            .spacing([10.0, 10.0])
            .show(ui, |ui| {
                for pad in 1..=16 {
                    let pad_address = ProgramPad {
                        bank: PadBank::A,
                        pad_number: pad,
                    };
                    let selected = program_mode && selected_program_pad == pad_address;
                    if ui
                        .selectable_label(selected, format!("PAD {pad:02}"))
                        .clicked()
                    {
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
            "LCD updated: {} focus, Seq {:02}, Trk {:02}, {}, Tempo {}, Bars {:03}, Tick {}, Events {}",
            state.selected_main_field.label(),
            state.sequence_index,
            state.selected_track,
            state.current_program.name,
            tempo_text(state.tempo_bpm_x100),
            state.bar_count,
            state.playhead_ticks,
            state.recorded_events.len()
        ),
        Mode::Program => format!(
            "LCD updated: PROGRAM {}, {}",
            program_pad_label(state.selected_program_pad),
            selected_assignment_text(state)
        ),
        mode => format!("LCD updated: {mode:?}"),
    }
}

fn tempo_text(tempo_bpm_x100: u32) -> String {
    format!("{}.{:02} BPM", tempo_bpm_x100 / 100, tempo_bpm_x100 % 100)
}

fn assignment_action_text(action: PadAssignmentChange) -> &'static str {
    match action {
        PadAssignmentChange::Cleared => "cleared",
        PadAssignmentChange::Restored => "assigned",
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
            "Assignment: {} level {} pan {}",
            assignment.sample.name, assignment.level, assignment.pan
        ),
        None => "Assignment: unassigned".to_string(),
    }
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
            "Last playback: {} {} vel {}",
            program_pad_label(ProgramPad {
                bank: intent.bank,
                pad_number: intent.pad_number,
            }),
            intent.sample_name,
            intent.velocity
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

fn last_synthetic_render_text(summary: Option<&AudioRenderSummary>, error: Option<&str>) -> String {
    match (summary, error) {
        (_, Some(error)) => error.to_string(),
        (Some(summary), None) => format!(
            "Synthetic render: {} {} frames @ {} Hz peak L{} R{} balance {:?}",
            summary.source_sample_name,
            summary.frame_count,
            summary.sample_rate_hz,
            summary.peak_left,
            summary.peak_right,
            summary.channel_balance
        ),
        (None, None) => "Synthetic render: none".to_string(),
    }
}

fn host_audio_error_message(event: &HostAudioEvent) -> Option<String> {
    match event {
        HostAudioEvent::Failed { error, .. } => Some(format!("Host audio failed: {error}")),
        HostAudioEvent::Ignored { .. } | HostAudioEvent::Enqueued { .. } => None,
    }
}

fn host_audio_state_text(state: &HostAudioState) -> String {
    format!(
        "Host audio: {:?} backend {} queued {} played {}",
        state.mode, state.backend_name, state.queued_render_count, state.played_render_count
    )
}

fn last_host_audio_event_text(event: Option<&HostAudioEvent>) -> String {
    match event {
        Some(HostAudioEvent::Ignored { reason, .. }) => {
            format!("Host audio event: ignored {reason:?}")
        }
        Some(HostAudioEvent::Enqueued { receipt, .. }) => format!(
            "Host audio event: {} {} frames queued={} played={}",
            receipt.summary.source_sample_name, receipt.frame_count, receipt.queued, receipt.played
        ),
        Some(HostAudioEvent::Failed { error, summary, .. }) => {
            let sample = summary
                .as_ref()
                .map(|summary| summary.source_sample_name.as_str())
                .unwrap_or("no render");
            format!("Host audio event: failed {sample}: {error}")
        }
        None => "Host audio event: none".to_string(),
    }
}
