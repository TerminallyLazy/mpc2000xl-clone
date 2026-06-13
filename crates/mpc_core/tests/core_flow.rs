use mpc_core::{
    HardwareEvent, INTERNAL_PPQN, MachineOutput, MainScreenField, Mode, MpcCore,
    PROJECT_SNAPSHOT_VERSION, PadAssignmentChange, PadBank, PanelControl, PlaybackMissReason,
    ProgramEditField, ProgramPad, ProjectSnapshot, ProjectSnapshotError, SamplePlaybackIntent,
    SamplePlaybackResolution, SequenceEvent,
};

#[test]
fn core_starts_on_main_screen() {
    let core = MpcCore::new();

    assert_eq!(core.state().mode, Mode::Main);
    assert_eq!(core.state().lcd.title, "MAIN");
    assert_eq!(core.state().sequence_index, 1);
    assert_eq!(core.state().sequence_name, "Sequence01");
    assert_eq!(core.state().selected_track, 1);
    assert_eq!(core.state().bar_count, 1);
    assert_eq!(core.state().selected_main_field, MainScreenField::Tempo);
    assert_eq!(core.state().playhead_ticks, 0);
    assert_eq!(core.state().recorded_events, Vec::new());
    assert_eq!(core.state().current_program.index, 1);
    assert_eq!(core.state().current_program.name, "Program01");
    assert_eq!(core.state().current_program.pad_assignments.len(), 16);
    assert_eq!(
        core.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::A,
            pad_number: 1
        }
    );
    assert_eq!(core.state().last_playback, None);
    assert_eq!(
        core.state().selected_program_edit_field,
        ProgramEditField::Pad
    );
    assert!(!core.state().playing);
}

#[test]
fn mode_button_changes_lcd_screen() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });

    assert_eq!(core.state().mode, Mode::Program);
    assert_eq!(core.state().lcd.title, "PROGRAM");
    assert!(outputs.iter().any(|output| matches!(
        output,
        mpc_core::MachineOutput::ModeChanged {
            mode: Mode::Program
        }
    )));
}

#[test]
fn transport_buttons_update_play_state() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    assert!(core.state().playing);
    assert!(core.state().lcd.lines[2].contains("PLAY"));

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });
    assert!(!core.state().playing);
    assert!(!core.state().recording);
    assert!(core.state().lcd.lines[2].contains("STOP"));
}

#[test]
fn valid_pad_strike_is_reported() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::B,
        pad: 12,
        velocity: 96,
    });

    assert_eq!(core.state().pad_bank, PadBank::B);
    assert!(outputs.iter().any(|output| matches!(
        output,
        mpc_core::MachineOutput::PadTriggered {
            bank: PadBank::B,
            pad: 12,
            velocity: 96
        }
    )));
}

#[test]
fn default_program_assigns_bank_a_to_synthetic_samples() {
    let core = MpcCore::new();

    assert_eq!(core.state().current_program.pad_assignments.len(), 16);
    for pad_number in 1..=16 {
        let assignment = core
            .state()
            .current_program
            .pad_assignments
            .iter()
            .find(|assignment| {
                assignment.pad
                    == ProgramPad {
                        bank: PadBank::A,
                        pad_number,
                    }
            })
            .expect("bank A pad should have a default synthetic assignment");
        assert_eq!(assignment.sample.id, format!("synthetic_a_{pad_number:02}"));
        assert_eq!(assignment.sample.name, format!("SYN-A{pad_number:02}"));
        assert_eq!(assignment.level, 100);
        assert_eq!(assignment.pan, 0);
        assert_eq!(assignment.tune_cents, 0);
    }
}

#[test]
fn assigned_pad_strike_emits_sample_playback_intent() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 5,
        velocity: 101,
    });

    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::PadTriggered {
            bank: PadBank::A,
            pad: 5,
            velocity: 101
        }
    )));
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackIntent { intent }
            if intent.bank == PadBank::A
                && intent.pad_number == 5
                && intent.sample_id == "synthetic_a_05"
                && intent.sample_name == "SYN-A05"
                && intent.velocity == 101
                && intent.selected_track == 1
                && intent.program_index == 1
                && intent.program_name == "Program01"
                && intent.level == 100
                && intent.pan == 0
                && intent.tune_cents == 0
    )));
    assert!(matches!(
        &core.state().last_playback,
        Some(SamplePlaybackResolution::Intent { intent })
            if intent.sample_id == "synthetic_a_05" && intent.velocity == 101
    ));
}

#[test]
fn stopped_or_armed_pad_strikes_do_not_record_sequence_events() {
    let mut core = MpcCore::new();

    let stopped_outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 90,
    });
    assert!(
        stopped_outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::PadTriggered { .. }))
    );
    assert!(core.state().recorded_events.is_empty());

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    assert!(core.state().recording);
    assert!(!core.state().playing);

    let armed_outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::B,
        pad: 2,
        velocity: 91,
    });
    assert!(
        !armed_outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::SequenceEventRecorded { .. }))
    );
    assert!(core.state().recorded_events.is_empty());
}

#[test]
fn rec_then_play_records_pad_strike_as_sequence_event() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    assert!(core.state().recording);
    assert!(!core.state().playing);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::B,
        pad: 12,
        velocity: 96,
    });

    let expected = SequenceEvent {
        selected_track: 1,
        pad_bank: PadBank::B,
        pad_number: 12,
        velocity: 96,
        tick: 0,
        playback: None,
    };
    assert!(core.state().playing);
    assert!(core.state().recording);
    assert_eq!(core.state().recorded_events, vec![expected.clone()]);
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SequenceEventRecorded { event } if event == &expected
    )));
    assert!(core.state().lcd.lines[3].contains("E001"));
}

