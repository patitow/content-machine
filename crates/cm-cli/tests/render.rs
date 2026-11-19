#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;

fn setup(root: &Path, extra_config: &str) -> (PathBuf, PathBuf) {
    let cfg = root.join("config.toml");
    std::fs::write(
        &cfg,
        format!(
            "obs_dir = '{}'\ncache_dir = '{}'\n{extra_config}",
            root.join("obs").display(),
            root.join("cache").display()
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

fn dims(video: &Path) -> (u32, u32) {
    let rec = cm_media::probe(&cm_media::SystemRunner, video).unwrap();
    (rec.width, rec.height)
}

#[test]
fn render_sample_with_manual_webcam_writes_a_vertical_clip() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, video) = setup(
        d.path(),
        "[webcam_overrides.sessao]\nx = 0\ny = 60\nw = 120\nh = 80\n",
    );
    let out = d.path().join("corte.mp4");
    cortes(&cfg)
        .arg("render-sample")
        .arg(&video)
        .args(["--start", "1", "--dur", "2", "--out"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains("Corte 9:16 gravado em"));
    assert_eq!(dims(&out), (1080, 1920));
}

#[test]
fn render_sample_without_webcam_warns_and_still_renders() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, video) = setup(d.path(), "");
    let out = d.path().join("solo.mp4");
    cortes(&cfg)
        .arg("render-sample")
        .arg(&video)
        .args(["--start", "0", "--dur", "2", "--out"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "sem webcam confiável: corte só do jogo",
        ));
    assert_eq!(dims(&out), (1080, 1920));
}

#[test]
fn duration_longer_than_the_video_is_clamped() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, video) = setup(d.path(), "");
    let out = d.path().join("curto.mp4");
    cortes(&cfg)
        .arg("render-sample")
        .arg(&video)
        .args(["--start", "4", "--dur", "60", "--out"])
        .arg(&out)
        .assert()
        .success();
    let rec = cm_media::probe(&cm_media::SystemRunner, &out).unwrap();
    assert!(rec.duration_s < 3.5, "{}", rec.duration_s);
}

#[test]
fn start_past_the_end_is_a_clear_error() {
    let d = tempfile::tempdir().unwrap();
    let (cfg, video) = setup(d.path(), "");
    cortes(&cfg)
        .arg("render-sample")
        .arg(&video)
        .args(["--start", "999"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Erro"));
}
