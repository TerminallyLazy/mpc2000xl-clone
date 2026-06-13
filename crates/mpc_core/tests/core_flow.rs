use mpc_core::{HardwareEvent, Mode, MpcCore, PadBank, PanelControl};

#[test]
fn core_starts_on_main_screen() {
    let core = MpcCore::new();

    assert_eq!(core.state().mode, Mode::Main);
    assert_eq!(core.state().lcd.title, "MAIN");
    assert_eq!(core.state().sequence_name, "Sequence01");
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
            control: PanelControl::Program,
        },
        HardwareEvent::Press {
            control: PanelControl::Play,
        },
        HardwareEvent::TurnDataWheel { delta: 2 },
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
