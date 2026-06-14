use mpc_core::{
    DiskOperation, HardwareEvent, INTERNAL_PPQN, MachineOutput, MainScreenField, MidiSettingsField,
    Mode, MpcCore, PROJECT_SNAPSHOT_VERSION, PadAssignment, PadAssignmentChange, PadBank,
    PanelControl, PlaybackMissReason, ProgramEditField, ProgramPad, ProjectSetupSnapshot,
    ProjectSnapshot, ProjectSnapshotError, ProjectSongSnapshot, SamplePlaybackIntent,
    SamplePlaybackResolution, SampleTrim, SequenceEvent, SetupField, SetupPreferences,
    SongEditField, SongStep, SyntheticSample, TrimEditField, sequence_length_ticks_for_bars,
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
    assert_eq!(core.state().current_program.pad_assignments.len(), 64);
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
fn pad_bank_controls_change_active_bank_and_selected_program_pad() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 7,
        velocity: 90,
    });
    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::PadBankB,
    });

    assert_eq!(core.state().pad_bank, PadBank::B);
    assert_eq!(
        core.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::B,
            pad_number: 7
        }
    );
    assert!(core.state().lcd.lines[1].contains("B07"));
    assert!(core.state().lcd.lines[1].contains("SYN-B07"));
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::BankChanged { bank: PadBank::B }))
    );
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::LcdChanged))
    );

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::PadBankC,
    });

    assert_eq!(core.state().pad_bank, PadBank::C);
    assert_eq!(
        core.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::C,
            pad_number: 7
        }
    );
    assert!(core.state().lcd.lines[1].contains("C07"));
    assert!(core.state().lcd.lines[1].contains("SYN-C07"));
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::BankChanged { bank: PadBank::C }))
    );
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::LcdChanged))
    );

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::PadBankD,
    });
    assert_eq!(core.state().pad_bank, PadBank::D);
    assert_eq!(core.state().selected_program_pad.bank, PadBank::D);
    assert_eq!(core.state().selected_program_pad.pad_number, 7);
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::BankChanged { bank: PadBank::D }))
    );

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::PadBankA,
    });
    assert_eq!(core.state().pad_bank, PadBank::A);
    assert_eq!(core.state().selected_program_pad.bank, PadBank::A);
    assert_eq!(core.state().selected_program_pad.pad_number, 7);
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::BankChanged { bank: PadBank::A }))
    );
}

#[test]
fn default_program_assigns_all_pad_banks_to_synthetic_samples() {
    let core = MpcCore::new();

    assert_eq!(core.state().current_program.pad_assignments.len(), 64);
    for bank in [PadBank::A, PadBank::B, PadBank::C, PadBank::D] {
        for pad_number in 1..=16 {
            let assignment = core
                .state()
                .current_program
                .pad_assignments
                .iter()
                .find(|assignment| assignment.pad == ProgramPad { bank, pad_number })
                .expect("bank pad should have a default synthetic assignment");
            assert_eq!(
                assignment.sample.id,
                format!(
                    "synthetic_{}_{pad_number:02}",
                    bank.label().to_ascii_lowercase()
                )
            );
            assert_eq!(
                assignment.sample.name,
                format!("SYN-{}{:02}", bank.label(), pad_number)
            );
            assert_eq!(assignment.level, 100);
            assert_eq!(assignment.pan, 0);
            assert_eq!(assignment.tune_cents, 0);
        }
    }
}

#[test]
fn sample_catalog_default_selection_is_deterministic_and_lcd_reflects_metadata_only() {
    let mut core = MpcCore::new();

    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_a_01")
    );
    let selected_sample = core
        .state()
        .selected_sample()
        .expect("default catalog should select A01");
    assert_eq!(selected_sample.index, 1);
    assert_eq!(selected_sample.count, 64);
    assert_eq!(selected_sample.sample.id, "synthetic_a_01");
    assert_eq!(selected_sample.sample.name, "SYN-A01");
    assert_eq!(
        selected_sample.source_pad,
        ProgramPad {
            bank: PadBank::A,
            pad_number: 1
        }
    );

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::Sample,
    });

    assert_eq!(core.state().mode, Mode::Sample);
    assert_eq!(core.state().lcd.title, "SAMPLE");
    assert!(core.state().lcd.lines[0].contains("Sample 01/64 SYN-A01"));
    assert!(core.state().lcd.lines[1].contains("synthetic_a_01"));
    assert!(core.state().lcd.lines[3].contains("Metadata only"));
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::ModeChanged { mode: Mode::Sample }))
    );
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::LcdChanged))
    );
}

#[test]
fn sample_catalog_data_wheel_selects_next_previous_and_emits_outputs() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Sample,
    });

    let outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 1 });

    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_a_02")
    );
    assert!(core.state().lcd.lines[0].contains("Sample 02/64 SYN-A02"));
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SampleSelected { entry }
            if entry.index == 2
                && entry.count == 64
                && entry.sample.id == "synthetic_a_02"
                && entry.source_pad == ProgramPad { bank: PadBank::A, pad_number: 2 }
    )));
    assert!(
        outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::LcdChanged))
    );

    let outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 });

    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_a_01")
    );
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SampleSelected { entry }
            if entry.index == 1 && entry.sample.id == "synthetic_a_01"
    )));
}

#[test]
fn sample_catalog_dedupes_by_sample_id_so_selection_identity_is_stable() {
    let mut snapshot = MpcCore::new().export_project_snapshot();
    snapshot.machine.mode = Mode::Sample;
    snapshot.machine.selected_sample_id = Some("shared_sample".to_string());
    snapshot.program.pad_assignments = vec![
        PadAssignment {
            pad: ProgramPad {
                bank: PadBank::A,
                pad_number: 1,
            },
            sample: SyntheticSample {
                id: "shared_sample".to_string(),
                name: "FIRST-NAME".to_string(),
            },
            level: 100,
            pan: 0,
            tune_cents: 0,
        },
        PadAssignment {
            pad: ProgramPad {
                bank: PadBank::A,
                pad_number: 2,
            },
            sample: SyntheticSample {
                id: "shared_sample".to_string(),
                name: "SECOND-NAME".to_string(),
            },
            level: 100,
            pan: 0,
            tune_cents: 0,
        },
        PadAssignment {
            pad: ProgramPad {
                bank: PadBank::A,
                pad_number: 3,
            },
            sample: SyntheticSample {
                id: "unique_sample".to_string(),
                name: "UNIQUE".to_string(),
            },
            level: 100,
            pan: 0,
            tune_cents: 0,
        },
    ];

    let mut core = MpcCore::new();
    core.restore_project_snapshot(snapshot)
        .expect("duplicate sample ids should collapse in the catalog");

    let catalog = core.state().sample_catalog();
    assert_eq!(catalog.len(), 2);
    assert_eq!(catalog[0].sample.id, "shared_sample");
    assert_eq!(catalog[0].sample.name, "FIRST-NAME");
    assert_eq!(catalog[0].source_pad.pad_number, 1);
    assert_eq!(catalog[1].sample.id, "unique_sample");

    let outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 1 });

    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("unique_sample")
    );
    assert!(core.state().lcd.lines[0].contains("UNIQUE"));
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SampleSelected { entry }
            if entry.sample.id == "unique_sample" && entry.index == 2 && entry.count == 2
    )));
}

#[test]
fn sample_catalog_trim_shares_selection_with_sample_mode() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Sample,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 16 });
    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_b_01")
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Trim,
    });

    assert_eq!(core.state().mode, Mode::Trim);
    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_b_01")
    );
    assert!(core.state().lcd.lines[0].contains("Trim 17/64 SYN-B01"));
    assert!(core.state().lcd.lines[1].contains("Start 000000"));
    assert!(core.state().lcd.lines[2].contains("Src B01"));
}

#[test]
fn sample_catalog_empty_navigation_is_ignored_safely() {
    let mut snapshot = MpcCore::new().export_project_snapshot();
    snapshot.machine.mode = Mode::Sample;
    snapshot.machine.selected_sample_id = None;
    snapshot.program.pad_assignments.clear();

    let mut core = MpcCore::new();
    core.restore_project_snapshot(snapshot)
        .expect("empty assignment catalog is valid metadata");

    assert_eq!(core.state().mode, Mode::Sample);
    assert_eq!(core.state().selected_sample_id, None);
    assert!(core.state().selected_sample().is_none());
    assert!(core.state().lcd.lines[0].contains("empty catalog"));

    let outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 1 });

    assert_eq!(core.state().selected_sample_id, None);
    assert_eq!(
        outputs,
        vec![MachineOutput::Ignored {
            reason: "sample_catalog.empty".to_string()
        }]
    );
}

#[test]
fn sample_catalog_project_snapshot_round_trip_preserves_selected_sample_identity() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Sample,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 49 });
    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_d_02")
    );

    let json = core
        .to_project_json()
        .expect("sample selection snapshot should encode");

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("sample selection snapshot should restore");

    assert_eq!(restored.state().mode, Mode::Sample);
    assert_eq!(
        restored.state().selected_sample_id.as_deref(),
        Some("synthetic_d_02")
    );
    let selected_sample = restored
        .state()
        .selected_sample()
        .expect("restored sample selection should resolve");
    assert_eq!(selected_sample.index, 50);
    assert_eq!(selected_sample.sample.name, "SYN-D02");
    assert!(restored.state().lcd.lines[0].contains("Sample 50/64 SYN-D02"));
}

#[test]
fn sample_catalog_output_serializes_with_stable_shape() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Sample,
    });
    let outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 16 });
    let sample_selected = outputs
        .iter()
        .find(|output| matches!(output, MachineOutput::SampleSelected { .. }))
        .expect("wheel navigation should emit SampleSelected");

    let json = serde_json::to_value(sample_selected).expect("output should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "sample_selected",
            "entry": {
                "index": 17,
                "count": 64,
                "sample": {
                    "id": "synthetic_b_01",
                    "name": "SYN-B01"
                },
                "source_pad": {
                    "bank": "b",
                    "pad_number": 1
                },
                "start_frame": 0,
                "end_frame": 67199,
                "window_length_frames": 67200,
                "length_frames": 67200
            }
        })
    );
}

#[test]
fn trim_default_window_is_visible_and_selected_field_defaults_to_start() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Trim,
    });

    let selected = core
        .state()
        .selected_sample()
        .expect("default selected sample should resolve");
    assert_eq!(core.state().selected_trim_edit_field, TrimEditField::Start);
    assert_eq!(selected.sample.id, "synthetic_a_01");
    assert_eq!(selected.start_frame, 0);
    assert_eq!(selected.end_frame, 47_999);
    assert_eq!(selected.window_length_frames, 48_000);
    assert_eq!(selected.length_frames, 48_000);
    assert!(core.state().lcd.lines[0].contains("Edit start"));
    assert!(core.state().lcd.lines[1].contains(">Start 000000"));
    assert!(core.state().lcd.lines[1].contains(" End 047999"));
    assert!(core.state().lcd.lines[2].contains("Window 048000"));
}

