use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use cm_domain::Recording;
use serde::{Deserialize, Serialize};

use crate::runner::{CommandRunner, MediaError};

/// Versão do formato de saída do ingest; mudar invalida o cache.
pub const INGEST_VERSION: u32 = 1;

const MANIFEST: &str = "manifest.json";
const VIDEO_PROXY: &str = "video_proxy.gray";

#[derive(Debug, Clone, PartialEq)]
pub struct IngestOutput {
    pub fingerprint: String,
    /// (índice da faixa de áudio, arquivo f32le mono 16 kHz)
    pub audio_files: Vec<(u32, PathBuf)>,
    /// quadros gray 320x180 de quadros-chave, concatenados
    pub video_proxy: PathBuf,
    pub from_cache: bool,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    ingest_version: u32,
    audio: Vec<(u32, String)>,
    video_proxy: String,
}

fn output_from(dir: &Path, fingerprint: &str, m: &Manifest, from_cache: bool) -> IngestOutput {
    IngestOutput {
        fingerprint: fingerprint.to_string(),
        audio_files: m.audio.iter().map(|(i, f)| (*i, dir.join(f))).collect(),
        video_proxy: dir.join(&m.video_proxy),
        from_cache,
    }
}

fn cached(dir: &Path, fingerprint: &str, version: u32) -> Option<IngestOutput> {
    let raw = fs::read(dir.join(MANIFEST)).ok()?;
    let m: Manifest = serde_json::from_slice(&raw).ok()?;
    let out = output_from(dir, fingerprint, &m, true);
    let complete = m.ingest_version == version
        && out.video_proxy.exists()
        && out.audio_files.iter().all(|(_, p)| p.exists());
    complete.then_some(out)
}

pub fn ingest(
    runner: &dyn CommandRunner,
    rec: &Recording,
    fingerprint: &str,
    cache_dir: &Path,
) -> Result<IngestOutput, MediaError> {
    ingest_with_version(runner, rec, fingerprint, cache_dir, INGEST_VERSION)
}

/// Uma única invocação do ffmpeg lê o arquivo uma vez e produz áudio por faixa
/// e o proxy de vídeo. A saída vai para um diretório temporário e só é movida
/// para o nome final depois do sucesso, com o manifesto como marca de completo.
#[doc(hidden)]
pub fn ingest_with_version(
    runner: &dyn CommandRunner,
    rec: &Recording,
    fingerprint: &str,
    cache_dir: &Path,
    version: u32,
) -> Result<IngestOutput, MediaError> {
    let final_dir = cache_dir.join(format!("{fingerprint}.v{version}"));
    let tmp_dir = cache_dir.join(format!("{fingerprint}.v{version}.tmp"));
    if let Some(hit) = cached(&final_dir, fingerprint, version) {
        return Ok(hit);
    }
    fs::create_dir_all(cache_dir)?;
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;

    let mut args: Vec<OsString> = ["-v", "error", "-y", "-skip_frame", "nokey", "-i"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(rec.path.as_os_str().to_owned());
    let mut audio = Vec::new();
    for track in &rec.audio_tracks {
        let file = format!("audio_{}.f32", track.index);
        args.extend(
            ["-map".to_string(), format!("0:a:{}", track.index)]
                .into_iter()
                .map(OsString::from),
        );
        args.extend(["-ac", "1", "-ar", "16000", "-f", "f32le"].map(OsString::from));
        args.push(tmp_dir.join(&file).into_os_string());
        audio.push((track.index, file));
    }
    args.extend(
        [
            "-map",
            "0:v:0",
            "-vf",
            "scale=320:180,format=gray",
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
        ]
        .map(OsString::from),
    );
    args.push(tmp_dir.join(VIDEO_PROXY).into_os_string());

    if let Err(e) = runner.run("ffmpeg", &args) {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(e);
    }

    let manifest = Manifest {
        ingest_version: version,
        audio,
        video_proxy: VIDEO_PROXY.to_string(),
    };
    let written = serde_json::to_vec(&manifest)
        .map_err(|e| MediaError::Parse(e.to_string()))
        .and_then(|raw| fs::write(tmp_dir.join(MANIFEST), raw).map_err(MediaError::from));
    if let Err(e) = written {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(e);
    }
    if final_dir.exists() {
        fs::remove_dir_all(&final_dir)?;
    }
    fs::rename(&tmp_dir, &final_dir)?;
    Ok(output_from(&final_dir, fingerprint, &manifest, false))
}
