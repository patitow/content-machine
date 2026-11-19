use cm_media::fingerprint;

const MIB: usize = 1024 * 1024;

fn bytes(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i.wrapping_mul(31) % 251) as u8).collect()
}

#[test]
fn same_content_at_two_paths_has_same_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    std::fs::write(&a, bytes(4 * MIB)).unwrap();
    std::fs::write(&b, bytes(4 * MIB)).unwrap();
    assert_eq!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
}

#[test]
fn changing_last_byte_changes_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    let mut data = bytes(4 * MIB);
    std::fs::write(&a, &data).unwrap();
    *data.last_mut().unwrap() ^= 0xFF;
    std::fs::write(&b, &data).unwrap();
    assert_ne!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
}

#[test]
fn changing_middle_byte_changes_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    let mut data = bytes(4 * MIB);
    std::fs::write(&a, &data).unwrap();
    data[2 * MIB] ^= 0xFF;
    std::fs::write(&b, &data).unwrap();
    assert_ne!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
}

#[test]
fn files_smaller_than_three_mib_work() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("small.bin");
    std::fs::write(&a, bytes(1000)).unwrap();
    assert!(!fingerprint(&a).unwrap().is_empty());
}

#[test]
fn missing_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(fingerprint(&dir.path().join("nao-existe.bin")).is_err());
}
