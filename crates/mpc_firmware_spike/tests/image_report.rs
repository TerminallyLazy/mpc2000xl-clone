use mpc_firmware_spike::inspect_image;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn image_report_never_stores_firmware_bytes() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("mpc-synthetic-{unique}.bin"));
    fs::write(&path, [0x4d, 0x50, 0x43, 0x32]).expect("synthetic image should write");

    let report = inspect_image(&path).expect("synthetic image should inspect");
    fs::remove_file(&path).expect("synthetic image should be removable");

    assert_eq!(report.byte_len, 4);
    assert!(!report.sha256.is_empty());
    assert!(!report.stores_firmware_bytes);
}
