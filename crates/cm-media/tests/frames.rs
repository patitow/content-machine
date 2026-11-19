#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::cell::Cell;
use std::ffi::OsString;
use std::path::PathBuf;

use cm_domain::Recording;
use cm_media::{CommandOutput, CommandRunner, MediaError, SystemRunner, probe, sample_frames};

#[derive(Default)]
struct CountingRunner {
    calls: Cell<u32>,
}

impl CommandRunner for CountingRunner {
    fn run(&self, _program: &str, _args: &[OsString]) -> Result<CommandOutput, MediaError> {
        self.calls.set(self.calls.get() + 1);
        Ok(CommandOutput {
            stdout: vec![],
            stderr: String::new(),
        })
    }
}

fn recording_of(secs: u32, dir: &std::path::Path, name: &str) -> Recording {
    let path = dir.join(name);
    common::video_with_audio_tracks(&path, &[], secs);
    probe(&SystemRunner, &path).unwrap()
}

#[test]
fn returns_requested_number_of_full_frames_in_increasing_time() {
    let d = tempfile::tempdir().unwrap();
    let rec = recording_of(20, d.path(), "a.mkv");
    let frames = sample_frames(&SystemRunner, &rec, 8).unwrap();
    assert_eq!(frames.len(), 8);
    for f in &frames {
        assert_eq!((f.width, f.height), (320, 180));
        assert_eq!(f.data.len(), 320 * 180 * 3);
        assert!(f.time_s >= 0.05 * rec.duration_s - 0.01);
        assert!(f.time_s <= 0.95 * rec.duration_s + 0.01);
    }
    assert!(frames.windows(2).all(|w| w[0].time_s < w[1].time_s));
}

#[test]
fn short_video_does_not_fail_or_repeat_times() {
    let d = tempfile::tempdir().unwrap();
    let rec = recording_of(1, d.path(), "curto.mkv");
    let frames = sample_frames(&SystemRunner, &rec, 8).unwrap();
    assert!(!frames.is_empty());
    assert!(frames.windows(2).all(|w| w[0].time_s < w[1].time_s));
}

#[test]
fn zero_count_never_calls_ffmpeg() {
    let runner = CountingRunner::default();
    let rec = Recording {
        path: PathBuf::from("x.mkv"),
        duration_s: 10.0,
        width: 320,
        height: 180,
        fps: 30.0,
        audio_tracks: vec![],
    };
    assert!(sample_frames(&runner, &rec, 0).unwrap().is_empty());
    assert_eq!(runner.calls.get(), 0);
}

#[test]
fn path_with_leading_dot_spaces_and_accents_works() {
    let d = tempfile::tempdir().unwrap();
    let nested = d.path().join(".GRAVAÇÕES com espaço");
    std::fs::create_dir(&nested).unwrap();
    let rec = recording_of(6, &nested, "sessão 1.mkv");
    let frames = sample_frames(&SystemRunner, &rec, 3).unwrap();
    assert_eq!(frames.len(), 3);
}
