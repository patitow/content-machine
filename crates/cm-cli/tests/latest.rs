#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use cm_cli::latest_recording;

fn set_mtime(path: &std::path::Path, time: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

const SETTLE: Duration = Duration::from_secs(10);

fn file(dir: &Path, name: &str, bytes: usize, age_s: u64) {
    let p = dir.join(name);
    fs::write(&p, vec![1u8; bytes]).unwrap();
    let t = SystemTime::now() - Duration::from_secs(age_s);
    set_mtime(&p, t);
}

fn latest(dir: &Path) -> Option<String> {
    latest_recording(dir, SystemTime::now(), SETTLE)
        .unwrap()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
}

#[test]
fn returns_newest_settled_mkv_or_mp4() {
    let d = tempfile::tempdir().unwrap();
    file(d.path(), "a.mkv", 10, 300);
    file(d.path(), "b.mp4", 10, 100);
    file(d.path(), "c.mkv", 10, 200);
    assert_eq!(latest(d.path()).as_deref(), Some("b.mp4"));
}

#[test]
fn ignores_zero_byte_files() {
    let d = tempfile::tempdir().unwrap();
    file(d.path(), "boa.mkv", 10, 300);
    file(d.path(), "vazia.mkv", 0, 100);
    assert_eq!(latest(d.path()).as_deref(), Some("boa.mkv"));
}

#[test]
fn ignores_files_still_being_written() {
    let d = tempfile::tempdir().unwrap();
    file(d.path(), "pronta.mkv", 10, 300);
    file(d.path(), "gravando.mkv", 10, 1);
    assert_eq!(latest(d.path()).as_deref(), Some("pronta.mkv"));
}

#[test]
fn same_stem_mkv_and_mp4_returns_the_mkv() {
    let d = tempfile::tempdir().unwrap();
    file(d.path(), "2026-06-17 21-11-27.mkv", 10, 300);
    file(d.path(), "2026-06-17 21-11-27.mp4", 10, 100);
    assert_eq!(latest(d.path()).as_deref(), Some("2026-06-17 21-11-27.mkv"));
}

#[test]
fn ignores_subfolders_and_other_extensions() {
    let d = tempfile::tempdir().unwrap();
    file(d.path(), "real.mkv", 10, 300);
    file(d.path(), "capa.png", 10, 100);
    file(d.path(), "real.mkv.bak", 10, 100);
    fs::create_dir(d.path().join("SourceRecord")).unwrap();
    file(&d.path().join("SourceRecord"), "mais-nova.mkv", 10, 100);
    assert_eq!(latest(d.path()).as_deref(), Some("real.mkv"));
}

#[test]
fn empty_folder_gives_none() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(latest(d.path()), None);
}

#[test]
fn missing_folder_is_an_error() {
    let d = tempfile::tempdir().unwrap();
    assert!(latest_recording(&d.path().join("nao-existe"), SystemTime::now(), SETTLE).is_err());
}

#[test]
fn works_in_folder_with_leading_dot_and_accents() {
    let d = tempfile::tempdir().unwrap();
    let nested = d.path().join(".GRAVAÇÕES com espaço");
    fs::create_dir(&nested).unwrap();
    file(&nested, "sessão.mkv", 10, 300);
    assert_eq!(latest(&nested).as_deref(), Some("sessão.mkv"));
}
