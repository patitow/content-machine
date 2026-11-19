use std::path::PathBuf;

use cm_domain::{AudioTrack, Recording, TrackRole};
use cm_store::{Store, StoreError};

fn sample() -> Recording {
    Recording {
        path: PathBuf::from(r"E:\.VIDEOS GRAVADOS\OBS\2026-07-27 23-05-35.mkv"),
        duration_s: 1001.5,
        width: 1920,
        height: 1080,
        fps: 60.0,
        audio_tracks: vec![
            AudioTrack {
                index: 0,
                title: Some("Mix".into()),
                role: TrackRole::Mix,
            },
            AudioTrack {
                index: 1,
                title: None,
                role: TrackRole::Unknown,
            },
        ],
    }
}

#[test]
fn open_creates_schema_in_wal_mode() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("cortes.db");
    let _store = Store::open(&db).unwrap();
    let raw = rusqlite::Connection::open(&db).unwrap();
    let mode: String = raw
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn upsert_then_find_roundtrips_recording_with_roles() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("cortes.db")).unwrap();
    store.upsert_recording("abc123", &sample()).unwrap();
    assert_eq!(store.find_recording("abc123").unwrap(), Some(sample()));
}

#[test]
fn upsert_twice_does_not_duplicate_tracks() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("cortes.db")).unwrap();
    store.upsert_recording("abc123", &sample()).unwrap();
    store.upsert_recording("abc123", &sample()).unwrap();
    let found = store.find_recording("abc123").unwrap().unwrap();
    assert_eq!(found.audio_tracks.len(), 2);
}

#[test]
fn unknown_fingerprint_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("cortes.db")).unwrap();
    assert_eq!(store.find_recording("nope").unwrap(), None);
}

#[test]
fn reopening_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("cortes.db");
    Store::open(&db)
        .unwrap()
        .upsert_recording("abc123", &sample())
        .unwrap();
    let reopened = Store::open(&db).unwrap();
    assert_eq!(reopened.find_recording("abc123").unwrap(), Some(sample()));
}

#[test]
fn schema_newer_than_supported_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("cortes.db");
    {
        let raw = rusqlite::Connection::open(&db).unwrap();
        raw.pragma_update(None, "user_version", 99).unwrap();
    }
    assert!(matches!(
        Store::open(&db),
        Err(StoreError::SchemaTooNew { found: 99, .. })
    ));
}