#[test]
fn trim_cursor_left_right_selects_start_end_without_sample_mode_navigation() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Sample,
    });

    let sample_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });

    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_a_01")
    );
    assert_eq!(core.state().selected_trim_edit_field, TrimEditField::Start);
    assert_eq!(
        sample_outputs,
        vec![MachineOutput::Ignored {
            reason: "sample.cursor_right_unmapped".to_string()
        }]
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Trim,
    });
    let right_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(core.state().selected_trim_edit_field, TrimEditField::End);
    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_a_01")
    );
    assert_eq!(right_outputs, vec![MachineOutput::LcdChanged]);
    assert!(core.state().lcd.lines[0].contains("Edit end"));
    assert!(core.state().lcd.lines[1].contains(">End 047999"));

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    assert_eq!(core.state().selected_trim_edit_field, TrimEditField::Start);
}

#[test]
fn trim_data_wheel_edits_start_end_with_clamps_and_stable_output_shape() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Trim,
    });

    let outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 12 });

    assert_eq!(
        outputs,
        vec![
            MachineOutput::SampleTrimChanged {
                sample_id: "synthetic_a_01".to_string(),
                start_frame: 12,
                end_frame: 47_999,
                window_length_frames: 47_988,
                selected_field: TrimEditField::Start,
            },
            MachineOutput::LcdChanged,
        ]
    );
    let selected = core.state().selected_sample().unwrap();
    assert_eq!(selected.start_frame, 12);
    assert_eq!(selected.end_frame, 47_999);
    assert_eq!(selected.window_length_frames, 47_988);
    assert_eq!(
        serde_json::to_value(&outputs[0]).expect("trim output should serialize"),
        serde_json::json!({
            "type": "sample_trim_changed",
            "sample_id": "synthetic_a_01",
            "start_frame": 12,
            "end_frame": 47999,
            "window_length_frames": 47988,
            "selected_field": "start"
        })
    );

    let outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: -99 });
    assert_eq!(
        outputs,
        vec![
            MachineOutput::SampleTrimChanged {
                sample_id: "synthetic_a_01".to_string(),
                start_frame: 0,
                end_frame: 47_999,
                window_length_frames: 48_000,
                selected_field: TrimEditField::Start,
            },
            MachineOutput::LcdChanged,
        ]
    );
    assert!(core.state().current_program.pad_assignments.len() == 64);
    assert!(core.state().selected_sample().unwrap().start_frame == 0);

    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 }),
        vec![MachineOutput::Ignored {
            reason: "trim.start.boundary".to_string()
        }]
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 0 }),
        vec![MachineOutput::Ignored {
            reason: "trim.start.zero_delta_ignored".to_string()
        }]
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    let outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: -10 });
    assert_eq!(
        outputs,
        vec![
            MachineOutput::SampleTrimChanged {
                sample_id: "synthetic_a_01".to_string(),
                start_frame: 0,
                end_frame: 47_989,
                window_length_frames: 47_990,
                selected_field: TrimEditField::End,
            },
            MachineOutput::LcdChanged,
        ]
    );
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 100_000 });
    let selected = core.state().selected_sample().unwrap();
    assert_eq!(selected.start_frame, 47_989);
    assert_eq!(selected.end_frame, 47_989);
    assert_eq!(selected.window_length_frames, 1);
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 }),
        vec![MachineOutput::Ignored {
            reason: "trim.end.boundary".to_string()
        }]
    );
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 }),
        vec![
            MachineOutput::SampleTrimChanged {
                sample_id: "synthetic_a_01".to_string(),
                start_frame: 47_988,
                end_frame: 47_989,
                window_length_frames: 2,
                selected_field: TrimEditField::Start,
            },
            MachineOutput::LcdChanged,
        ]
    );
}

#[test]
fn sample_mode_wheel_navigates_and_trim_soft_keys_navigate_without_trim_mutation() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Sample,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 1 });

    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_a_02")
    );
    assert_eq!(core.state().selected_sample().unwrap().start_frame, 0);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Trim,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 5 });
    assert_eq!(core.state().selected_sample().unwrap().start_frame, 5);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_a_03")
    );
    assert_eq!(core.state().selected_sample().unwrap().start_frame, 0);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(1),
    });
    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("synthetic_a_02")
    );
    assert_eq!(core.state().selected_sample().unwrap().start_frame, 5);
}

#[test]
fn trim_window_is_carried_by_pad_strike_and_midi_note_on_playback_intents() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Trim,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 11 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: -7 });

    let pad_outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 90,
    });
    let pad_intent = playback_intents(&pad_outputs)
        .into_iter()
        .next()
        .expect("pad strike should emit playback intent");
    assert_eq!(pad_intent.start_frame, 11);
    assert_eq!(pad_intent.end_frame, 47_992);
    assert_eq!(pad_intent.window_length_frames, 47_982);

    let midi_outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 1,
        note: 36,
        velocity: 91,
    });
    let midi_intent = playback_intents(&midi_outputs)
        .into_iter()
        .next()
        .expect("midi note-on should emit playback intent");
    assert_eq!(midi_intent.sample_id, "synthetic_a_01");
    assert_eq!(midi_intent.start_frame, 11);
    assert_eq!(midi_intent.end_frame, 47_992);
    assert_eq!(midi_intent.window_length_frames, 47_982);
}

#[test]
fn trim_recording_snapshots_window_and_restored_playback_uses_recorded_window() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Trim,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 10 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 1,
        velocity: 84,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });

    let recorded_playback = core.state().recorded_events[0]
        .playback
        .as_ref()
        .expect("recorded event should snapshot playback");
    assert_eq!(recorded_playback.start_frame, 10);
    assert_eq!(recorded_playback.end_frame, 47_999);
    assert_eq!(recorded_playback.window_length_frames, 47_990);

    core.dispatch(HardwareEvent::TurnDataWheel { delta: 20 });
    assert_eq!(core.state().selected_sample().unwrap().start_frame, 30);
    assert_eq!(
        core.state().recorded_events[0]
            .playback
            .as_ref()
            .map(|intent| intent.start_frame),
        Some(10)
    );

    let mut snapshot = core.export_project_snapshot();
    reset_snapshot_playhead(&mut snapshot, 0);
    let mut restored = restore_snapshot(snapshot);
    restored.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = restored.dispatch(HardwareEvent::Tick { micros: 500_000 });
    let intents = playback_intents(&outputs);

    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].start_frame, 10);
    assert_eq!(intents[0].end_frame, 47_999);
    assert_eq!(intents[0].window_length_frames, 47_990);
    assert_eq!(
        restored.state().selected_sample().unwrap().start_frame,
        30,
        "restored current trim should not rewrite recorded playback"
    );
}

#[test]
fn trim_project_snapshot_defaults_round_trips_and_validates_entries() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Trim,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: -6 });

    let json = core.to_project_json().expect("trim snapshot should encode");
    assert!(json.contains(r#""selected_trim_edit_field": "end""#));
    assert!(json.contains(r#""sample_trims""#));
    assert!(json.contains(r#""start_frame": 4"#));
    assert!(json.contains(r#""end_frame": 47993"#));

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("trim snapshot should restore");
    assert_eq!(restored.state().mode, Mode::Trim);
    assert_eq!(
        restored.state().selected_trim_edit_field,
        TrimEditField::End
    );
    let selected = restored.state().selected_sample().unwrap();
    assert_eq!(selected.start_frame, 4);
    assert_eq!(selected.end_frame, 47_993);
    assert_eq!(selected.window_length_frames, 47_990);

    let mut older_value =
        serde_json::to_value(MpcCore::new().export_project_snapshot()).expect("snapshot value");
    older_value
        .pointer_mut("/machine")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("selected_trim_edit_field");
    older_value
        .pointer_mut("/program")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("sample_trims");
    let older_json = serde_json::to_string(&older_value).expect("older snapshot should encode");
    let mut older = MpcCore::new();
    older
        .restore_project_json(&older_json)
        .expect("older snapshot without trim fields should restore defaults");
    assert_eq!(older.state().selected_trim_edit_field, TrimEditField::Start);
    assert!(older.state().sample_trims.is_empty());
    assert_eq!(older.state().selected_sample().unwrap().start_frame, 0);
    assert_eq!(older.state().selected_sample().unwrap().end_frame, 47_999);

    let invalid_cases = [
        (
            vec![SampleTrim {
                sample_id: "".to_string(),
                start_frame: 0,
                end_frame: 1,
            }],
            "program.sample_trims[0].sample_id",
            "must not be empty",
        ),
        (
            vec![
                SampleTrim {
                    sample_id: "synthetic_a_01".to_string(),
                    start_frame: 0,
                    end_frame: 1,
                },
                SampleTrim {
                    sample_id: "synthetic_a_01".to_string(),
                    start_frame: 2,
                    end_frame: 3,
                },
            ],
            "program.sample_trims[1].sample_id",
            "duplicate",
        ),
        (
            vec![SampleTrim {
                sample_id: "missing".to_string(),
                start_frame: 0,
                end_frame: 1,
            }],
            "program.sample_trims[0].sample_id",
            "unknown sample id",
        ),
        (
            vec![SampleTrim {
                sample_id: "synthetic_a_01".to_string(),
                start_frame: 0,
                end_frame: 48_000,
            }],
            "program.sample_trims[0].end_frame",
            "generated sample length",
        ),
        (
            vec![SampleTrim {
                sample_id: "synthetic_a_01".to_string(),
                start_frame: 5,
                end_frame: 4,
            }],
            "program.sample_trims[0].start_frame",
            "<= end_frame",
        ),
    ];

    for (sample_trims, expected_field, expected_message) in invalid_cases {
        let mut snapshot = MpcCore::new().export_project_snapshot();
        snapshot.program.sample_trims = sample_trims;
        let mut invalid_core = MpcCore::new();
        let error = invalid_core
            .restore_project_snapshot(snapshot)
            .expect_err("invalid sample trims should be rejected");
        assert_invalid_project_field(error, expected_field, expected_message);
    }

    let mut invalid_selected = MpcCore::new().export_project_snapshot();
    invalid_selected.machine.selected_sample_id = Some("missing".to_string());
    let mut invalid_core = MpcCore::new();
    let error = invalid_core
        .restore_project_snapshot(invalid_selected)
        .expect_err("unknown selected sample id should be rejected");
    assert_invalid_project_field(error, "machine.selected_sample_id", "unknown sample id");

    let mut value = serde_json::to_value(restored.export_project_snapshot())
        .expect("trim snapshot should encode as value");
    insert_extra_json_field(&mut value, "/program/sample_trims/0", "audio_bytes");
    let json = serde_json::to_string(&value).expect("mutated trim JSON should encode");
    let error = MpcCore::from_project_json(&json)
        .expect_err("unknown nested trim JSON field should be rejected");
    assert_invalid_project_field(
        error,
        "program.sample_trims[0].audio_bytes",
        "unknown field",
    );
}

#[test]
fn program_mode_banked_pad_strike_selects_bank_and_emits_banked_intent() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::B,
        pad: 7,
        velocity: 104,
    });

    assert_eq!(core.state().pad_bank, PadBank::B);
    assert_eq!(
        core.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::B,
            pad_number: 7
        }
    );
    assert!(core.state().lcd.lines[1].contains("B07"));
    assert!(core.state().lcd.lines[1].contains("SYN-B07"));
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackIntent { intent }
            if intent.bank == PadBank::B
                && intent.pad_number == 7
                && intent.sample_id == "synthetic_b_07"
                && intent.sample_name == "SYN-B07"
                && intent.velocity == 104
    )));
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
        playback: Some(sample_playback_intent_for_bank_pad(PadBank::B, 12, 96)),
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
fn recording_banked_pad_stores_banked_event_and_playback_intent() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Rec,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::D,
        pad: 11,
        velocity: 77,
    });

    let expected_playback = sample_playback_intent_for_bank_pad(PadBank::D, 11, 77);
    let recorded = core
        .state()
        .recorded_events
        .last()
        .expect("banked pad should record while playing and recording");

    assert_eq!(recorded.pad_bank, PadBank::D);
    assert_eq!(recorded.pad_number, 11);
    assert_eq!(recorded.velocity, 77);
    assert_eq!(recorded.playback, Some(expected_playback.clone()));
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackIntent { intent } if intent == &expected_playback
    )));
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SequenceEventRecorded { event }
            if event.pad_bank == PadBank::D
                && event.pad_number == 11
                && event.playback.as_ref() == Some(&expected_playback)
    )));
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
    assert_eq!(
        core.state().playhead_ticks,
        core.state().sequence_length_ticks()
    );
    assert!(!core.state().playing);

    let after_large_tick = core.state().playhead_ticks;
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    assert_eq!(core.state().playhead_ticks, after_large_tick);
}

