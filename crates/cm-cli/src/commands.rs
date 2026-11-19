use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{Context, bail};
use cm_domain::TrackRole;
use cm_media::{SystemRunner, fingerprint, ingest, probe};
use cm_store::Store;

use crate::{Config, latest_recording};

const SETTLE: Duration = Duration::from_secs(10);

fn role_name(role: TrackRole) -> &'static str {
    match role {
        TrackRole::Mix => "mix",
        TrackRole::Game => "jogo",
        TrackRole::Comms => "comunicação",
        TrackRole::Mic => "microfone",
        TrackRole::Unknown => "desconhecida",
    }
}

fn resolve_latest(cfg: &Config) -> anyhow::Result<PathBuf> {
    latest_recording(&cfg.obs_dir, SystemTime::now(), SETTLE)
        .with_context(|| format!("não foi possível ler a pasta {}", cfg.obs_dir.display()))?
        .with_context(|| format!("nenhuma gravação encontrada em {}", cfg.obs_dir.display()))
}

pub fn latest_command(cfg: &Config, out: &mut dyn Write) -> anyhow::Result<()> {
    writeln!(out, "{}", resolve_latest(cfg)?.display())?;
    Ok(())
}

/// `target` é um caminho de arquivo ou a palavra `latest`.
pub fn ingest_command(cfg: &Config, target: &str, out: &mut dyn Write) -> anyhow::Result<()> {
    let path = if target == "latest" {
        resolve_latest(cfg)?
    } else {
        PathBuf::from(target)
    };
    if !path.is_file() {
        bail!("arquivo não encontrado: {}", path.display());
    }
    let rec = probe(&SystemRunner, &path).context("falha ao analisar a gravação")?;
    let fp = fingerprint(&path)?;
    std::fs::create_dir_all(&cfg.cache_dir)?;
    let store = Store::open(&cfg.db_path)?;
    store.upsert_recording(&fp, &rec)?;

    writeln!(out, "Gravação: {}", path.display())?;
    writeln!(
        out,
        "Duração: {:.1} s | {}x{} | {:.2} fps",
        rec.duration_s, rec.width, rec.height, rec.fps
    )?;
    if rec.audio_tracks.is_empty() {
        writeln!(out, "Faixas de áudio: nenhuma")?;
    } else {
        let tracks: Vec<String> = rec
            .audio_tracks
            .iter()
            .map(|t| format!("{} ({})", t.index, role_name(t.role)))
            .collect();
        writeln!(out, "Faixas de áudio: {}", tracks.join(", "))?;
    }

    let result = ingest(&SystemRunner, &rec, &fp, &cfg.cache_dir)
        .context("falha ao extrair áudio e proxy")?;
    let dir = result
        .video_proxy
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    if result.from_cache {
        writeln!(out, "Ingest: já em cache em {dir}")?;
    } else {
        writeln!(out, "Ingest: concluído em {dir}")?;
    }
    Ok(())
}