#[test]
fn overdub_starts_playback_and_records_pad_strike() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Overdub,
    });
    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::C,
        pad: 4,
        velocity: 64,
    });

    assert!(core.state().playing);
    assert!(core.state().recording);
    assert_eq!(core.state().recorded_events.len(), 1);
    assert_eq!(core.state().recorded_events[0].pad_bank, PadBank::C);
    assert_eq!(core.state().recorded_events[0].pad_number, 4);
    assert_eq!(core.state().recorded_events[0].velocity, 64);
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SequenceEventRecorded { event }
            if event.pad_bank == PadBank::C && event.pad_number == 4
    )));
}

#[test]
fn tick_advances_playhead_only_while_playing() {
    let mut core = MpcCore::new();

    let stopped_outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    assert!(stopped_outputs.is_empty());
    assert_eq!(core.state().playhead_ticks, 0);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let playing_outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    assert_eq!(core.state().playhead_ticks, u64::from(INTERNAL_PPQN));
    assert_eq!(playing_outputs, vec![MachineOutput::LcdChanged]);
    assert!(core.state().lcd.lines[3].contains("T000096"));

    core.dispatch(HardwareEvent::Tick { micros: u64::MAX });
    assert!(core.state().playhead_ticks > u64::from(INTERNAL_PPQN));

    let after_large_tick = core.state().playhead_ticks;
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    assert_eq!(core.state().playhead_ticks, after_large_tick);
}

#[test]
fn sequence_playback_schedules_recorded_assigned_event_on_crossing_tick() {
    let snapshot = snapshot_with_recorded_assigned_events_at_tick(&[(2, 82)]);
    let recorded_event = snapshot.sequence.recorded_events[0].clone();
    let mut core = restore_snapshot(snapshot);

    assert_eq!(core.state().playhead_ticks, 0);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    let intents = playback_intents(&outputs);

    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SequenceEventPlayed { event } if event == &recorded_event
    )));
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].sample_id, "synthetic_a_02");
    assert_eq!(intents[0].sample_name, "SYN-A02");
    assert_eq!(intents[0].velocity, 82);
    assert_eq!(intents[0].selected_track, 1);
    assert_eq!(
        core.state().last_playback,
        Some(SamplePlaybackResolution::Intent {
            intent: (*intents[0]).clone()
        })
    );
}

#[test]
fn sequence_playback_schedules_multiple_recorded_events_in_recorded_order() {
    let snapshot = snapshot_with_recorded_assigned_events_at_tick(&[(2, 70), (5, 88)]);
    let mut core = restore_snapshot(snapshot);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });

    let played_pads = outputs
        .iter()
        .filter_map(|output| match output {
            MachineOutput::SequenceEventPlayed { event } => Some(event.pad_number),
            _ => None,
        })
        .collect::<Vec<_>>();
    let scheduled_sample_ids = playback_intents(&outputs)
        .iter()
        .map(|intent| intent.sample_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(played_pads, vec![2, 5]);
    assert_eq!(
        scheduled_sample_ids,
        vec!["synthetic_a_02", "synthetic_a_05"]
    );
    assert!(matches!(
        &core.state().last_playback,
        Some(SamplePlaybackResolution::Intent { intent })
            if intent.sample_id == "synthetic_a_05" && intent.velocity == 88
    ));
}

#[test]
fn sequence_playback_does_not_retrigger_on_zero_or_non_advancing_ticks() {
    let snapshot = snapshot_with_recorded_assigned_events_at_tick(&[(3, 91)]);
    let mut core = restore_snapshot(snapshot);

    let stopped_outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    assert!(stopped_outputs.is_empty());
    assert_eq!(core.state().playhead_ticks, 0);
    assert_eq!(core.state().last_playback, None);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });

    let zero_outputs = core.dispatch(HardwareEvent::Tick { micros: 0 });
    assert!(playback_intents(&zero_outputs).is_empty());
    assert_eq!(core.state().playhead_ticks, 0);

    let non_advancing_outputs = core.dispatch(HardwareEvent::Tick { micros: 1 });
    assert!(playback_intents(&non_advancing_outputs).is_empty());
    assert_eq!(core.state().playhead_ticks, 0);

    let crossing_outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    assert_eq!(playback_intents(&crossing_outputs).len(), 1);
    assert_eq!(core.state().playhead_ticks, u64::from(INTERNAL_PPQN));

    let same_position_outputs = core.dispatch(HardwareEvent::Tick { micros: 0 });
    assert!(playback_intents(&same_position_outputs).is_empty());

    let playhead_after_crossing = core.state().playhead_ticks;
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });
    let stopped_after_playback_outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    assert!(stopped_after_playback_outputs.is_empty());
    assert_eq!(core.state().playhead_ticks, playhead_after_crossing);
}