#[test]
fn sequence_loop_locate_start_resets_playhead_and_remainder_while_stopped_or_playing() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    core.dispatch(HardwareEvent::Tick { micros: 1 });
    assert_eq!(core.state().playhead_ticks, u64::from(INTERNAL_PPQN));
    assert!(core.state().playhead_tick_remainder > 0);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Stop,
    });
    let stopped_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::LocateStart,
    });
    assert_eq!(core.state().playhead_ticks, 0);
    assert_eq!(core.state().playhead_tick_remainder, 0);
    assert!(!core.state().playing);
    assert_eq!(
        stopped_outputs,
        vec![
            MachineOutput::PlayheadLocated { tick: 0 },
            MachineOutput::LcdChanged,
        ]
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    core.dispatch(HardwareEvent::Tick { micros: 1 });
    let playing_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::LocateStart,
    });

    assert!(core.state().playing);
    assert_eq!(core.state().playhead_ticks, 0);
    assert_eq!(core.state().playhead_tick_remainder, 0);
    assert_eq!(
        playing_outputs,
        vec![
            MachineOutput::PlayheadLocated { tick: 0 },
            MachineOutput::LcdChanged,
        ]
    );
}

#[test]
fn sequence_loop_disabled_clamps_at_sequence_end_and_stops() {
    let mut core = MpcCore::new();
    let sequence_length_ticks = sequence_length_ticks_for_bars(1);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::Tick { micros: 2_000_000 });

    assert_eq!(core.state().playhead_ticks, sequence_length_ticks);
    assert_eq!(core.state().playhead_tick_remainder, 0);
    assert!(!core.state().playing);
    assert!(!core.state().recording);
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::TransportChanged {
            playing: false,
            recording: false
        }
    )));
    assert!(outputs.contains(&MachineOutput::LcdChanged));
}

#[test]
fn sequence_loop_enabled_wraps_and_schedules_events_on_both_sides_of_boundary() {
    let mut snapshot =
        snapshot_with_recorded_assigned_events(&[(0, 1, 81), (360, 2, 82), (48, 3, 83)]);
    snapshot.sequence.loop_enabled = true;
    reset_snapshot_playhead(&mut snapshot, 350);
    let mut core = restore_snapshot(snapshot);

    assert_eq!(core.state().sequence_length_ticks(), 384);
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

    assert_eq!(core.state().playhead_ticks, 62);
    assert!(core.state().playing);
    assert_eq!(played_pads, vec![2, 1, 3]);
    assert_eq!(
        scheduled_sample_ids,
        vec!["synthetic_a_02", "synthetic_a_01", "synthetic_a_03"]
    );
    assert!(matches!(
        &core.state().last_playback,
        Some(SamplePlaybackResolution::Intent { intent })
            if intent.sample_id == "synthetic_a_03" && intent.velocity == 83
    ));
}

#[test]
fn sequence_loop_does_not_fire_tick_zero_event_on_ordinary_play_from_start() {
    let mut snapshot = snapshot_with_recorded_assigned_events(&[(0, 1, 81), (48, 3, 83)]);
    snapshot.sequence.loop_enabled = true;
    reset_snapshot_playhead(&mut snapshot, 0);
    let mut core = restore_snapshot(snapshot);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::Tick { micros: 250_000 });

    let played_pads = outputs
        .iter()
        .filter_map(|output| match output {
            MachineOutput::SequenceEventPlayed { event } => Some(event.pad_number),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(core.state().playhead_ticks, 48);
    assert_eq!(played_pads, vec![3]);
}

#[test]
fn sequence_loop_length_follows_bar_count_edits() {
    let mut core = MpcCore::new();

    assert_eq!(core.state().sequence_length_ticks(), 384);
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 2 });
    assert_eq!(core.state().bar_count, 3);
    assert_eq!(core.state().sequence_length_ticks(), 1_152);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    core.dispatch(HardwareEvent::Tick { micros: 2_000_000 });
    assert_eq!(core.state().playhead_ticks, 384);
    assert!(core.state().playing);

    core.dispatch(HardwareEvent::Tick { micros: 4_000_000 });
    assert_eq!(core.state().playhead_ticks, 1_152);
    assert!(!core.state().playing);
}

#[test]
fn sequence_loop_project_snapshot_round_trip_preserves_loop_enabled() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::ToggleLoop,
    });
    assert_eq!(
        outputs,
        vec![
            MachineOutput::LoopChanged { enabled: true },
            MachineOutput::LcdChanged,
        ]
    );
    assert!(core.state().loop_enabled);

    let json = core.to_project_json().expect("snapshot should encode");
    assert!(json.contains(r#""loop_enabled": true"#));

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("snapshot should restore");
    assert!(restored.state().loop_enabled);
    assert!(!restored.state().playing);
}

#[test]
fn sequence_loop_missing_snapshot_field_defaults_to_disabled() {
    let core = MpcCore::new();
    let mut value = serde_json::to_value(core.export_project_snapshot())
        .expect("snapshot should encode as JSON value");
    value
        .pointer_mut("/sequence")
        .expect("sequence object should exist")
        .as_object_mut()
        .expect("sequence should be an object")
        .remove("loop_enabled");
    let json = serde_json::to_string(&value).expect("snapshot should encode");

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("older snapshot without loop_enabled should restore");

    assert!(!restored.state().loop_enabled);
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
fn sequence_erase_main_f5_erases_latest_selected_track_event_and_updates_lcd_count() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Overdub,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::B,
        pad: 3,
        velocity: 91,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::C,
        pad: 4,
        velocity: 92,
    });

    let retained = core.state().recorded_events[0].clone();
    let erased = core.state().recorded_events[1].clone();
    let last_playback_before_erase = core.state().last_playback.clone();
    let active_bank_before_erase = core.state().pad_bank;
    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(5),
    });

    assert_eq!(core.state().recorded_events, vec![retained]);
    assert_eq!(core.state().pad_bank, active_bank_before_erase);
    assert_eq!(core.state().last_playback, last_playback_before_erase);
    assert!(core.state().lcd.lines[3].contains("E001"));
    assert_eq!(
        outputs,
        vec![
            MachineOutput::SequenceEventsErased {
                selected_track: 1,
                count: 1,
                events: vec![erased],
            },
            MachineOutput::LcdChanged,
        ]
    );
}

#[test]
fn sequence_erase_main_f5_with_no_events_on_selected_track_is_ignored_without_lcd_change() {
    let mut snapshot = MpcCore::new().export_project_snapshot();
    snapshot.sequence.selected_track = 2;
    snapshot.sequence.recorded_events = vec![SequenceEvent {
        selected_track: 1,
        pad_bank: PadBank::A,
        pad_number: 1,
        velocity: 88,
        tick: 96,
        playback: Some(sample_playback_intent_for_track_bank_pad(
            1,
            PadBank::A,
            1,
            88,
        )),
    }];
    snapshot.machine.event_count = 7;
    let mut core = restore_snapshot(snapshot);
    let mut expected_state = core.state().clone();
    expected_state.event_count += 1;

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(5),
    });

    assert_eq!(
        outputs,
        vec![MachineOutput::Ignored {
            reason: "sequence.erase.track_2.no_events".to_string(),
        }]
    );
    assert_eq!(core.state(), &expected_state);
}

#[test]
fn sequence_erase_is_scoped_to_selected_track_and_preserves_remaining_order() {
    let mut snapshot = MpcCore::new().export_project_snapshot();
    snapshot.sequence.selected_track = 2;
    snapshot.sequence.recorded_events = vec![
        SequenceEvent {
            selected_track: 1,
            pad_bank: PadBank::A,
            pad_number: 1,
            velocity: 81,
            tick: 24,
            playback: Some(sample_playback_intent_for_track_bank_pad(
                1,
                PadBank::A,
                1,
                81,
            )),
        },
        SequenceEvent {
            selected_track: 2,
            pad_bank: PadBank::D,
            pad_number: 8,
            velocity: 82,
            tick: 48,
            playback: Some(sample_playback_intent_for_track_bank_pad(
                2,
                PadBank::D,
                8,
                82,
            )),
        },
        SequenceEvent {
            selected_track: 1,
            pad_bank: PadBank::B,
            pad_number: 3,
            velocity: 83,
            tick: 72,
            playback: Some(sample_playback_intent_for_track_bank_pad(
                1,
                PadBank::B,
                3,
                83,
            )),
        },
    ];
    snapshot.machine.event_count = 12;
    let mut core = restore_snapshot(snapshot);

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(5),
    });

    assert_eq!(
        core.state()
            .recorded_events
            .iter()
            .map(|event| (event.selected_track, event.pad_bank, event.pad_number))
            .collect::<Vec<_>>(),
        vec![(1, PadBank::A, 1), (1, PadBank::B, 3)]
    );
    assert!(matches!(
        outputs.as_slice(),
        [
            MachineOutput::SequenceEventsErased {
                selected_track: 2,
                count: 1,
                events,
            },
            MachineOutput::LcdChanged,
        ] if events.len() == 1
            && events[0].selected_track == 2
            && events[0].pad_bank == PadBank::D
            && events[0].pad_number == 8
    ));
}

