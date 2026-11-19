#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use cm_domain::{Rect, WebcamDetection, WebcamSource};
use cm_store::Store;
use predicates::prelude::*;

fn setup(root: &Path, extra_config: &str) -> (PathBuf, PathBuf) {
    let cache = root.join("cache");
    let cfg = root.join("config.toml");
    std::fs::write(
        &cfg,
        format!(
            "obs_dir = '{}'\ncache_dir = '{}'\n{extra_config}",
            root.join("obs").display(),
            cache.display()
        ),
    )
    .unwrap();
    let video = root.join("sessao.mkv");
    common::video_with_audio_tracks(&video, &["Mix"], 6);
    (cfg, video)
}

fn cortes(cfg: &Path) -> Command {
    let mut c = Command::cargo_bin("cortes").unwrap();
    c.env("CORTES_CONFIG", cfg);
    c
}

fn detection(x: u32) -> WebcamDetection {
    WebcamDetection {
        rect: Rect {
            x,
            y: 20,
            w: 100,
            h: 60,
        },
        confidence: 0.9,
        source: WebcamSource::Detected,
    }
}

#[test]
fn manual_override_wins_without_detecting() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, video) = setup(
        d.path(),
        "[webcam_overrides.sessao]\nx = 10\ny = 20\nw = 300\nh = 200\n",
    );
    cortes(&cfg)
        .arg("webcam")
        .arg(&video)
        .assert()
        .success()
        .stdout(predicate::str::contains("x=10 y=20 w=300 h=200"))
        .stdout(predicate::str::contains("manual"));
}

#[test]
fn video_without_a_webcam_reports_game_only_and_succeeds() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, video) = setup(d.path(), "");
    cortes(&cfg)
        .arg("webcam")
        .arg(&video)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Webcam: não encontrada, corte será só do jogo",
        ));
}

#[test]
fn stored_rect_is_read_from_the_database() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, video) = setup(d.path(), "");
    std::fs::create_dir_all(d.path().join("cache")).unwrap();
    let rec = cm_media::probe(&cm_media::SystemRunner, &video).unwrap();
    let fp = cm_media::fingerprint(&video).unwrap();
    let store = Store::open(&d.path().join("cache").join("cortes.db")).unwrap();
    store.upsert_recording(&fp, &rec).unwrap();
    store
        .save_webcam(&fp, &detection(7), rec.width, rec.height)
        .unwrap();
    cortes(&cfg)
        .arg("webcam")
        .arg(&video)
        .assert()
        .success()
        .stdout(predicate::str::contains("x=7 y=20 w=100 h=60"))
        .stdout(predicate::str::contains("já salva"));
}

#[test]
fn history_rect_is_not_used_when_no_face_falls_inside_it() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, video) = setup(d.path(), "");
    std::fs::create_dir_all(d.path().join("cache")).unwrap();
    let rec = cm_media::probe(&cm_media::SystemRunner, &video).unwrap();
    let store = Store::open(&d.path().join("cache").join("cortes.db")).unwrap();
    store.upsert_recording("outra", &rec).unwrap();
    store
        .save_webcam("outra", &detection(5), rec.width, rec.height)
        .unwrap();
    cortes(&cfg)
        .arg("webcam")
        .arg(&video)
        .assert()
        .success()
        .stdout(predicate::str::contains("não encontrada"));
}

#[test]
fn missing_file_fails_cleanly() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, _video) = setup(d.path(), "");
    cortes(&cfg)
        .args(["webcam", "nao-existe.mkv"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Erro"));
}
