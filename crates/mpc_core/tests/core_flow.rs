use mpc_core::{
    HardwareEvent, INTERNAL_PPQN, MachineOutput, MainScreenField, Mode, MpcCore,
    PadAssignmentChange, PadBank, PanelControl, PlaybackMissReason, ProgramPad,
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