#[test]
fn sequence_playback_uses_recorded_metadata_after_program_assignment_is_cleared() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 4,
        velocity: 70,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 4,
        velocity: 99,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(1),
    });

    let mut snapshot = core.export_project_snapshot();
    reset_snapshot_playhead(&mut snapshot, 0);
    let mut restored = restore_snapshot(snapshot);

    assert!(restored.state().current_program.pad_assignments.iter().all(
        |assignment| assignment.pad
            != ProgramPad {
                bank: PadBank::A,
                pad_number: 4,
            }
    ));

    restored.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = restored.dispatch(HardwareEvent::Tick { micros: 500_000 });
    let intents = playback_intents(&outputs);

    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].sample_id, "synthetic_a_04");
    assert_eq!(intents[0].sample_name, "SYN-A04");
    assert_eq!(intents[0].velocity, 70);
    assert!(matches!(
        &restored.state().last_playback,
        Some(SamplePlaybackResolution::Intent { intent })
            if intent.sample_id == "synthetic_a_04" && intent.velocity == 70
    ));
}

#[test]
fn stop_disarms_recording() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Overdub,
    });
    assert!(core.state().playing);
    assert!(core.state().recording);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });
    assert!(!core.state().playing);
    assert!(!core.state().recording);

    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 100,
    });
    assert!(core.state().recorded_events.is_empty());
}

#[test]
fn transport_outputs_include_lcd_change_after_state_change() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });

    assert_eq!(
        outputs,
        vec![
            mpc_core::MachineOutput::TransportChanged {
                playing: true,
                recording: false,
            },
            mpc_core::MachineOutput::LcdChanged,
        ]
    );
}

#[test]
fn tempo_adjustment_clamps_extreme_deltas_without_overflow() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MAX });
    assert_eq!(core.state().tempo_bpm_x100, 30000);

    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MIN });
    assert_eq!(core.state().tempo_bpm_x100, 3000);
}

#[test]
fn main_screen_cursor_left_and_right_move_focus_and_refresh_lcd() {
    let mut core = MpcCore::new();

    let left_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });

    assert_eq!(core.state().selected_main_field, MainScreenField::Track);
    assert_eq!(left_outputs, vec![MachineOutput::LcdChanged]);
    assert!(core.state().lcd.lines[1].starts_with(">Trk"));

    let right_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });

    assert_eq!(core.state().selected_main_field, MainScreenField::Tempo);
    assert_eq!(right_outputs, vec![MachineOutput::LcdChanged]);
    assert!(core.state().lcd.lines[2].starts_with(">Tempo"));
}

#[test]
fn data_wheel_edits_selected_main_screen_field() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::TurnDataWheel { delta: 2 });
    assert_eq!(core.state().tempo_bpm_x100, 12200);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 2 });
    assert_eq!(core.state().selected_track, 3);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 });
    assert_eq!(core.state().sequence_index, 5);
    assert_eq!(core.state().sequence_name, "Sequence05");

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 6 });
    assert_eq!(core.state().bar_count, 7);
    assert!(core.state().lcd.lines[3].contains("Bars 007"));
}

#[test]
fn main_screen_edit_fields_clamp_to_foundation_ranges() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MAX });
    assert_eq!(core.state().selected_track, 64);
    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MIN });
    assert_eq!(core.state().selected_track, 1);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MAX });
    assert_eq!(core.state().sequence_index, 99);
    assert_eq!(core.state().sequence_name, "Sequence99");
    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MIN });
    assert_eq!(core.state().sequence_index, 1);
    assert_eq!(core.state().sequence_name, "Sequence01");

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MAX });
    assert_eq!(core.state().bar_count, 999);
    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MIN });
    assert_eq!(core.state().bar_count, 1);
}

#[test]
fn main_screen_track_soft_keys_change_track_or_report_structured_ignore() {
    let mut core = MpcCore::new();

    let increment = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert_eq!(increment, vec![MachineOutput::LcdChanged]);
    assert_eq!(core.state().selected_main_field, MainScreenField::Track);
    assert_eq!(core.state().selected_track, 2);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(3),
    });
    assert_eq!(core.state().selected_track, 1);

    let unsupported = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(1),
    });
    assert_eq!(
        unsupported,
        vec![MachineOutput::Ignored {
            reason: "main_screen.soft_key.1_unimplemented".to_string(),
        }]
    );
}

#[test]
fn program_mode_soft_key_clear_makes_selected_pad_unassigned() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    let clear_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(1),
    });
    let strike_outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 88,
    });

    assert_eq!(core.state().mode, Mode::Program);
    assert_eq!(
        core.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::A,
            pad_number: 1
        }
    );
    assert_eq!(core.state().current_program.pad_assignments.len(), 15);
    assert!(clear_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::PadAssignmentChanged {
            bank: PadBank::A,
            pad: 1,
            action: PadAssignmentChange::Cleared,
            assignment: None,
        }
    )));
    assert!(strike_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::PadTriggered {
            bank: PadBank::A,
            pad: 1,
            velocity: 88
        }
    )));
    assert!(strike_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackMiss { miss }
            if miss.bank == PadBank::A
                && miss.pad_number == 1
                && miss.velocity == 88
                && miss.reason == PlaybackMissReason::PadUnassigned
    )));
    assert!(core.state().lcd.lines[1].contains("unassigned"));
    assert!(matches!(
        &core.state().last_playback,
        Some(SamplePlaybackResolution::Miss { miss })
            if miss.reason == PlaybackMissReason::PadUnassigned
    ));
}

