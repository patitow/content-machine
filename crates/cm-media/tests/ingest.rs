mod common;

use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::path::PathBuf;

use cm_domain::{AudioTrack, Recording, TrackRole};
use cm_media::{
    CommandOutput, CommandRunner, MediaError, SystemRunner, fingerprint, ingest,
    ingest_with_version, probe,
};

/// Runner falso: registra chamadas e cria os arquivos de saída que o ffmpeg criaria.
#[derive(Default)]
struct FakeRunner {
    ffmpeg_calls: Cell<u32>,
    fail_after_first_output: Cell<bool>,
    log: RefCell<Vec<Vec<OsString>>>,
}

impl CommandRunner for FakeRunner {
    fn run(&self, program: &str, args: &[OsString]) -> Result<CommandOutput, MediaError> {
        if program == "ffmpeg" {
            self.ffmpeg_calls.set(self.ffmpeg_calls.get() + 1);
            self.log.borrow_mut().push(args.to_vec());
            let outputs: Vec<PathBuf> = args
                .iter()
                .map(PathBuf::from)
                .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("f32" | "gray")))
                .collect();
            for (i, out) in outputs.iter().enumerate() {
                std::fs::write(out, b"x")?;
                if self.fail_after_first_output.get() && i == 0 {
                    return Err(MediaError::NonZeroExit {
                        program: "ffmpeg".into(),
                        code: 1,
                        stderr: "No space left on device".into(),
                    });
                }
            }
        }
        Ok(CommandOutput {
            stdout: vec![],
            stderr: String::new(),
        })
    }
}

fn recording(tracks: usize) -> Recording {
    Recording {
        path: PathBuf::from("gravacao.mkv"),
        duration_s: 10.0,
        width: 1920,
        height: 1080,
        fps: 60.0,
        audio_tracks: (0..tracks)
            .map(|i| AudioTrack {
                index: i as u32,
                title: None,
                role: TrackRole::Unknown,
            })
            .collect(),
    }
}

#[test]
fn one_ffmpeg_invocation_produces_all_outputs() {
    let cache = tempfile::tempdir().unwrap();
    let runner = FakeRunner::default();
    let out = ingest(&runner, &recording(4), "fp1", cache.path()).unwrap();
    assert_eq!(runner.ffmpeg_calls.get(), 1);
    assert_eq!(out.audio_files.len(), 4);
    assert!(out.audio_files.iter().all(|(_, p)| p.exists()));
    assert!(out.video_proxy.exists());
    assert!(!out.from_cache);
}

#[test]
fn second_ingest_comes_from_cache_without_running_ffmpeg() {
    let cache = tempfile::tempdir().unwrap();
    let runner = FakeRunner::default();
    ingest(&runner, &recording(2), "fp1", cache.path()).unwrap();
    let again = ingest(&runner, &recording(2), "fp1", cache.path()).unwrap();
    assert!(again.from_cache);
    assert_eq!(again.audio_files.len(), 2);
    assert_eq!(runner.ffmpeg_calls.get(), 1);
}

#[test]
fn recording_without_audio_has_no_audio_files() {
    let cache = tempfile::tempdir().unwrap();
    let runner = FakeRunner::default();
    let out = ingest(&runner, &recording(0), "fp1", cache.path()).unwrap();
    assert!(out.audio_files.is_empty());
    assert!(out.video_proxy.exists());
}

#[test]
fn failed_ffmpeg_leaves_no_valid_cache_and_next_run_retries() {
    let cache = tempfile::tempdir().unwrap();
    let runner = FakeRunner::default();
    runner.fail_after_first_output.set(true);
    assert!(ingest(&runner, &recording(2), "fp1", cache.path()).is_err());

    runner.fail_after_first_output.set(false);
    let out = ingest(&runner, &recording(2), "fp1", cache.path()).unwrap();
    assert!(!out.from_cache);
    assert_eq!(runner.ffmpeg_calls.get(), 2);
    assert_eq!(out.audio_files.len(), 2);
}

#[test]
fn stale_tmp_dir_from_a_killed_run_is_discarded() {
    let cache = tempfile::tempdir().unwrap();
    let stale = cache.path().join("fp1.v1.tmp");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("lixo.f32"), b"parcial").unwrap();
    let runner = FakeRunner::default();
    let out = ingest(&runner, &recording(1), "fp1", cache.path()).unwrap();
    assert!(!out.from_cache);
    assert!(!stale.exists());
}

#[test]
fn changing_ingest_version_invalidates_cache() {
    let cache = tempfile::tempdir().unwrap();
    let runner = FakeRunner::default();
    ingest_with_version(&runner, &recording(1), "fp1", cache.path(), 1).unwrap();
    let out = ingest_with_version(&runner, &recording(1), "fp1", cache.path(), 2).unwrap();
    assert!(!out.from_cache);
    assert_eq!(runner.ffmpeg_calls.get(), 2);
}

#[test]
fn real_ffmpeg_ingest_outputs_expected_sizes() {
    let dir = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let video = dir.path().join("sessao.mkv");
    common::video_with_audio_tracks(&video, &["Mix", "Jogo (padrao)", "Comunicacao", "Mic"], 10);
    let rec = probe(&SystemRunner, &video).unwrap();
    let fp = fingerprint(&video).unwrap();

    let out = ingest(&SystemRunner, &rec, &fp, cache.path()).unwrap();

    assert_eq!(out.audio_files.len(), 4);
    let expected_audio = 10 * 16000 * 4;
    for (_, p) in &out.audio_files {
        let len = std::fs::metadata(p).unwrap().len() as i64;
        assert!(
            (len - expected_audio).abs() < expected_audio / 20,
            "áudio com {len} bytes, esperado ≈ {expected_audio}"
        );
    }
    let frames = std::fs::metadata(&out.video_proxy).unwrap().len() / (320 * 180);
    assert!(
        (4..=6).contains(&frames),
        "proxy com {frames} quadros, esperado ≈ 5 (um por quadro-chave de 2 s)"
    );
}
