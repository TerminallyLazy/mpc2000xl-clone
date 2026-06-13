use mpc_conformance::{load_fixture, run_fixture};

#[test]
fn fixture_with_source_reference_passes() {
    let fixture = load_fixture(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/main_screen.json"
    ))
    .expect("fixture should load");

    let report = run_fixture(&fixture);

    assert!(report.passed, "{:?}", report.details);
    assert_eq!(report.id, "core.main.program-mode");
}
