use std::path::PathBuf;

#[test]
fn all_json_fixtures_with_source_references_pass() {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut fixture_paths = std::fs::read_dir(&fixture_dir)
        .expect("fixture directory should be readable")
        .map(|entry| {
            entry
                .expect("fixture directory entry should be readable")
                .path()
        })
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    fixture_paths.sort();

    assert!(
        !fixture_paths.is_empty(),
        "fixture directory has no JSON files"
    );

    for fixture_path in fixture_paths {
        let report = mpc_conformance::run_fixture_path(&fixture_path)
            .unwrap_or_else(|error| panic!("{}: {error:#}", fixture_path.display()));

        assert!(
            report.passed,
            "{} ({}) failed: {:?}",
            report.id, report.name, report.details
        );
    }
}