#[test]
fn program_mode_soft_key_reassign_restores_generated_assignment() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 7,
        velocity: 90,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(1),
    });
    assert_eq!(core.state().current_program.pad_assignments.len(), 15);

    let restore_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });

    assert_eq!(
        core.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::A,
            pad_number: 7
        }
    );
    assert_eq!(core.state().current_program.pad_assignments.len(), 16);
    assert!(restore_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::PadAssignmentChanged {
            bank: PadBank::A,
            pad: 7,
            action: PadAssignmentChange::Restored,
            assignment: Some(assignment),
        } if assignment.sample.id == "synthetic_a_07"
            && assignment.sample.name == "SYN-A07"
            && assignment.level == 100
            && assignment.pan == 0
            && assignment.tune_cents == 0
    )));
    assert!(core.state().lcd.lines[1].contains("SYN-A07"));
}

#[test]
fn program_mode_pad_strike_selects_pad_and_triggers_assignment() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 9,
        velocity: 93,
    });

    assert_eq!(
        core.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::A,
            pad_number: 9
        }
    );
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackIntent { intent }
            if intent.sample_id == "synthetic_a_09" && intent.velocity == 93
    )));
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::LcdChanged))
    );
    assert!(core.state().lcd.lines[1].contains("SYN-A09"));
}

#[test]
fn program_parameter_cursor_up_down_cycles_edit_field_and_lcd_reflects_it() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    assert_eq!(
        core.state().selected_program_edit_field,
        ProgramEditField::Pad
    );
    assert!(core.state().lcd.lines[0].contains("Edit pad"));
    assert!(core.state().lcd.lines[1].starts_with(">Pad"));

    let level_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    assert_eq!(level_outputs, vec![MachineOutput::LcdChanged]);
    assert_eq!(
        core.state().selected_program_edit_field,
        ProgramEditField::Level
    );
    assert!(core.state().lcd.lines[3].contains(">Level"));

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    assert_eq!(
        core.state().selected_program_edit_field,
        ProgramEditField::Pan
    );
    assert!(core.state().lcd.lines[3].contains(">Pan"));

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    assert_eq!(
        core.state().selected_program_edit_field,
        ProgramEditField::Tune
    );
    assert!(core.state().lcd.lines[3].contains(">Tune"));

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    assert_eq!(
        core.state().selected_program_edit_field,
        ProgramEditField::Pad
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorUp,
    });
    assert_eq!(
        core.state().selected_program_edit_field,
        ProgramEditField::Tune
    );
}

#[test]
fn program_parameter_data_wheel_edits_level_pan_tune_with_clamping() {
    let mut core = MpcCore::new();
    let pad = ProgramPad {
        bank: PadBank::A,
        pad_number: 1,
    };

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });

    let level_outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 40 });
    assert!(matches!(
        level_outputs.as_slice(),
        [
            MachineOutput::PadParameterChanged {
                parameter: ProgramEditField::Level,
                value: 127,
                ..
            },
            MachineOutput::LcdChanged,
        ]
    ));
    assert_eq!(
        assignment_for_pad(&core, pad)
            .expect("selected pad should be assigned")
            .level,
        127
    );
    core.dispatch(HardwareEvent::TurnDataWheel { delta: -200 });
    assert_eq!(
        assignment_for_pad(&core, pad)
            .expect("selected pad should be assigned")
            .level,
        0
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 80 });
    assert_eq!(
        assignment_for_pad(&core, pad)
            .expect("selected pad should be assigned")
            .pan,
        50
    );
    core.dispatch(HardwareEvent::TurnDataWheel { delta: -200 });
    assert_eq!(
        assignment_for_pad(&core, pad)
            .expect("selected pad should be assigned")
            .pan,
        -50
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    let tune_outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 20 });
    assert!(matches!(
        tune_outputs.as_slice(),
        [
            MachineOutput::PadParameterChanged {
                parameter: ProgramEditField::Tune,
                value: 1200,
                ..
            },
            MachineOutput::LcdChanged,
        ]
    ));
    assert_eq!(
        assignment_for_pad(&core, pad)
            .expect("selected pad should be assigned")
            .tune_cents,
        1200
    );
    core.dispatch(HardwareEvent::TurnDataWheel { delta: -30 });
    assert_eq!(
        assignment_for_pad(&core, pad)
            .expect("selected pad should be assigned")
            .tune_cents,
        -1200
    );
}

#[test]
fn program_parameter_unassigned_pad_edit_returns_structured_ignore_without_assignment() {
    let mut core = MpcCore::new();
    let pad = ProgramPad {
        bank: PadBank::A,
        pad_number: 1,
    };

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(1),
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    let outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 7 });

    assert_eq!(
        outputs,
        vec![MachineOutput::Ignored {
            reason: "program.level.unassigned_a01".to_string(),
        }]
    );
    assert!(assignment_for_pad(&core, pad).is_none());
}

#[test]
fn program_parameter_pad_strike_playback_intent_carries_edited_values() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 7 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: -12 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 3 });

    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 88,
    });
    let intent = playback_intents(&outputs)
        .into_iter()
        .next()
        .expect("assigned pad should emit playback intent");

    assert_eq!(intent.level, 107);
    assert_eq!(intent.pan, -12);
    assert_eq!(intent.tune_cents, 300);
}

