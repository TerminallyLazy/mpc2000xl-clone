use mpc_firmware_spike::inspect_image;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn image_report_never_stores_firmware_bytes() {
    let mut file = NamedTempFile::new().expect("synthetic image should be creatable");
    file.write_all(b"MPC2")
        .expect("synthetic image should write");
    file.flush().expect("synthetic image should flush");

    let report = inspect_image(file.path()).expect("synthetic image should inspect");

    assert_eq!(report.byte_len, 4);
    assert_eq!(
        report.sha256,
        "05e71909ec817edba4a8c4cc7a55f0d8c7bc0a592f7a12ae272f5fbfcc44e427"
    );
    assert!(!report.stores_firmware_bytes);

    let json = serde_json::to_string(&report).expect("report should serialize");
    assert!(!json.contains("MPC2"));
    assert!(!json.contains("4d504332"));
}
