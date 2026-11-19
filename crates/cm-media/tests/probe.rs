mod common;

use cm_domain::TrackRole;
use cm_media::{MediaError, SystemRunner, probe};
use common::{ffmpeg, video_with_audio_tracks as make};

#[test]
fn probes_mkv_with_four_titled_audio_tracks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gameplay.mkv");
    make(&path, &["Mix", "Jogo (padrao)", "Comunicacao", "Mic"], 2);

    let rec = probe(&SystemRunner, &path).unwrap();

    assert_eq!((rec.width, rec.height), (320, 180));
    assert!((rec.fps - 30.0).abs() < 0.01);
    assert!((rec.duration_s - 2.0).abs() < 0.5);
    let roles: Vec<TrackRole> = rec.audio_tracks.iter().map(|t| t.role).collect();
    assert_eq!(
        roles,
        vec![
            TrackRole::Mix,
            TrackRole::Game,
            TrackRole::Comms,
            TrackRole::Mic
        ]
    );
    assert_eq!(rec.audio_tracks[1].title.as_deref(), Some("Jogo (padrao)"));
}

#[test]
fn probes_video_without_audio() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mudo.mp4");
    make(&path, &[], 2);

    let rec = probe(&SystemRunner, &path).unwrap();

    assert!(rec.audio_tracks.is_empty());
}

#[test]
fn probes_path_with_leading_dot_spaces_and_accents() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join(".pasta com espaço e acentuação ção");
    std::fs::create_dir_all(&nested).unwrap();
    let path = nested.join("gravação 2026-07-27.mkv");
    make(&path, &["Mix"], 2);

    let rec = probe(&SystemRunner, &path).unwrap();

    assert_eq!(rec.audio_tracks.len(), 1);
}

#[test]
fn rejects_non_media_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nota.txt");
    std::fs::write(&path, "isto não é um vídeo").unwrap();

    let err = probe(&SystemRunner, &path).unwrap_err();

    assert!(matches!(
        err,
        MediaError::Parse(_) | MediaError::NonZeroExit { .. }
    ));
}

#[test]
fn rejects_audio_only_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("so-audio.mka");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=f=440:d=1",
        path.to_str().unwrap(),
    ]);

    let err = probe(&SystemRunner, &path).unwrap_err();

    assert!(matches!(err, MediaError::NoVideoStream));
}