#[test]
fn program_parameter_recording_snapshots_edited_values_and_replays_stored_metadata() {
    let mut core = MpcCore::new();
    let pad = ProgramPad {
        bank: PadBank::A,
        pad_number: 2,
    };

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 2,
        velocity: 90,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: -10 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: -7 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 5 });

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 2,
        velocity: 80,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });

    core.dispatch(HardwareEvent::TurnDataWheel { delta: -10 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorUp,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 20 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorUp,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 30 });

    let recorded_playback = core.state().recorded_events[0]
        .playback
        .as_ref()
        .expect("recorded assigned pad should snapshot playback");
    assert_eq!(recorded_playback.level, 90);
    assert_eq!(recorded_playback.pan, -7);
    assert_eq!(recorded_playback.tune_cents, 500);
    let current_assignment = assignment_for_pad(&core, pad).expect("pad should remain assigned");
    assert_eq!(current_assignment.level, 120);
    assert_eq!(current_assignment.pan, 13);
    assert_eq!(current_assignment.tune_cents, -500);

    let mut snapshot = core.export_project_snapshot();
    reset_snapshot_playhead(&mut snapshot, 0);
    let mut restored = restore_snapshot(snapshot);

    restored.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = restored.dispatch(HardwareEvent::Tick { micros: 500_000 });
    let intents = playback_intents(&outputs);

    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].level, 90);
    assert_eq!(intents[0].pan, -7);
    assert_eq!(intents[0].tune_cents, 500);
}

#[test]
fn program_parameter_project_snapshot_round_trip_preserves_tune_and_edit_field() {
    let mut core = MpcCore::new();
    let pad = ProgramPad {
        bank: PadBank::A,
        pad_number: 1,
    };

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 });

    let json = core.to_project_json().expect("snapshot should encode");
    assert!(json.contains(r#""selected_program_edit_field": "tune""#));
    assert!(json.contains(r#""tune_cents": 400"#));

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("snapshot should restore");

    assert_eq!(
        restored.state().selected_program_edit_field,
        ProgramEditField::Tune
    );
    assert_eq!(
        assignment_for_pad(&restored, pad)
            .expect("pad should remain assigned")
            .tune_cents,
        400
    );
    assert!(restored.state().lcd.lines[0].contains("Edit tune"));
    assert!(restored.state().lcd.lines[3].contains(">Tune +400"));
}

#[test]
fn program_parameter_project_snapshot_rejects_invalid_tune() {
    let core = MpcCore::new();
    let mut snapshot = core.export_project_snapshot();
    snapshot.program.pad_assignments[0].tune_cents = 1201;
    let mut restored = MpcCore::new();

    let error = restored
        .restore_project_snapshot(snapshot)
        .expect_err("assignment tune outside foundation range should be rejected");

    assert_invalid_project_field(
        error,
        "program.pad_assignments[0].tune_cents",
        "-1200..=1200",
    );
}

#[test]
fn recording_assigned_pad_captures_sample_metadata() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 3,
        velocity: 77,
    });

    let recorded = core
        .state()
        .recorded_events
        .last()
        .expect("assigned pad should record sequence event");
    let playback = recorded
        .playback
        .as_ref()
        .expect("assigned pad recording should snapshot playback intent");

    assert_eq!(recorded.pad_bank, PadBank::A);
    assert_eq!(recorded.pad_number, 3);
    assert_eq!(recorded.velocity, 77);
    assert_eq!(playback.sample_id, "synthetic_a_03");
    assert_eq!(playback.sample_name, "SYN-A03");
    assert_eq!(playback.program_index, 1);
    assert_eq!(playback.program_name, "Program01");
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SequenceEventRecorded { event }
            if event.playback.as_ref().map(|intent| intent.sample_id.as_str())
                == Some("synthetic_a_03")
    )));
}

#[test]
fn midi_note_36_maps_to_pad_a01_and_reuses_playback_intent() {
    let mut physical_core = MpcCore::new();
    let mut midi_core = MpcCore::new();

    let physical_outputs = physical_core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 100,
    });
    let midi_outputs = midi_core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 1,
        note: 36,
        velocity: 100,
    });

    assert!(midi_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiNoteMapped {
            channel: 1,
            note: 36,
            bank: PadBank::A,
            pad: 1,
            velocity: 100
        }
    )));
    assert!(midi_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::PadTriggered {
            bank: PadBank::A,
            pad: 1,
            velocity: 100
        }
    )));
    assert_eq!(
        playback_intents(&midi_outputs),
        playback_intents(&physical_outputs)
    );
    assert_eq!(
        midi_core.state().last_playback,
        physical_core.state().last_playback
    );
}

#[test]
fn midi_note_51_maps_to_pad_a16() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 16,
        note: 51,
        velocity: 127,
    });

    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiNoteMapped {
            channel: 16,
            note: 51,
            bank: PadBank::A,
            pad: 16,
            velocity: 127
        }
    )));
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackIntent { intent }
            if intent.bank == PadBank::A
                && intent.pad_number == 16
                && intent.sample_id == "synthetic_a_16"
                && intent.sample_name == "SYN-A16"
                && intent.velocity == 127
    )));
    assert!(matches!(
        &core.state().last_playback,
        Some(SamplePlaybackResolution::Intent { intent })
            if intent.sample_id == "synthetic_a_16" && intent.velocity == 127
    ));
}

#[test]
fn midi_out_of_range_note_is_ignored_without_playback_or_recording_change() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 2,
        velocity: 91,
    });
    let previous_last_playback = core.state().last_playback.clone();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });

    let outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 1,
        note: 35,
        velocity: 100,
    });

    assert_eq!(
        outputs,
        vec![MachineOutput::MidiInputIgnored {
            reason: "midi note 35 is not mapped in this slice; mapped range is 36..=51".to_string(),
        }]
    );
    assert_eq!(core.state().last_playback, previous_last_playback);
    assert!(core.state().recorded_events.is_empty());
}