#[test]
fn sequence_erase_while_playing_and_recording_preserves_transport_playhead_and_last_playback() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Overdub,
    });
    core.dispatch(HardwareEvent::Tick { micros: 250_000 });
    core.dispatch(HardwareEvent::Tick { micros: 1 });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::D,
        pad: 9,
        velocity: 94,
    });

    let playing = core.state().playing;
    let recording = core.state().recording;
    let playhead_ticks = core.state().playhead_ticks;
    let playhead_tick_remainder = core.state().playhead_tick_remainder;
    let pad_bank = core.state().pad_bank;
    let last_playback = core.state().last_playback.clone();

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(5),
    });

    assert!(matches!(
        outputs.as_slice(),
        [
            MachineOutput::SequenceEventsErased {
                selected_track: 1,
                count: 1,
                ..
            },
            MachineOutput::LcdChanged,
        ]
    ));
    assert_eq!(core.state().recorded_events.len(), 0);
    assert_eq!(core.state().playing, playing);
    assert_eq!(core.state().recording, recording);
    assert_eq!(core.state().playhead_ticks, playhead_ticks);
    assert_eq!(
        core.state().playhead_tick_remainder,
        playhead_tick_remainder
    );
    assert_eq!(core.state().pad_bank, pad_bank);
    assert_eq!(core.state().last_playback, last_playback);
}

#[test]
fn sequence_erase_playback_after_erase_schedules_only_retained_events() {
    let snapshot = snapshot_with_recorded_assigned_events_at_tick(&[(1, 71), (2, 72)]);
    let mut core = restore_snapshot(snapshot);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(5),
    });
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

    assert_eq!(core.state().recorded_events.len(), 1);
    assert_eq!(core.state().recorded_events[0].pad_number, 1);
    assert_eq!(played_pads, vec![1]);
    assert_eq!(scheduled_sample_ids, vec!["synthetic_a_01"]);
}

#[test]
fn sequence_erase_project_snapshot_round_trip_contains_only_retained_events() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Overdub,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::B,
        pad: 5,
        velocity: 90,
    });
    core.dispatch(HardwareEvent::Tick { micros: 500_000 });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 6,
        velocity: 91,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(5),
    });

    let json = core.to_project_json().expect("snapshot should encode");
    let value: serde_json::Value =
        serde_json::from_str(&json).expect("snapshot JSON should parse as value");
    let recorded_events = value
        .pointer("/sequence/recorded_events")
        .and_then(serde_json::Value::as_array)
        .expect("snapshot should contain recorded events");
    assert_eq!(recorded_events.len(), 1);
    assert_eq!(
        recorded_events[0]
            .pointer("/playback/sample_id")
            .and_then(serde_json::Value::as_str),
        Some("synthetic_b_05")
    );

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("snapshot should restore");

    assert_eq!(restored.state().recorded_events.len(), 1);
    assert_eq!(restored.state().recorded_events[0].pad_bank, PadBank::B);
    assert_eq!(restored.state().recorded_events[0].pad_number, 5);
    assert_eq!(
        restored.state().recorded_events[0]
            .playback
            .as_ref()
            .map(|intent| intent.sample_id.as_str()),
        Some("synthetic_b_05")
    );
}

#[test]
fn sequence_erase_output_serializes_with_stable_shape() {
    let output = MachineOutput::SequenceEventsErased {
        selected_track: 7,
        count: 1,
        events: vec![SequenceEvent {
            selected_track: 7,
            pad_bank: PadBank::D,
            pad_number: 16,
            velocity: 127,
            tick: 384,
            playback: None,
        }],
    };

    let json = serde_json::to_string(&output).expect("erase output should serialize");

    assert_eq!(
        json,
        r#"{"type":"sequence_events_erased","selected_track":7,"count":1,"events":[{"selected_track":7,"pad_bank":"d","pad_number":16,"velocity":127,"tick":384}]}"#
    );
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
fn track_mute_default_state_and_lcd_are_deterministic() {
    let core = MpcCore::new();

    assert!(core.state().muted_tracks.is_empty());
    assert!(!core.state().is_track_muted(1));
    assert!(core.state().lcd.lines[1].contains("Trk 01 Mute off/00"));
    assert_eq!(core.state().lcd.soft_keys[3], "Mute");
}

#[test]
fn track_mute_main_soft_key_4_toggles_outputs_and_serializes() {
    let mut core = MpcCore::new();

    let mute_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(4),
    });

    assert_eq!(core.state().muted_tracks, vec![1]);
    assert!(core.state().is_track_muted(1));
    assert!(core.state().lcd.lines[1].contains("Mute on/01"));
    assert_eq!(
        mute_outputs,
        vec![
            MachineOutput::TrackMuteChanged {
                track: 1,
                muted: true,
                muted_tracks: vec![1],
            },
            MachineOutput::LcdChanged,
        ]
    );
    assert_eq!(
        serde_json::to_value(&mute_outputs[0]).expect("track mute output should serialize"),
        serde_json::json!({
            "type": "track_mute_changed",
            "track": 1,
            "muted": true,
            "muted_tracks": [1]
        })
    );

    let unmute_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(4),
    });

    assert!(core.state().muted_tracks.is_empty());
    assert!(core.state().lcd.lines[1].contains("Mute off/00"));
    assert_eq!(
        unmute_outputs,
        vec![
            MachineOutput::TrackMuteChanged {
                track: 1,
                muted: false,
                muted_tracks: Vec::new(),
            },
            MachineOutput::LcdChanged,
        ]
    );
}

#[test]
fn track_mute_playback_skips_muted_track_without_outputs_or_last_playback() {
    let mut snapshot = snapshot_with_recorded_track_events(&[(1, 48, PadBank::A, 3, 83)]);
    snapshot.sequence.muted_tracks = vec![1];
    let previous_playback = SamplePlaybackResolution::Intent {
        intent: sample_playback_intent_for_track_bank_pad(2, PadBank::D, 16, 127),
    };
    reset_snapshot_playhead(&mut snapshot, 0);
    snapshot.machine.last_playback = Some(previous_playback.clone());
    let mut core = restore_snapshot(snapshot);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });

    assert_eq!(core.state().playhead_ticks, 96);
    assert!(outputs.contains(&MachineOutput::LcdChanged));
    assert!(
        !outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::SequenceEventPlayed { .. }))
    );
    assert!(playback_intents(&outputs).is_empty());
    assert_eq!(core.state().last_playback, Some(previous_playback));
}

#[test]
fn track_mute_playback_schedules_only_unmuted_tracks() {
    let mut snapshot = snapshot_with_recorded_track_events(&[
        (1, 48, PadBank::A, 1, 81),
        (2, 96, PadBank::A, 2, 82),
    ]);
    snapshot.sequence.muted_tracks = vec![2];
    reset_snapshot_playhead(&mut snapshot, 0);
    let mut core = restore_snapshot(snapshot);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });

    let played_tracks = outputs
        .iter()
        .filter_map(|output| match output {
            MachineOutput::SequenceEventPlayed { event } => Some(event.selected_track),
            _ => None,
        })
        .collect::<Vec<_>>();
    let intent_tracks = playback_intents(&outputs)
        .iter()
        .map(|intent| intent.selected_track)
        .collect::<Vec<_>>();

    assert_eq!(played_tracks, vec![1]);
    assert_eq!(intent_tracks, vec![1]);
    assert!(matches!(
        &core.state().last_playback,
        Some(SamplePlaybackResolution::Intent { intent })
            if intent.selected_track == 1 && intent.sample_id == "synthetic_a_01"
    ));
}

#[test]
fn track_mute_loop_playback_skips_muted_track_across_boundary() {
    let mut snapshot = snapshot_with_recorded_track_events(&[
        (1, 360, PadBank::A, 1, 81),
        (2, 48, PadBank::A, 2, 82),
    ]);
    snapshot.sequence.loop_enabled = true;
    snapshot.sequence.muted_tracks = vec![1];
    reset_snapshot_playhead(&mut snapshot, 350);
    let mut core = restore_snapshot(snapshot);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Play,
    });
    let outputs = core.dispatch(HardwareEvent::Tick { micros: 500_000 });

    let played_tracks = outputs
        .iter()
        .filter_map(|output| match output {
            MachineOutput::SequenceEventPlayed { event } => Some(event.selected_track),
            _ => None,
        })
        .collect::<Vec<_>>();
    let intent_tracks = playback_intents(&outputs)
        .iter()
        .map(|intent| intent.selected_track)
        .collect::<Vec<_>>();

    assert_eq!(core.state().playhead_ticks, 62);
    assert_eq!(played_tracks, vec![2]);
    assert_eq!(intent_tracks, vec![2]);
    assert!(matches!(
        &core.state().last_playback,
        Some(SamplePlaybackResolution::Intent { intent })
            if intent.selected_track == 2 && intent.sample_id == "synthetic_a_02"
    ));
}

#[test]
fn track_mute_recording_while_muted_still_records_metadata() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(4),
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Overdub,
    });
    let outputs = core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::A,
        pad: 3,
        velocity: 83,
    });

    assert_eq!(core.state().muted_tracks, vec![1]);
    assert_eq!(core.state().recorded_events.len(), 1);
    let event = &core.state().recorded_events[0];
    assert_eq!(event.selected_track, 1);
    assert_eq!(
        event
            .playback
            .as_ref()
            .map(|intent| intent.sample_id.as_str()),
        Some("synthetic_a_03")
    );
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SequenceEventRecorded { event }
            if event.selected_track == 1
                && event.playback.as_ref().map(|intent| intent.sample_id.as_str())
                    == Some("synthetic_a_03")
    )));
    assert_eq!(playback_intents(&outputs).len(), 1);
}

#[test]
fn track_mute_erase_while_muted_still_erases_selected_track_events() {
    let mut snapshot = snapshot_with_recorded_track_events(&[(1, 48, PadBank::A, 4, 84)]);
    snapshot.sequence.muted_tracks = vec![1];
    snapshot.sequence.selected_track = 1;
    let mut core = restore_snapshot(snapshot);

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(5),
    });

    assert!(core.state().recorded_events.is_empty());
    assert_eq!(core.state().muted_tracks, vec![1]);
    assert!(matches!(
        outputs.as_slice(),
        [
            MachineOutput::SequenceEventsErased {
                selected_track: 1,
                count: 1,
                events,
            },
            MachineOutput::LcdChanged,
        ] if events.len() == 1
            && events[0].selected_track == 1
            && events[0].pad_number == 4
    ));
}

