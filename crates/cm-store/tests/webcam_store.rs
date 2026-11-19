#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use cm_domain::{Recording, Rect, WebcamDetection, WebcamSource};
use cm_store::Store;

fn detection(x: u32, source: WebcamSource) -> WebcamDetection {
    WebcamDetection {
        rect: Rect {
            x,
            y: 438,
            w: 408,
            h: 284,
        },
        confidence: 0.86,
        source,
    }
}

fn recording() -> Recording {
    Recording {
        path: PathBuf::from("a.mkv"),
        duration_s: 10.0,
        width: 1920,
        height: 1080,
        fps: 60.0,
        audio_tracks: vec![],
    }
}

#[test]
fn v1_database_migrates_to_v2_keeping_recordings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("antigo.db");
    {
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.execute_batch(
            "CREATE TABLE recording (fingerprint TEXT PRIMARY KEY, path TEXT NOT NULL,
               duration_s REAL NOT NULL, width INTEGER NOT NULL, height INTEGER NOT NULL, fps REAL NOT NULL);
             CREATE TABLE audio_track (fingerprint TEXT NOT NULL REFERENCES recording(fingerprint) ON DELETE CASCADE,
               idx INTEGER NOT NULL, title TEXT, role TEXT NOT NULL, PRIMARY KEY (fingerprint, idx));
             INSERT INTO recording VALUES ('fp1', 'a.mkv', 10.0, 1920, 1080, 60.0);",
        )
        .unwrap();
        raw.pragma_update(None, "user_version", 1).unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert!(store.find_recording("fp1").unwrap().is_some());
    assert!(store.find_webcam("fp1").unwrap().is_none());
}

#[test]
fn save_then_find_roundtrips_a_detection() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("s.db")).unwrap();
    store.upsert_recording("fp1", &recording()).unwrap();
    let d = detection(0, WebcamSource::Detected);
    store.save_webcam("fp1", &d, 1920, 1080).unwrap();
    assert_eq!(store.find_webcam("fp1").unwrap(), Some(d));
}

#[test]
fn saving_again_replaces_the_previous_rect() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("s.db")).unwrap();
    store.upsert_recording("fp1", &recording()).unwrap();
    store
        .save_webcam("fp1", &detection(0, WebcamSource::Detected), 1920, 1080)
        .unwrap();
    let newer = detection(10, WebcamSource::Override);
    store.save_webcam("fp1", &newer, 1920, 1080).unwrap();
    assert_eq!(store.find_webcam("fp1").unwrap(), Some(newer));
}

#[test]
fn last_webcam_for_resolution_returns_the_latest_detected_of_that_size() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("s.db")).unwrap();
    for (fp, x, w, h) in [
        ("a", 1, 1920, 1080),
        ("b", 2, 1920, 1080),
        ("c", 3, 1280, 720),
    ] {
        store.upsert_recording(fp, &recording()).unwrap();
        store
            .save_webcam(fp, &detection(x, WebcamSource::Detected), w, h)
            .unwrap();
    }
    let last = store
        .last_webcam_for_resolution(1920, 1080)
        .unwrap()
        .unwrap();
    assert_eq!(last.x, 2);
    assert!(
        store
            .last_webcam_for_resolution(640, 480)
            .unwrap()
            .is_none()
    );
}

#[test]
fn overrides_and_saved_rects_are_not_reused_as_history() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("s.db")).unwrap();
    store.upsert_recording("a", &recording()).unwrap();
    store
        .save_webcam("a", &detection(5, WebcamSource::Override), 1920, 1080)
        .unwrap();
    assert!(
        store
            .last_webcam_for_resolution(1920, 1080)
            .unwrap()
            .is_none()
    );
}