#[test]
fn midi_note_off_is_noop_and_does_not_trigger_playback_or_recording() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Overdub,
    });

    let outputs = core.dispatch(HardwareEvent::MidiNoteOff {
        channel: 1,
        note: 36,
        velocity: 64,
    });

    assert_eq!(
        outputs,
        vec![MachineOutput::MidiInputIgnored {
            reason: "midi note-off is a no-op in this slice".to_string(),
        }]
    );
    assert!(
        !outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::PadTriggered { .. }))
    );
    assert!(playback_intents(&outputs).is_empty());
    assert!(
        !outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::SequenceEventRecorded { .. }))
    );
    assert_eq!(core.state().last_playback, None);
    assert!(core.state().recorded_events.is_empty());
    assert!(core.state().playing);
    assert!(core.state().recording);
}

#[test]
fn midi_note_on_overdub_records_mapped_pad_sample_metadata() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Overdub,
    });
    let outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 10,
        note: 40,
        velocity: 88,
    });

    let recorded = core
        .state()
        .recorded_events
        .last()
        .expect("mapped MIDI note should record sequence event while overdubbing");
    let playback = recorded
        .playback
        .as_ref()
        .expect("mapped assigned pad should snapshot playback metadata");

    assert_eq!(recorded.selected_track, 1);
    assert_eq!(recorded.pad_bank, PadBank::A);
    assert_eq!(recorded.pad_number, 5);
    assert_eq!(recorded.velocity, 88);
    assert_eq!(recorded.tick, 0);
    assert_eq!(playback.sample_id, "synthetic_a_05");
    assert_eq!(playback.sample_name, "SYN-A05");
    assert_eq!(playback.velocity, 88);
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiNoteMapped {
            channel: 10,
            note: 40,
            bank: PadBank::A,
            pad: 5,
            velocity: 88
        }
    )));
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SequenceEventRecorded { event }
            if event.pad_bank == PadBank::A
                && event.pad_number == 5
                && event.playback.as_ref().map(|intent| intent.sample_id.as_str())
                    == Some("synthetic_a_05")
    )));
}

#[test]
fn midi_invalid_channel_note_and_velocity_are_ignored_deterministically() {
    let cases = [
        (
            HardwareEvent::MidiNoteOn {
                channel: 0,
                note: 36,
                velocity: 100,
            },
            "midi channel must be in range 1..=16",
        ),
        (
            HardwareEvent::MidiNoteOn {
                channel: 17,
                note: 36,
                velocity: 100,
            },
            "midi channel must be in range 1..=16",
        ),
        (
            HardwareEvent::MidiNoteOn {
                channel: 1,
                note: 128,
                velocity: 100,
            },
            "midi note must be in range 0..=127",
        ),
        (
            HardwareEvent::MidiNoteOn {
                channel: 1,
                note: 36,
                velocity: 0,
            },
            "midi note-on velocity must be in range 1..=127",
        ),
        (
            HardwareEvent::MidiNoteOn {
                channel: 1,
                note: 36,
                velocity: 128,
            },
            "midi note-on velocity must be in range 1..=127",
        ),
        (
            HardwareEvent::MidiNoteOff {
                channel: 1,
                note: 36,
                velocity: 128,
            },
            "midi note-off velocity must be in range 0..=127",
        ),
    ];

    for (event, expected_reason) in cases {
        let mut core = MpcCore::new();
        core.dispatch(HardwareEvent::Press {
            control: PanelControl::Overdub,
        });

        let outputs = core.dispatch(event);

        assert_eq!(
            outputs,
            vec![MachineOutput::MidiInputIgnored {
                reason: expected_reason.to_string(),
            }]
        );
        assert_eq!(core.state().last_playback, None);
        assert!(core.state().recorded_events.is_empty());
    }
}

#[test]
fn invalid_pad_and_velocity_are_ignored() {
    let mut core = MpcCore::new();

    let invalid_pad = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 17,
        velocity: 100,
    });
    assert_eq!(
        invalid_pad,
        vec![mpc_core::MachineOutput::Ignored {
            reason: "pad must be in range 1..=16".to_string(),
        }]
    );

    let invalid_velocity = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 128,
    });
    assert_eq!(
        invalid_velocity,
        vec![mpc_core::MachineOutput::Ignored {
            reason: "velocity must be in range 1..=127".to_string(),
        }]
    );
}

#[test]
fn ignored_controls_are_reported_without_changing_mode() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorUp,
    });

    assert_eq!(core.state().mode, Mode::Main);
    assert!(matches!(
        outputs.as_slice(),
        [mpc_core::MachineOutput::Ignored { reason }] if reason.contains("CursorUp")
    ));
}

#[test]
fn replaying_same_events_produces_same_state() {
    let events = vec![
        HardwareEvent::Press {
            control: PanelControl::Rec,
        },
        HardwareEvent::Press {
            control: PanelControl::Play,
        },
        HardwareEvent::Tick { micros: 500_000 },
        HardwareEvent::StrikePad {
            bank: PadBank::C,
            pad: 4,
            velocity: 64,
        },
    ];

    let mut first = MpcCore::new();
    let mut second = MpcCore::new();

    for event in &events {
        first.dispatch(event.clone());
        second.dispatch(event.clone());
    }

    assert_eq!(first.state(), second.state());
    assert_eq!(
        first.state().recorded_events,
        vec![SequenceEvent {
            selected_track: 1,
            pad_bank: PadBank::C,
            pad_number: 4,
            velocity: 64,
            tick: u64::from(INTERNAL_PPQN),
            playback: None,
        }]
    );
}