#[test]
fn track_mute_snapshot_defaults_round_trips_sorts_and_validates() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(4),
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(4),
    });

    let json = core
        .to_project_json()
        .expect("track mute snapshot should encode");
    assert!(json.contains(r#""muted_tracks": ["#));

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("track mute snapshot should restore");
    assert_eq!(restored.state().selected_track, 2);
    assert_eq!(restored.state().muted_tracks, vec![1, 2]);
    assert!(restored.state().lcd.lines[1].contains("Mute on/02"));

    let mut older_value =
        serde_json::to_value(MpcCore::new().export_project_snapshot()).expect("snapshot value");
    older_value
        .pointer_mut("/sequence")
        .and_then(serde_json::Value::as_object_mut)
        .expect("snapshot sequence should be an object")
        .remove("muted_tracks");
    let older_json = serde_json::to_string(&older_value).expect("older JSON should encode");
    let mut older = MpcCore::new();
    older
        .restore_project_json(&older_json)
        .expect("older snapshot without muted_tracks should restore");
    assert!(older.state().muted_tracks.is_empty());

    let mut unsorted = MpcCore::new().export_project_snapshot();
    unsorted.sequence.muted_tracks = vec![4, 2];
    let mut unsorted_restored = MpcCore::new();
    unsorted_restored
        .restore_project_snapshot(unsorted)
        .expect("unsorted unique muted tracks should restore");
    assert_eq!(unsorted_restored.state().muted_tracks, vec![2, 4]);

    let mut invalid_track = MpcCore::new().export_project_snapshot();
    invalid_track.sequence.muted_tracks = vec![65];
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_snapshot(invalid_track)
            .expect_err("out-of-range muted track should be rejected"),
        "sequence.muted_tracks[0]",
        "1..=64",
    );

    let mut duplicate_track = MpcCore::new().export_project_snapshot();
    duplicate_track.sequence.muted_tracks = vec![2, 2];
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_snapshot(duplicate_track)
            .expect_err("duplicate muted tracks should be rejected"),
        "sequence.muted_tracks[1]",
        "duplicate muted track",
    );
}

#[test]
fn track_mute_soft_key_4_is_main_only() {
    let modes = [
        (PanelControl::Program, Mode::Program),
        (PanelControl::Sample, Mode::Sample),
        (PanelControl::Trim, Mode::Trim),
        (PanelControl::Song, Mode::Song),
        (PanelControl::Midi, Mode::Midi),
        (PanelControl::Disk, Mode::Disk),
        (PanelControl::Setup, Mode::Setup),
    ];

    for (control, mode) in modes {
        let mut core = MpcCore::new();
        core.dispatch(HardwareEvent::Press { control });

        let outputs = core.dispatch(HardwareEvent::Press {
            control: PanelControl::SoftKey(4),
        });

        assert_eq!(core.state().mode, mode);
        assert!(core.state().muted_tracks.is_empty());
        assert_no_track_mute_outputs(&outputs);
    }
}

#[test]
fn disk_mode_default_screen_shows_project_json_boundary() {
    let mut core = MpcCore::new();

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::Disk,
    });

    assert_eq!(core.state().mode, Mode::Disk);
    assert_eq!(
        core.state().selected_disk_operation,
        DiskOperation::SaveProject
    );
    assert_eq!(
        outputs,
        vec![
            MachineOutput::ModeChanged { mode: Mode::Disk },
            MachineOutput::LcdChanged,
        ]
    );
    assert_eq!(core.state().lcd.title, "DISK");
    assert!(core.state().lcd.lines[0].contains(">Save Project"));
    assert_eq!(core.state().lcd.lines[1], "Project file JSON only");
    assert_eq!(core.state().lcd.lines[2], "Virtual disk via host path");
    assert_eq!(core.state().lcd.lines[3], "No MPC disk/image formats");
    assert_eq!(core.state().lcd.soft_keys[1], "Save");
    assert_eq!(core.state().lcd.soft_keys[2], "Load");
}

#[test]
fn disk_operation_selection_uses_cursor_and_data_wheel_outputs() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Disk,
    });

    let right_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(
        right_outputs,
        vec![
            MachineOutput::DiskOperationSelected {
                operation: DiskOperation::LoadProject,
            },
            MachineOutput::LcdChanged,
        ]
    );
    assert_eq!(
        core.state().selected_disk_operation,
        DiskOperation::LoadProject
    );
    assert!(core.state().lcd.lines[0].contains(">Load Project"));

    let left_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    assert_eq!(
        left_outputs,
        vec![
            MachineOutput::DiskOperationSelected {
                operation: DiskOperation::SaveProject,
            },
            MachineOutput::LcdChanged,
        ]
    );
    assert_eq!(
        core.state().selected_disk_operation,
        DiskOperation::SaveProject
    );

    let positive_wheel = core.dispatch(HardwareEvent::TurnDataWheel { delta: 8 });
    assert_eq!(
        positive_wheel,
        vec![
            MachineOutput::DiskOperationSelected {
                operation: DiskOperation::LoadProject,
            },
            MachineOutput::LcdChanged,
        ]
    );

    let negative_wheel = core.dispatch(HardwareEvent::TurnDataWheel { delta: -2 });
    assert_eq!(
        negative_wheel,
        vec![
            MachineOutput::DiskOperationSelected {
                operation: DiskOperation::SaveProject,
            },
            MachineOutput::LcdChanged,
        ]
    );

    let zero_wheel = core.dispatch(HardwareEvent::TurnDataWheel { delta: 0 });
    assert_eq!(
        zero_wheel,
        vec![MachineOutput::Ignored {
            reason: "disk.data_wheel_zero_delta_ignored".to_string(),
        }]
    );
    assert_eq!(
        core.state().selected_disk_operation,
        DiskOperation::SaveProject
    );
}

#[test]
fn disk_soft_keys_request_save_and_load_project_operations() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Disk,
    });

    let save_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert_eq!(
        save_outputs,
        vec![MachineOutput::DiskOperationRequested {
            operation: DiskOperation::SaveProject,
        }]
    );

    let load_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(3),
    });
    assert_eq!(
        load_outputs,
        vec![MachineOutput::DiskOperationRequested {
            operation: DiskOperation::LoadProject,
        }]
    );

    let unsupported_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(5),
    });
    assert_eq!(
        unsupported_outputs,
        vec![MachineOutput::Ignored {
            reason: "disk.soft_key.5_unmapped".to_string(),
        }]
    );
}

#[test]
fn disk_operation_outputs_serialize_snake_case() {
    let selected = MachineOutput::DiskOperationSelected {
        operation: DiskOperation::LoadProject,
    };
    let requested = MachineOutput::DiskOperationRequested {
        operation: DiskOperation::SaveProject,
    };

    assert_eq!(
        serde_json::to_value(selected).expect("selected output should serialize"),
        serde_json::json!({
            "type": "disk_operation_selected",
            "operation": "load_project"
        })
    );
    assert_eq!(
        serde_json::to_value(requested).expect("requested output should serialize"),
        serde_json::json!({
            "type": "disk_operation_requested",
            "operation": "save_project"
        })
    );
}

#[test]
fn disk_soft_keys_are_isolated_from_non_disk_modes() {
    let mut core = MpcCore::new();

    let main_track_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert_eq!(main_track_outputs, vec![MachineOutput::LcdChanged]);
    assert_eq!(core.state().mode, Mode::Main);
    assert_eq!(core.state().selected_track, 2);
    assert!(
        !main_track_outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::DiskOperationRequested { .. }))
    );

    let main_erase_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(5),
    });
    assert_eq!(
        main_erase_outputs,
        vec![MachineOutput::Ignored {
            reason: "sequence.erase.track_2.no_events".to_string(),
        }]
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    let program_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert!(program_outputs.iter().any(|output| {
        matches!(
            output,
            MachineOutput::PadAssignmentChanged {
                action: PadAssignmentChange::Restored,
                ..
            }
        )
    }));
    assert!(
        !program_outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::DiskOperationRequested { .. }))
    );
}

#[test]
fn disk_operation_project_snapshot_defaults_missing_field_and_round_trips_selection() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Disk,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });

    let json = core
        .to_project_json()
        .expect("DISK operation snapshot should encode");
    assert!(json.contains(r#""selected_disk_operation": "load_project""#));

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("DISK operation snapshot should restore");
    assert_eq!(restored.state().mode, Mode::Disk);
    assert_eq!(
        restored.state().selected_disk_operation,
        DiskOperation::LoadProject
    );
    assert!(restored.state().lcd.lines[0].contains(">Load Project"));

    let mut value =
        serde_json::to_value(MpcCore::new().export_project_snapshot()).expect("snapshot value");
    value
        .pointer_mut("/machine")
        .and_then(serde_json::Value::as_object_mut)
        .expect("snapshot machine should be an object")
        .remove("selected_disk_operation");
    let missing_field_json =
        serde_json::to_string(&value).expect("older snapshot JSON should encode");

    let mut older = MpcCore::new();
    older
        .restore_project_json(&missing_field_json)
        .expect("older snapshot without DISK operation should restore");
    assert_eq!(
        older.state().selected_disk_operation,
        DiskOperation::SaveProject
    );
}

#[test]
fn song_mode_default_screen_shows_chain_editor() {
    let mut core = MpcCore::new();

    assert_eq!(
        core.state().song_steps,
        vec![SongStep {
            sequence_index: 0,
            repeats: 1,
        }]
    );
    assert_eq!(core.state().selected_song_step_index, 0);
    assert_eq!(core.state().selected_song_edit_field, SongEditField::Step);

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::Song,
    });

    assert_eq!(
        outputs,
        vec![
            MachineOutput::ModeChanged { mode: Mode::Song },
            MachineOutput::LcdChanged,
        ]
    );
    assert_eq!(core.state().mode, Mode::Song);
    assert_eq!(core.state().lcd.title, "SONG");
    assert_eq!(core.state().lcd.lines[0], "Step 01/01 Edit step");
    assert_eq!(core.state().lcd.lines[1], ">Step 01   Seq 01");
    assert_eq!(core.state().lcd.lines[2], "Sequence Sequence01");
    assert_eq!(core.state().lcd.lines[3], " Repeats 01");
    assert_eq!(core.state().lcd.soft_keys[1], "Insert");
    assert_eq!(core.state().lcd.soft_keys[2], "Delete");
}

#[test]
fn song_field_selection_cycles_with_cursor_left_right() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Song,
    });

    let sequence_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(sequence_outputs, vec![MachineOutput::LcdChanged]);
    assert_eq!(
        core.state().selected_song_edit_field,
        SongEditField::Sequence
    );
    assert!(core.state().lcd.lines[1].contains(">Seq 01"));

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(
        core.state().selected_song_edit_field,
        SongEditField::Repeats
    );
    assert!(core.state().lcd.lines[3].starts_with(">Repeats"));

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(core.state().selected_song_edit_field, SongEditField::Step);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    assert_eq!(
        core.state().selected_song_edit_field,
        SongEditField::Repeats
    );
}

#[test]
fn song_step_selection_bounds_emit_selected_or_ignored() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Song,
    });

    let previous_at_first = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorUp,
    });
    assert_eq!(
        previous_at_first,
        vec![MachineOutput::Ignored {
            reason: "song.step.previous_unavailable".to_string(),
        }]
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert_eq!(core.state().selected_song_step_index, 1);

    let next_at_last = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorDown,
    });
    assert_eq!(
        next_at_last,
        vec![MachineOutput::Ignored {
            reason: "song.step.next_unavailable".to_string(),
        }]
    );

    let previous_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorUp,
    });
    assert_eq!(
        previous_outputs,
        vec![
            MachineOutput::SongStepSelected {
                index: 0,
                step: SongStep {
                    sequence_index: 0,
                    repeats: 1,
                },
            },
            MachineOutput::LcdChanged,
        ]
    );
    assert_eq!(core.state().selected_song_step_index, 0);
}

