#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::path::Path;
use std::time::{Duration, SystemTime};

use assert_cmd::Command;
use predicates::prelude::*;

fn set_mtime(path: &std::path::Path, time: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

/// Config de teste apontando para pastas temporárias; devolve o caminho do arquivo.
fn config(root: &Path) -> std::path::PathBuf {
    let obs = root.join("obs");
    std::fs::create_dir_all(&obs).unwrap();
    let cfg = root.join("config.toml");
    let toml = format!(
        "obs_dir = '{}'\ncache_dir = '{}'\n",
        obs.display(),
        root.join("cache").display()
    );
    std::fs::write(&cfg, toml).unwrap();
    cfg
}

fn settled_recording(root: &Path, name: &str) -> std::path::PathBuf {
    let p = root.join("obs").join(name);
    common::video_with_audio_tracks(&p, &["Mix", "Jogo (padrao)", "Comunicacao", "Mic"], 6);
    let old = SystemTime::now() - Duration::from_secs(600);
    set_mtime(&p, old);
    p
}

fn cortes(cfg: &Path) -> Command {
    let mut c = Command::cargo_bin("cortes").unwrap();
    c.env("CORTES_CONFIG", cfg);
    c
}

#[test]
fn latest_prints_the_newest_recording_path() {
    let d = tempfile::tempdir().unwrap();
    let cfg = config(d.path());
    let rec = settled_recording(d.path(), "sessao.mkv");
    cortes(&cfg)
        .arg("latest")
        .assert()
        .success()
        .stdout(predicate::str::contains(rec.to_string_lossy().as_ref()));
}

#[test]
fn ingest_prints_summary_in_portuguese_and_stores_the_recording() {
    let d = tempfile::tempdir().unwrap();
    let cfg = config(d.path());
    let rec = settled_recording(d.path(), "sessao.mkv");
    cortes(&cfg)
        .arg("ingest")
        .arg(&rec)
        .assert()
        .success()
        .stdout(predicate::str::contains("320x180"))
        .stdout(predicate::str::contains("jogo"))
        .stdout(predicate::str::contains("Ingest: concluído"));

    let store = cm_store::Store::open(&d.path().join("cache").join("cortes.db")).unwrap();
    let fp = cm_media::fingerprint(&rec).unwrap();
    assert!(store.find_recording(&fp).unwrap().is_some());
}

#[test]
fn second_ingest_reports_cache() {
    let d = tempfile::tempdir().unwrap();
    let cfg = config(d.path());
    let rec = settled_recording(d.path(), "sessao.mkv");
    cortes(&cfg).arg("ingest").arg(&rec).assert().success();
    cortes(&cfg)
        .arg("ingest")
        .arg(&rec)
        .assert()
        .success()
        .stdout(predicate::str::contains("já em cache"));
}

#[test]
fn ingest_latest_resolves_the_newest_recording() {
    let d = tempfile::tempdir().unwrap();
    let cfg = config(d.path());
    settled_recording(d.path(), "sessao.mkv");
    cortes(&cfg)
        .args(["ingest", "latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Ingest: concluído"));
}

#[test]
fn ingest_of_missing_file_fails_cleanly_in_portuguese() {
    let d = tempfile::tempdir().unwrap();
    let cfg = config(d.path());
    cortes(&cfg)
        .args(["ingest", "nao-existe.mkv"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Erro"))
        .stderr(predicate::str::contains("panicked").not());
}