#[test]
fn project_snapshot_round_trips_after_edits_and_recording() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::TurnDataWheel { delta: 5 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 2 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 3 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 5,
        velocity: 90,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 5,
        velocity: 87,
    });

    let json = core.to_project_json().expect("snapshot should encode");
    assert!(json.contains("\"version\": 1"));
    assert!(json.contains("metadata_only_no_audio_bytes"));

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("snapshot should restore");

    assert_eq!(restored.state().mode, Mode::Program);
    assert_eq!(restored.state().sequence_index, 3);
    assert_eq!(restored.state().sequence_name, "Sequence03");
    assert_eq!(restored.state().tempo_bpm_x100, 12500);
    assert_eq!(restored.state().selected_track, 5);
    assert_eq!(restored.state().bar_count, 4);
    assert_eq!(
        restored.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::A,
            pad_number: 5
        }
    );
    assert_eq!(restored.state().recorded_events.len(), 1);
    assert_eq!(
        restored.state().recorded_events[0]
            .playback
            .as_ref()
            .map(|intent| (
                intent.sample_id.as_str(),
                intent.sample_name.as_str(),
                intent.velocity
            )),
        Some(("synthetic_a_05", "SYN-A05", 87))
    );
    assert_eq!(restored.state().playhead_ticks, 100);
    assert!(!restored.state().playing);
    assert!(!restored.state().recording);
}

#[test]
fn project_snapshot_rejects_invalid_version() {
    let core = MpcCore::new();
    let mut snapshot = core.export_project_snapshot();
    snapshot.version = PROJECT_SNAPSHOT_VERSION + 1;
    let mut restored = MpcCore::new();

    let error = restored
        .restore_project_snapshot(snapshot)
        .expect_err("unsupported version should be rejected");

    assert!(matches!(
        error,
        ProjectSnapshotError::UnsupportedVersion {
            version,
            supported: PROJECT_SNAPSHOT_VERSION,
        } if version == PROJECT_SNAPSHOT_VERSION + 1
    ));
}

#[test]
fn project_snapshot_rejects_invalid_assignment_pad() {
    let core = MpcCore::new();
    let mut snapshot = core.export_project_snapshot();
    snapshot.program.pad_assignments[0].pad.pad_number = 17;
    let mut restored = MpcCore::new();

    let error = restored
        .restore_project_snapshot(snapshot)
        .expect_err("assignment pad outside 1..=16 should be rejected");

    assert!(matches!(
        error,
        ProjectSnapshotError::InvalidValue { field, .. }
            if field == "program.pad_assignments[0].pad.pad_number"
    ));
}

#[test]
fn project_snapshot_rejects_duplicate_pad_assignments() {
    let core = MpcCore::new();
    let mut snapshot = core.export_project_snapshot();
    snapshot.program.pad_assignments[1].pad = snapshot.program.pad_assignments[0].pad;
    let mut restored = MpcCore::new();

    let error = restored
        .restore_project_snapshot(snapshot)
        .expect_err("duplicate pad assignment should be rejected");

    assert!(matches!(
        error,
        ProjectSnapshotError::DuplicatePadAssignment { pad }
            if pad == ProgramPad {
                bank: PadBank::A,
                pad_number: 1
            }
    ));
}

#[test]
fn project_snapshot_json_rejects_unknown_fields_at_persisted_boundaries() {
    let cases = [
        ("root", "", "audio_bytes", "audio_bytes"),
        ("machine", "/machine", "playing", "machine.playing"),
        (
            "machine playback",
            "/machine/last_playback",
            "file_path",
            "machine.last_playback.file_path",
        ),
        (
            "sequence",
            "/sequence",
            "sample_file_contents",
            "sequence.sample_file_contents",
        ),
        ("program", "/program", "file_path", "program.file_path"),
        (
            "assignment",
            "/program/pad_assignments/0",
            "audio_bytes",
            "program.pad_assignments[0].audio_bytes",
        ),
        (
            "sample",
            "/program/pad_assignments/0/sample",
            "sample_file_contents",
            "program.pad_assignments[0].sample.sample_file_contents",
        ),
        (
            "recorded event",
            "/sequence/recorded_events/0",
            "playing",
            "sequence.recorded_events[0].playing",
        ),
        (
            "recorded playback",
            "/sequence/recorded_events/0/playback",
            "file_path",
            "sequence.recorded_events[0].playback.file_path",
        ),
    ];

    for (label, pointer, extra_field, expected_field) in cases {
        let mut value = recorded_project_snapshot_json_value();
        insert_extra_json_field(&mut value, pointer, extra_field);
        let json = serde_json::to_string(&value).expect("mutated JSON should encode");

        let error = MpcCore::from_project_json(&json)
            .expect_err(&format!("{label} extra field should be rejected"));

        assert_invalid_project_field(error, expected_field, "unknown field");
    }
}

#[test]
fn project_snapshot_rejects_event_count_less_than_recorded_events() {
    let mut snapshot = recorded_project_snapshot();
    snapshot.machine.last_playback = None;
    snapshot.machine.event_count = snapshot.sequence.recorded_events.len() as u64 - 1;
    let mut restored = MpcCore::new();

    let error = restored
        .restore_project_snapshot(snapshot)
        .expect_err("event_count below recorded_events length should be rejected");

    assert_invalid_project_field(error, "machine.event_count", "recorded_events.len");
}