#[test]
fn song_data_wheel_edits_step_sequence_and_repeats_with_clamps() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Song,
    });

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    let sequence_outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 });
    assert_eq!(
        sequence_outputs,
        vec![
            MachineOutput::SongStepChanged {
                index: 0,
                field: SongEditField::Sequence,
                step: SongStep {
                    sequence_index: 4,
                    repeats: 1,
                },
            },
            MachineOutput::LcdChanged,
        ]
    );
    assert!(core.state().lcd.lines[1].contains(">Seq 05"));
    assert!(core.state().lcd.lines[2].contains("Sequence05"));

    let zero_outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 0 });
    assert_eq!(
        zero_outputs,
        vec![MachineOutput::Ignored {
            reason: "song.sequence.zero_delta_ignored".to_string(),
        }]
    );

    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MAX });
    assert_eq!(core.state().song_steps[0].sequence_index, 98);
    assert!(core.state().lcd.lines[1].contains(">Seq 99"));
    assert!(core.state().lcd.lines[2].contains("Sequence99"));
    let sequence_boundary = core.dispatch(HardwareEvent::TurnDataWheel { delta: 1 });
    assert_eq!(
        sequence_boundary,
        vec![MachineOutput::Ignored {
            reason: "song.sequence.boundary".to_string(),
        }]
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    let repeats_outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 8 });
    assert_eq!(
        repeats_outputs,
        vec![
            MachineOutput::SongStepChanged {
                index: 0,
                field: SongEditField::Repeats,
                step: SongStep {
                    sequence_index: 98,
                    repeats: 9,
                },
            },
            MachineOutput::LcdChanged,
        ]
    );

    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MIN });
    assert_eq!(core.state().song_steps[0].repeats, 1);
    let repeats_boundary = core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 });
    assert_eq!(
        repeats_boundary,
        vec![MachineOutput::Ignored {
            reason: "song.repeats.boundary".to_string(),
        }]
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(core.state().selected_song_edit_field, SongEditField::Step);
    let step_outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 });
    assert_eq!(
        step_outputs,
        vec![
            MachineOutput::SongStepSelected {
                index: 0,
                step: SongStep {
                    sequence_index: 98,
                    repeats: 1,
                },
            },
            MachineOutput::LcdChanged,
        ]
    );
}

#[test]
fn song_insert_delete_preserves_at_least_one_step() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Song,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 6 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 2 });

    let insert_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert_eq!(core.state().song_steps.len(), 2);
    assert_eq!(core.state().selected_song_step_index, 1);
    assert_eq!(
        core.state().song_steps,
        vec![
            SongStep {
                sequence_index: 6,
                repeats: 3,
            },
            SongStep {
                sequence_index: 6,
                repeats: 1,
            },
        ]
    );
    assert_eq!(
        insert_outputs,
        vec![
            MachineOutput::SongStepInserted {
                index: 1,
                step: SongStep {
                    sequence_index: 6,
                    repeats: 1,
                },
            },
            MachineOutput::SongStepSelected {
                index: 1,
                step: SongStep {
                    sequence_index: 6,
                    repeats: 1,
                },
            },
            MachineOutput::LcdChanged,
        ]
    );

    let delete_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(3),
    });
    assert_eq!(
        delete_outputs,
        vec![
            MachineOutput::SongStepDeleted {
                index: 1,
                step: SongStep {
                    sequence_index: 6,
                    repeats: 1,
                },
            },
            MachineOutput::SongStepSelected {
                index: 0,
                step: SongStep {
                    sequence_index: 6,
                    repeats: 3,
                },
            },
            MachineOutput::LcdChanged,
        ]
    );
    assert_eq!(core.state().song_steps.len(), 1);
    assert_eq!(core.state().selected_song_step_index, 0);

    let delete_last_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(3),
    });
    assert_eq!(
        delete_last_outputs,
        vec![MachineOutput::Ignored {
            reason: "song.delete.last_step_ignored".to_string(),
        }]
    );
    assert_eq!(core.state().song_steps.len(), 1);
}

#[test]
fn song_outputs_serialize_with_stable_snake_case_shape() {
    let changed = MachineOutput::SongStepChanged {
        index: 3,
        field: SongEditField::Repeats,
        step: SongStep {
            sequence_index: 12,
            repeats: 4,
        },
    };
    let inserted = MachineOutput::SongStepInserted {
        index: 4,
        step: SongStep {
            sequence_index: 12,
            repeats: 1,
        },
    };

    assert_eq!(
        serde_json::to_value(changed).expect("song output should serialize"),
        serde_json::json!({
            "type": "song_step_changed",
            "index": 3,
            "field": "repeats",
            "step": {
                "sequence_index": 12,
                "repeats": 4
            }
        })
    );
    assert_eq!(
        serde_json::to_value(inserted).expect("song output should serialize"),
        serde_json::json!({
            "type": "song_step_inserted",
            "index": 4,
            "step": {
                "sequence_index": 12,
                "repeats": 1
            }
        })
    );
}

#[test]
fn song_soft_keys_are_isolated_from_non_song_modes() {
    let mut core = MpcCore::new();

    let main_f2 = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert_eq!(main_f2, vec![MachineOutput::LcdChanged]);
    assert_eq!(core.state().selected_track, 2);
    assert_no_song_outputs(&main_f2);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    let program_f2 = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert!(
        program_f2
            .iter()
            .any(|output| matches!(output, MachineOutput::PadAssignmentChanged { .. }))
    );
    assert_no_song_outputs(&program_f2);

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Disk,
    });
    let disk_f3 = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(3),
    });
    assert_eq!(
        disk_f3,
        vec![MachineOutput::DiskOperationRequested {
            operation: DiskOperation::LoadProject,
        }]
    );
    assert_no_song_outputs(&disk_f3);

    assert_eq!(core.state().song_steps.len(), 1);
}

#[test]
fn song_project_snapshot_defaults_round_trips_and_validates() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Song,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 8 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });

    let json = core
        .to_project_json()
        .expect("SONG metadata snapshot should encode");
    assert!(json.contains(r#""song""#));
    assert!(json.contains(r#""selected_field": "repeats""#));

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("SONG metadata snapshot should restore");
    assert_eq!(restored.state().mode, Mode::Song);
    assert_eq!(
        restored.state().song_steps,
        vec![
            SongStep {
                sequence_index: 8,
                repeats: 5,
            },
            SongStep {
                sequence_index: 8,
                repeats: 1,
            },
        ]
    );
    assert_eq!(restored.state().selected_song_step_index, 1);
    assert_eq!(
        restored.state().selected_song_edit_field,
        SongEditField::Repeats
    );
    assert_eq!(restored.state().lcd.title, "SONG");

    let mut older_value =
        serde_json::to_value(MpcCore::new().export_project_snapshot()).expect("snapshot value");
    older_value
        .as_object_mut()
        .expect("snapshot root should be an object")
        .remove("song");
    let older_json = serde_json::to_string(&older_value).expect("older JSON should encode");
    let mut older = MpcCore::new();
    older
        .restore_project_json(&older_json)
        .expect("older snapshot without song metadata should restore");
    assert_eq!(
        older.state().song_steps,
        vec![SongStep {
            sequence_index: 0,
            repeats: 1,
        }]
    );
    assert_eq!(older.state().selected_song_step_index, 0);
    assert_eq!(older.state().selected_song_edit_field, SongEditField::Step);

    let mut malformed_song_value =
        serde_json::to_value(MpcCore::new().export_project_snapshot()).expect("snapshot value");
    malformed_song_value
        .as_object_mut()
        .expect("snapshot root should be an object")
        .insert("song".to_string(), serde_json::json!({}));
    let malformed_song_json =
        serde_json::to_string(&malformed_song_value).expect("malformed song JSON should encode");
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_json(&malformed_song_json)
            .expect_err("present incomplete song object should be rejected"),
        "song.steps",
        "required field is missing",
    );

    let mut invalid_empty = MpcCore::new().export_project_snapshot();
    invalid_empty.song.steps.clear();
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_snapshot(invalid_empty)
            .expect_err("empty song should be rejected"),
        "song.steps",
        "at least one song step",
    );

    let mut invalid_selected = MpcCore::new().export_project_snapshot();
    invalid_selected.song.selected_step_index = 1;
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_snapshot(invalid_selected)
            .expect_err("out-of-range selected song step should be rejected"),
        "song.selected_step_index",
        "0..=0",
    );

    let mut invalid_sequence = MpcCore::new().export_project_snapshot();
    invalid_sequence.song.steps[0].sequence_index = 99;
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_snapshot(invalid_sequence)
            .expect_err("out-of-range song sequence should be rejected"),
        "song.steps[0].sequence_index",
        "0..=98",
    );

    let invalid_repeats = ProjectSnapshot {
        song: ProjectSongSnapshot {
            steps: vec![SongStep {
                sequence_index: 0,
                repeats: 0,
            }],
            selected_step_index: 0,
            selected_field: SongEditField::Step,
        },
        ..MpcCore::new().export_project_snapshot()
    };
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_snapshot(invalid_repeats)
            .expect_err("out-of-range song repeats should be rejected"),
        "song.steps[0].repeats",
        "1..=99",
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
    assert_eq!(core.state().current_program.pad_assignments.len(), 63);
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
    assert_eq!(core.state().current_program.pad_assignments.len(), 63);

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
    assert_eq!(core.state().current_program.pad_assignments.len(), 64);
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

    assert_eq!(midi_core.state().midi_input_channel, None);
    assert_eq!(midi_core.state().midi_base_note, 36);
    assert_eq!(
        midi_core.state().selected_midi_settings_field,
        MidiSettingsField::InputChannel
    );

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
fn midi_note_on_uses_bank_a_even_when_another_bank_is_active() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::PadBankD,
    });
    assert_eq!(core.state().pad_bank, PadBank::D);

    let outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 1,
        note: 36,
        velocity: 91,
    });

    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiNoteMapped {
            bank: PadBank::A,
            pad: 1,
            ..
        }
    )));
    assert_eq!(core.state().pad_bank, PadBank::A);
    assert_eq!(
        core.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::A,
            pad_number: 1
        }
    );
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackIntent { intent }
            if intent.bank == PadBank::A
                && intent.pad_number == 1
                && intent.sample_id == "synthetic_a_01"
                && intent.velocity == 91
    )));
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
fn midi_settings_mode_edits_base_note_and_mapping_follows_base() {
    let mut core = MpcCore::new();

    let mode_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::Midi,
    });
    assert_eq!(core.state().mode, Mode::Midi);
    assert!(
        mode_outputs
            .iter()
            .any(|output| matches!(output, MachineOutput::LcdChanged))
    );
    assert_eq!(core.state().lcd.title, "MIDI");
    assert!(core.state().lcd.lines[0].contains("Input Omni"));
    assert!(core.state().lcd.lines[1].contains("Base 036 Range 036-051"));
    assert!(core.state().lcd.lines[2].contains("Host MIDI I/O: off"));

    let cursor_outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(
        core.state().selected_midi_settings_field,
        MidiSettingsField::BaseNote
    );
    assert!(cursor_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiSettingsChanged {
            input_channel: None,
            base_note: 36,
            selected_field: MidiSettingsField::BaseNote
        }
    )));

    let edit_outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 });
    assert_eq!(core.state().midi_base_note, 40);
    assert!(edit_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiSettingsChanged {
            input_channel: None,
            base_note: 40,
            selected_field: MidiSettingsField::BaseNote
        }
    )));
    assert!(core.state().lcd.lines[1].contains("Base 040 Range 040-055"));

    let mapped_outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 9,
        note: 40,
        velocity: 90,
    });
    assert!(mapped_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiNoteMapped {
            channel: 9,
            note: 40,
            bank: PadBank::A,
            pad: 1,
            velocity: 90
        }
    )));
    assert!(mapped_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackIntent { intent }
            if intent.bank == PadBank::A
                && intent.pad_number == 1
                && intent.sample_id == "synthetic_a_01"
    )));

    let top_outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 9,
        note: 55,
        velocity: 91,
    });
    assert!(top_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiNoteMapped {
            note: 55,
            pad: 16,
            ..
        }
    )));

    let ignored_outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 9,
        note: 39,
        velocity: 90,
    });
    assert_eq!(
        ignored_outputs,
        vec![MachineOutput::MidiInputIgnored {
            reason: "midi note 39 is not mapped in this slice; mapped range is 40..=55".to_string(),
        }]
    );

    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MAX });
    assert_eq!(core.state().midi_base_note, 112);
    assert!(core.state().lcd.lines[1].contains("Base 112 Range 112-127"));
    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MIN });
    assert_eq!(core.state().midi_base_note, 0);
    assert!(core.state().lcd.lines[1].contains("Base 000 Range 000-015"));
}

