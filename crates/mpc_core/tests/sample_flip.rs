use mpc_core::{
    apply_sample_flip_plan_to_project_snapshot, build_pad_bank_sample_flip_plan, MpcCore, PadBank,
    ProgramPad, SampleFlipRegion, SampleFlipSource, SampleSourceKind, SAMPLE_FLIP_PAD_COUNT,
};

#[test]
fn builds_equal_slices_across_a_pad_bank() {
    let plan = build_pad_bank_sample_flip_plan(test_source(1_600), PadBank::B, None)
        .expect("valid source should build a flip plan");

    assert_eq!(plan.bank, PadBank::B);
    assert_eq!(
        plan.region,
        SampleFlipRegion {
            start_frame: 0,
            end_frame: 1_599,
        }
    );
    assert_eq!(plan.slices.len(), usize::from(SAMPLE_FLIP_PAD_COUNT));
    assert_eq!(
        plan.slices[0].pad,
        ProgramPad {
            bank: PadBank::B,
            pad_number: 1,
        }
    );
    assert_eq!(plan.slices[0].start_frame, 0);
    assert_eq!(plan.slices[0].end_frame, 99);
    assert_eq!(plan.slices[0].sample_id, "flip_test_loop_1_b_p01");
    assert_eq!(
        plan.slices[15].pad,
        ProgramPad {
            bank: PadBank::B,
            pad_number: 16,
        }
    );
    assert_eq!(plan.slices[15].start_frame, 1_500);
    assert_eq!(plan.slices[15].end_frame, 1_599);
}

#[test]
fn applies_flip_plan_as_imported_trimmed_bank_metadata() {
    let mut core = MpcCore::new();
    let mut snapshot = core.export_project_snapshot();
    let plan = build_pad_bank_sample_flip_plan(
        test_source(1_600),
        PadBank::C,
        Some(SampleFlipRegion {
            start_frame: 160,
            end_frame: 959,
        }),
    )
    .expect("valid source region should build a flip plan");

    apply_sample_flip_plan_to_project_snapshot(&mut snapshot, &plan)
        .expect("valid plan should apply to project metadata");

    let bank_c_assignments = snapshot
        .program
        .pad_assignments
        .iter()
        .filter(|assignment| assignment.pad.bank == PadBank::C)
        .collect::<Vec<_>>();
    assert_eq!(bank_c_assignments.len(), usize::from(SAMPLE_FLIP_PAD_COUNT));
    assert!(bank_c_assignments
        .iter()
        .all(|assignment| assignment.sample.source_kind == SampleSourceKind::Imported));
    assert_eq!(
        snapshot.program.sample_trims.len(),
        usize::from(SAMPLE_FLIP_PAD_COUNT)
    );
    assert_eq!(
        snapshot.program.imported_media_references.len(),
        usize::from(SAMPLE_FLIP_PAD_COUNT)
    );
    assert_eq!(snapshot.machine.pad_bank, PadBank::C);
    assert_eq!(
        snapshot.machine.selected_program_pad,
        ProgramPad {
            bank: PadBank::C,
            pad_number: 1,
        }
    );
    assert_eq!(
        snapshot.machine.selected_sample_id.as_deref(),
        Some("flip_test_loop_1_c_p01")
    );

    core.restore_project_snapshot(snapshot)
        .expect("sample flip project metadata should pass core validation");
    assert_eq!(core.state().pad_bank, PadBank::C);
    assert_eq!(
        core.state().selected_sample_id.as_deref(),
        Some("flip_test_loop_1_c_p01")
    );
}

#[test]
fn rejects_regions_that_cannot_fill_all_sixteen_pads() {
    let error = build_pad_bank_sample_flip_plan(test_source(8), PadBank::A, None)
        .expect_err("short source should be rejected");

    assert!(error.to_string().contains("cannot fill 16 pad"));
}

#[test]
fn rejects_empty_sources_before_region_math() {
    let error = build_pad_bank_sample_flip_plan(test_source(0), PadBank::A, None)
        .expect_err("empty source should be rejected");

    assert!(error.to_string().contains("has no frames"));
}

fn test_source(frame_count: u32) -> SampleFlipSource {
    SampleFlipSource {
        source_id: "Test Loop #1".to_string(),
        source_title: "Test Loop #1".to_string(),
        source_path: "local-assets/samples/user-authorized/test-loop-1.wav".to_string(),
        managed_copy_path: None,
        sample_rate_hz: 44_100,
        frame_count,
        byte_count: usize::try_from(frame_count)
            .unwrap_or(usize::MAX)
            .saturating_mul(4),
    }
}