#[test]
fn project_snapshot_rejects_last_playback_without_events() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 100,
    });
    let mut snapshot = core.export_project_snapshot();
    snapshot.machine.event_count = 0;
    let mut restored = MpcCore::new();

    let error = restored
        .restore_project_snapshot(snapshot)
        .expect_err("last_playback with zero event_count should be rejected");

    assert_invalid_project_field(error, "machine.last_playback", "event_count > 0");
}

#[test]
fn project_snapshot_rejects_saturated_playhead_with_remainder() {
    let mut snapshot = MpcCore::new().export_project_snapshot();
    snapshot.machine.playhead_ticks = u64::MAX;
    snapshot.machine.playhead_tick_remainder = 1;
    let mut restored = MpcCore::new();

    let error = restored
        .restore_project_snapshot(snapshot)
        .expect_err("saturated playhead must not retain a tick remainder");

    assert_invalid_project_field(error, "machine.playhead_tick_remainder", "must be 0");
}

#[test]
fn project_snapshot_restore_refreshes_lcd() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 6,
        velocity: 90,
    });

    let snapshot = core.export_project_snapshot();
    let mut restored = MpcCore::new();
    restored
        .restore_project_snapshot(snapshot)
        .expect("snapshot should restore");

    assert_eq!(restored.state().lcd.title, "PROGRAM");
    assert!(restored.state().lcd.lines[0].contains("Program01"));
    assert!(restored.state().lcd.lines[1].contains("SYN-A06"));
    assert!(!restored.state().lcd.lines[2].contains("PLAY"));
}

#[test]
fn restored_project_can_still_emit_playback_intent() {
    let core = MpcCore::new();
    let snapshot = core.export_project_snapshot();
    let mut restored = MpcCore::new();
    restored
        .restore_project_snapshot(snapshot)
        .expect("snapshot should restore");

    let outputs = restored.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 8,
        velocity: 111,
    });

    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackIntent { intent }
            if intent.sample_id == "synthetic_a_08"
                && intent.sample_name == "SYN-A08"
                && intent.velocity == 111
    )));
}

#[test]
fn hardware_event_serializes_with_snake_case_tags() {
    let event = HardwareEvent::StrikePad {
        bank: PadBank::D,
        pad: 16,
        velocity: 127,
    };

    let json = serde_json::to_string(&event).expect("event should serialize");

    assert_eq!(
        json,
        r#"{"type":"strike_pad","bank":"d","pad":16,"velocity":127}"#
    );
}

fn snapshot_with_recorded_assigned_events_at_tick(events: &[(u8, u8)]) -> ProjectSnapshot {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });

    for (pad, velocity) in events {
        core.dispatch(HardwareEvent::StrikePad {
            bank: PadBank::A,
            pad: *pad,
            velocity: *velocity,
        });
    }

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });
    let mut snapshot = core.export_project_snapshot();
    reset_snapshot_playhead(&mut snapshot, 0);
    snapshot
}

fn reset_snapshot_playhead(snapshot: &mut ProjectSnapshot, playhead_ticks: u64) {
    snapshot.machine.playhead_ticks = playhead_ticks;
    snapshot.machine.playhead_tick_remainder = 0;
    snapshot.machine.last_playback = None;
}

fn restore_snapshot(snapshot: ProjectSnapshot) -> MpcCore {
    let mut core = MpcCore::new();
    core.restore_project_snapshot(snapshot)
        .expect("snapshot should restore");
    core
}

fn playback_intents(outputs: &[MachineOutput]) -> Vec<&SamplePlaybackIntent> {
    outputs
        .iter()
        .filter_map(|output| match output {
            MachineOutput::SamplePlaybackIntent { intent } => Some(intent),
            _ => None,
        })
        .collect()
}

fn assignment_for_pad(core: &MpcCore, pad: ProgramPad) -> Option<&mpc_core::PadAssignment> {
    core.state()
        .current_program
        .pad_assignments
        .iter()
        .find(|assignment| assignment.pad == pad)
}

fn recorded_project_snapshot() -> ProjectSnapshot {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 100,
    });
    core.export_project_snapshot()
}

fn recorded_project_snapshot_json_value() -> serde_json::Value {
    let json = serde_json::to_string(&recorded_project_snapshot()).expect("snapshot should encode");
    serde_json::from_str(&json).expect("snapshot JSON should parse as value")
}

fn insert_extra_json_field(value: &mut serde_json::Value, pointer: &str, field: &str) {
    let target = if pointer.is_empty() {
        value
    } else {
        value
            .pointer_mut(pointer)
            .unwrap_or_else(|| panic!("snapshot JSON pointer should exist: {pointer}"))
    };
    let object = target
        .as_object_mut()
        .unwrap_or_else(|| panic!("snapshot JSON pointer should be an object: {pointer}"));
    object.insert(field.to_string(), serde_json::json!("must be rejected"));
}

fn assert_invalid_project_field(
    error: ProjectSnapshotError,
    expected_field: &str,
    expected_message: &str,
) {
    match error {
        ProjectSnapshotError::InvalidValue { field, message } => {
            assert_eq!(field, expected_field);
            assert!(
                message.contains(expected_message),
                "expected message {message:?} to contain {expected_message:?}"
            );
        }
        error => panic!("expected InvalidValue for {expected_field}, got {error:?}"),
    }
}