#[test]
fn midi_settings_input_channel_filter_blocks_non_matching_channel_and_accepts_match() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Midi,
    });
    let channel_outputs = core.dispatch(HardwareEvent::TurnDataWheel { delta: 2 });
    assert_eq!(core.state().midi_input_channel, Some(2));
    assert!(channel_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiSettingsChanged {
            input_channel: Some(2),
            base_note: 36,
            selected_field: MidiSettingsField::InputChannel
        }
    )));
    assert!(core.state().lcd.lines[0].contains("Input Ch 02"));

    let blocked_outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 1,
        note: 36,
        velocity: 100,
    });
    assert_eq!(
        blocked_outputs,
        vec![MachineOutput::MidiInputIgnored {
            reason: "midi channel 1 ignored; input channel is 2".to_string(),
        }]
    );
    assert_eq!(core.state().last_playback, None);
    assert!(core.state().recorded_events.is_empty());

    let accepted_outputs = core.dispatch(HardwareEvent::MidiNoteOn {
        channel: 2,
        note: 36,
        velocity: 101,
    });
    assert!(accepted_outputs.iter().any(|output| matches!(
        output,
        MachineOutput::MidiNoteMapped {
            channel: 2,
            note: 36,
            pad: 1,
            velocity: 101,
            ..
        }
    )));
    assert!(matches!(
        &core.state().last_playback,
        Some(SamplePlaybackResolution::Intent { intent })
            if intent.pad_number == 1 && intent.velocity == 101
    ));

    let note_off_outputs = core.dispatch(HardwareEvent::MidiNoteOff {
        channel: 1,
        note: 36,
        velocity: 64,
    });
    assert_eq!(
        note_off_outputs,
        vec![MachineOutput::MidiInputIgnored {
            reason: "midi note-off is a no-op in this slice".to_string(),
        }]
    );

    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MAX });
    assert_eq!(core.state().midi_input_channel, Some(16));
    core.dispatch(HardwareEvent::TurnDataWheel { delta: i32::MIN });
    assert_eq!(core.state().midi_input_channel, None);
}

#[test]
fn midi_settings_project_snapshot_defaults_missing_fields_and_round_trips_explicit_settings() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Midi,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 3 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 });

    let json = core
        .to_project_json()
        .expect("MIDI settings snapshot should encode");
    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("MIDI settings snapshot should restore");

    assert_eq!(restored.state().mode, Mode::Midi);
    assert_eq!(restored.state().midi_input_channel, Some(3));
    assert_eq!(restored.state().midi_base_note, 40);
    assert_eq!(
        restored.state().selected_midi_settings_field,
        MidiSettingsField::BaseNote
    );
    assert!(restored.state().lcd.lines[0].contains("Input Ch 03"));
    assert!(restored.state().lcd.lines[1].contains("Base 040 Range 040-055"));

    let mut value =
        serde_json::to_value(MpcCore::new().export_project_snapshot()).expect("snapshot value");
    let machine = value
        .pointer_mut("/machine")
        .and_then(serde_json::Value::as_object_mut)
        .expect("snapshot machine should be an object");
    machine.remove("midi_input_channel");
    machine.remove("midi_base_note");
    machine.remove("selected_midi_settings_field");
    let missing_fields_json =
        serde_json::to_string(&value).expect("older snapshot JSON should encode");

    let mut older = MpcCore::new();
    older
        .restore_project_json(&missing_fields_json)
        .expect("older snapshot without MIDI fields should restore");
    assert_eq!(older.state().midi_input_channel, None);
    assert_eq!(older.state().midi_base_note, 36);
    assert_eq!(
        older.state().selected_midi_settings_field,
        MidiSettingsField::InputChannel
    );
}

#[test]
fn midi_settings_changed_output_serializes_stably() {
    let output = MachineOutput::MidiSettingsChanged {
        input_channel: None,
        base_note: 36,
        selected_field: MidiSettingsField::InputChannel,
    };

    let json = serde_json::to_value(output).expect("MIDI settings output should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "midi_settings_changed",
            "input_channel": null,
            "base_note": 36,
            "selected_field": "input_channel"
        })
    );
}

#[test]
fn setup_mode_default_screen_shows_preferences() {
    let mut core = MpcCore::new();

    assert_eq!(
        core.state().setup_preferences,
        setup_preferences(true, 0, 5)
    );
    assert_eq!(core.state().selected_setup_field, SetupField::Metronome);

    let outputs = core.dispatch(HardwareEvent::Press {
        control: PanelControl::Setup,
    });

    assert_eq!(core.state().mode, Mode::Setup);
    assert_eq!(core.state().lcd.title, "SETUP");
    assert_eq!(core.state().lcd.lines[0], ">Metronome On");
    assert_eq!(core.state().lcd.lines[1], " Count-in bars 0");
    assert_eq!(core.state().lcd.lines[2], " LCD contrast 05");
    assert_eq!(core.state().lcd.lines[3], "Edit metronome");
    assert_eq!(
        core.state().lcd.soft_keys,
        ["F1", "F2", "F3", "F4", "F5", "F6"].map(std::string::ToString::to_string)
    );
    assert_eq!(
        outputs,
        vec![
            MachineOutput::ModeChanged { mode: Mode::Setup },
            MachineOutput::LcdChanged,
        ]
    );
}

#[test]
fn setup_cursor_left_right_cycles_fields_and_emits_settings_output() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Setup,
    });

    let right_to_count = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(core.state().selected_setup_field, SetupField::CountInBars);
    assert_eq!(core.state().lcd.lines[1], ">Count-in bars 0");
    assert_eq!(
        right_to_count,
        setup_changed_outputs(setup_preferences(true, 0, 5), SetupField::CountInBars)
    );

    let right_to_contrast = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(core.state().selected_setup_field, SetupField::LcdContrast);
    assert_eq!(core.state().lcd.lines[2], ">LCD contrast 05");
    assert_eq!(
        right_to_contrast,
        setup_changed_outputs(setup_preferences(true, 0, 5), SetupField::LcdContrast)
    );

    let right_to_metronome = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(core.state().selected_setup_field, SetupField::Metronome);
    assert_eq!(
        right_to_metronome,
        setup_changed_outputs(setup_preferences(true, 0, 5), SetupField::Metronome)
    );

    let left_to_contrast = core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorLeft,
    });
    assert_eq!(core.state().selected_setup_field, SetupField::LcdContrast);
    assert_eq!(
        left_to_contrast,
        setup_changed_outputs(setup_preferences(true, 0, 5), SetupField::LcdContrast)
    );
}

#[test]
fn setup_data_wheel_edits_preferences_clamps_and_ignores_zero_delta() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Setup,
    });

    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 0 }),
        vec![MachineOutput::Ignored {
            reason: "setup.metronome.zero_delta_ignored".to_string(),
        }]
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 1 }),
        vec![MachineOutput::Ignored {
            reason: "setup.metronome.boundary".to_string(),
        }]
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 }),
        setup_changed_outputs(setup_preferences(false, 0, 5), SetupField::Metronome)
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 }),
        vec![MachineOutput::Ignored {
            reason: "setup.metronome.boundary".to_string(),
        }]
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 0 }),
        vec![MachineOutput::Ignored {
            reason: "setup.count_in_bars.zero_delta_ignored".to_string(),
        }]
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 4 }),
        setup_changed_outputs(setup_preferences(false, 4, 5), SetupField::CountInBars)
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 1 }),
        vec![MachineOutput::Ignored {
            reason: "setup.count_in_bars.boundary".to_string(),
        }]
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: -4 }),
        setup_changed_outputs(setup_preferences(false, 0, 5), SetupField::CountInBars)
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 0 }),
        vec![MachineOutput::Ignored {
            reason: "setup.lcd_contrast.zero_delta_ignored".to_string(),
        }]
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 5 }),
        setup_changed_outputs(setup_preferences(false, 0, 10), SetupField::LcdContrast)
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: 1 }),
        vec![MachineOutput::Ignored {
            reason: "setup.lcd_contrast.boundary".to_string(),
        }]
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: -10 }),
        setup_changed_outputs(setup_preferences(false, 0, 0), SetupField::LcdContrast)
    );
    assert_eq!(
        core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 }),
        vec![MachineOutput::Ignored {
            reason: "setup.lcd_contrast.boundary".to_string(),
        }]
    );
}

#[test]
fn setup_project_snapshot_defaults_round_trips_and_validates() {
    let mut core = MpcCore::new();
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Setup,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: -1 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 2 });
    core.dispatch(HardwareEvent::Press {
        control: PanelControl::CursorRight,
    });
    core.dispatch(HardwareEvent::TurnDataWheel { delta: 3 });

    let json = core
        .to_project_json()
        .expect("SETUP preferences snapshot should encode");
    assert!(json.contains(r#""setup""#));
    assert!(json.contains(r#""selected_field": "lcd_contrast""#));

    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("SETUP preferences snapshot should restore");
    assert_eq!(restored.state().mode, Mode::Setup);
    assert_eq!(
        restored.state().setup_preferences,
        setup_preferences(false, 2, 8)
    );
    assert_eq!(
        restored.state().selected_setup_field,
        SetupField::LcdContrast
    );
    assert_eq!(restored.state().lcd.title, "SETUP");
    assert_eq!(restored.state().lcd.lines[2], ">LCD contrast 08");

    let mut older_value =
        serde_json::to_value(MpcCore::new().export_project_snapshot()).expect("snapshot value");
    older_value
        .as_object_mut()
        .expect("snapshot root should be an object")
        .remove("setup");
    let older_json = serde_json::to_string(&older_value).expect("older JSON should encode");
    let mut older = MpcCore::new();
    older
        .restore_project_json(&older_json)
        .expect("older snapshot without setup metadata should restore");
    assert_eq!(
        older.state().setup_preferences,
        setup_preferences(true, 0, 5)
    );
    assert_eq!(older.state().selected_setup_field, SetupField::Metronome);

    let mut malformed_setup_value =
        serde_json::to_value(MpcCore::new().export_project_snapshot()).expect("snapshot value");
    malformed_setup_value
        .as_object_mut()
        .expect("snapshot root should be an object")
        .insert("setup".to_string(), serde_json::json!({}));
    let malformed_setup_json =
        serde_json::to_string(&malformed_setup_value).expect("malformed setup JSON should encode");
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_json(&malformed_setup_json)
            .expect_err("present incomplete setup object should be rejected"),
        "setup.preferences",
        "required field is missing",
    );

    let mut malformed_preferences_value =
        serde_json::to_value(MpcCore::new().export_project_snapshot()).expect("snapshot value");
    *malformed_preferences_value
        .pointer_mut("/setup/preferences")
        .expect("setup preferences should exist") = serde_json::json!({
        "metronome_enabled": true,
        "lcd_contrast": 5
    });
    let malformed_preferences_json = serde_json::to_string(&malformed_preferences_value)
        .expect("malformed preferences JSON should encode");
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_json(&malformed_preferences_json)
            .expect_err("present incomplete setup preferences should be rejected"),
        "setup.preferences.count_in_bars",
        "required field is missing",
    );

    let mut invalid_count = MpcCore::new().export_project_snapshot();
    invalid_count.setup.preferences.count_in_bars = 5;
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_snapshot(invalid_count)
            .expect_err("out-of-range setup count-in should be rejected"),
        "setup.preferences.count_in_bars",
        "0..=4",
    );

    let invalid_contrast = ProjectSnapshot {
        setup: ProjectSetupSnapshot {
            preferences: setup_preferences(true, 0, 11),
            selected_field: SetupField::Metronome,
        },
        ..MpcCore::new().export_project_snapshot()
    };
    assert_invalid_project_field(
        MpcCore::new()
            .restore_project_snapshot(invalid_contrast)
            .expect_err("out-of-range setup LCD contrast should be rejected"),
        "setup.preferences.lcd_contrast",
        "0..=10",
    );
}

#[test]
fn setup_preferences_changed_output_serializes_stably() {
    let output = MachineOutput::SetupPreferencesChanged {
        preferences: setup_preferences(false, 2, 8),
        selected_field: SetupField::LcdContrast,
    };

    let json = serde_json::to_value(output).expect("SETUP preferences output should serialize");

    assert_eq!(
        json,
        serde_json::json!({
            "type": "setup_preferences_changed",
            "preferences": {
                "metronome_enabled": false,
                "count_in_bars": 2,
                "lcd_contrast": 8
            },
            "selected_field": "lcd_contrast"
        })
    );
}

#[test]
fn setup_soft_keys_are_unmapped_and_isolated_from_other_modes() {
    let mut core = MpcCore::new();

    let main_f2 = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert_no_setup_outputs(&main_f2);
    assert_eq!(
        core.state().setup_preferences,
        setup_preferences(true, 0, 5)
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Setup,
    });
    let setup_f2 = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert_eq!(
        setup_f2,
        vec![MachineOutput::Ignored {
            reason: "setup.soft_key.2_unmapped".to_string(),
        }]
    );
    assert_no_setup_outputs(&setup_f2);
    assert_eq!(
        core.state().setup_preferences,
        setup_preferences(true, 0, 5)
    );

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Disk,
    });
    let disk_f2 = core.dispatch(HardwareEvent::Press {
        control: PanelControl::SoftKey(2),
    });
    assert!(
        disk_f2
            .iter()
            .any(|output| matches!(output, MachineOutput::DiskOperationRequested { .. }))
    );
    assert_no_setup_outputs(&disk_f2);
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
            playback: Some(sample_playback_intent_for_bank_pad(PadBank::C, 4, 64)),
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
        ("setup", "/setup", "audio_bytes", "setup.audio_bytes"),
        (
            "setup preferences",
            "/setup/preferences",
            "service_menu",
            "setup.preferences.service_menu",
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
fn project_snapshot_round_trip_preserves_active_pad_bank_and_selected_pad_bank() {
    let mut core = MpcCore::new();

    core.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    core.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::C,
        pad: 9,
        velocity: 92,
    });

    let json = core.to_project_json().expect("snapshot JSON should encode");
    let mut restored = MpcCore::new();
    restored
        .restore_project_json(&json)
        .expect("snapshot JSON should restore");

    assert_eq!(restored.state().pad_bank, PadBank::C);
    assert_eq!(
        restored.state().selected_program_pad,
        ProgramPad {
            bank: PadBank::C,
            pad_number: 9
        }
    );
    assert_eq!(restored.state().current_program.pad_assignments.len(), 64);
    assert!(restored.state().lcd.lines[1].contains("C09"));
    assert!(restored.state().lcd.lines[1].contains("SYN-C09"));
}

#[test]
fn old_a_only_project_snapshot_restores_missing_banks_as_unassigned() {
    let mut snapshot = MpcCore::new().export_project_snapshot();
    snapshot
        .program
        .pad_assignments
        .retain(|assignment| assignment.pad.bank == PadBank::A);

    let mut restored = MpcCore::new();
    restored
        .restore_project_snapshot(snapshot)
        .expect("A-only snapshot should remain valid");
    assert_eq!(restored.state().current_program.pad_assignments.len(), 16);

    restored.dispatch(HardwareEvent::Press {
        control: PanelControl::Program,
    });
    restored.dispatch(HardwareEvent::Press {
        control: PanelControl::PadBankB,
    });
    let outputs = restored.dispatch(HardwareEvent::StrikePad {
        bank: PadBank::B,
        pad: 1,
        velocity: 88,
    });

    assert_eq!(restored.state().pad_bank, PadBank::B);
    assert!(outputs.iter().any(|output| matches!(
        output,
        MachineOutput::SamplePlaybackMiss { miss }
            if miss.bank == PadBank::B
                && miss.pad_number == 1
                && miss.velocity == 88
                && miss.reason == PlaybackMissReason::PadUnassigned
    )));
    assert!(matches!(
        &restored.state().last_playback,
        Some(SamplePlaybackResolution::Miss { miss })
            if miss.bank == PadBank::B && miss.reason == PlaybackMissReason::PadUnassigned
    ));
    assert!(restored.state().lcd.lines[1].contains("B01"));
    assert!(restored.state().lcd.lines[1].contains("unassigned"));
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

fn snapshot_with_recorded_assigned_events(events: &[(u64, u8, u8)]) -> ProjectSnapshot {
    let core = MpcCore::new();
    let mut snapshot = core.export_project_snapshot();
    snapshot.sequence.recorded_events = events
        .iter()
        .map(|(tick, pad, velocity)| SequenceEvent {
            selected_track: 1,
            pad_bank: PadBank::A,
            pad_number: *pad,
            velocity: *velocity,
            tick: *tick,
            playback: Some(sample_playback_intent_for_pad(*pad, *velocity)),
        })
        .collect();
    snapshot.machine.event_count = snapshot.sequence.recorded_events.len() as u64;
    snapshot
}

fn snapshot_with_recorded_track_events(events: &[(u8, u64, PadBank, u8, u8)]) -> ProjectSnapshot {
    let core = MpcCore::new();
    let mut snapshot = core.export_project_snapshot();
    snapshot.sequence.recorded_events = events
        .iter()
        .map(
            |(selected_track, tick, bank, pad, velocity)| SequenceEvent {
                selected_track: *selected_track,
                pad_bank: *bank,
                pad_number: *pad,
                velocity: *velocity,
                tick: *tick,
                playback: Some(sample_playback_intent_for_track_bank_pad(
                    *selected_track,
                    *bank,
                    *pad,
                    *velocity,
                )),
            },
        )
        .collect();
    snapshot.machine.event_count = snapshot.sequence.recorded_events.len() as u64;
    snapshot
}

fn sample_playback_intent_for_pad(pad: u8, velocity: u8) -> SamplePlaybackIntent {
    sample_playback_intent_for_bank_pad(PadBank::A, pad, velocity)
}

fn sample_playback_intent_for_bank_pad(
    bank: PadBank,
    pad: u8,
    velocity: u8,
) -> SamplePlaybackIntent {
    sample_playback_intent_for_track_bank_pad(1, bank, pad, velocity)
}

fn sample_playback_intent_for_track_bank_pad(
    selected_track: u8,
    bank: PadBank,
    pad: u8,
    velocity: u8,
) -> SamplePlaybackIntent {
    let bank_label = bank.label();
    let length_frames = mpc_core::generated_sample_length_frames(ProgramPad {
        bank,
        pad_number: pad,
    });
    SamplePlaybackIntent {
        selected_track,
        program_index: 1,
        program_name: "Program01".to_string(),
        bank,
        pad_number: pad,
        sample_id: format!("synthetic_{}_{pad:02}", bank_label.to_ascii_lowercase()),
        sample_name: format!("SYN-{bank_label}{pad:02}"),
        velocity,
        level: 100,
        pan: 0,
        tune_cents: 0,
        start_frame: 0,
        end_frame: length_frames.saturating_sub(1),
        window_length_frames: length_frames,
    }
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

fn assert_no_song_outputs(outputs: &[MachineOutput]) {
    assert!(
        outputs.iter().all(|output| !matches!(
            output,
            MachineOutput::SongStepSelected { .. }
                | MachineOutput::SongStepChanged { .. }
                | MachineOutput::SongStepInserted { .. }
                | MachineOutput::SongStepDeleted { .. }
        )),
        "non-SONG output sequence must not contain song outputs: {outputs:?}"
    );
}

fn assert_no_setup_outputs(outputs: &[MachineOutput]) {
    assert!(
        outputs
            .iter()
            .all(|output| !matches!(output, MachineOutput::SetupPreferencesChanged { .. })),
        "non-SETUP output sequence must not contain setup outputs: {outputs:?}"
    );
}

fn assert_no_track_mute_outputs(outputs: &[MachineOutput]) {
    assert!(
        outputs
            .iter()
            .all(|output| !matches!(output, MachineOutput::TrackMuteChanged { .. })),
        "non-MAIN output sequence must not contain track mute outputs: {outputs:?}"
    );
}

fn setup_preferences(
    metronome_enabled: bool,
    count_in_bars: u8,
    lcd_contrast: u8,
) -> SetupPreferences {
    SetupPreferences {
        metronome_enabled,
        count_in_bars,
        lcd_contrast,
    }
}

fn setup_changed_outputs(
    preferences: SetupPreferences,
    selected_field: SetupField,
) -> Vec<MachineOutput> {
    vec![
        MachineOutput::SetupPreferencesChanged {
            preferences,
            selected_field,
        },
        MachineOutput::LcdChanged,
    ]
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
