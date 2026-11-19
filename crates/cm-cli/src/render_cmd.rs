use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, bail};
use cm_media::{SystemRunner, fingerprint, plan_layout, probe, render_9x16};
use cm_store::Store;

use crate::webcam_cmd::{describe, resolve_webcam};
use crate::{Config, latest_recording};

pub struct RenderSampleArgs {
    pub target: String,
    pub start_s: f64,
    pub dur_s: f64,
    pub out: Option<PathBuf>,
}

/// Renderiza um trecho 9:16 de teste: webcam em cima (se achada) e jogo embaixo.
pub fn render_sample_command(
    cfg: &Config,
    args: &RenderSampleArgs,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let path = if args.target == "latest" {
        latest_recording(
            &cfg.obs_dir,
            std::time::SystemTime::now(),
            std::time::Duration::from_secs(10),
        )?
        .with_context(|| format!("nenhuma gravação encontrada em {}", cfg.obs_dir.display()))?
    } else {
        PathBuf::from(&args.target)
    };
    if !path.is_file() {
        bail!("arquivo não encontrado: {}", path.display());
    }
    let rec = probe(&SystemRunner, &path).context("falha ao analisar a gravação")?;
    if args.start_s < 0.0 || args.start_s >= rec.duration_s {
        bail!(
            "início em {:.1} s fora da gravação (duração {:.1} s)",
            args.start_s,
            rec.duration_s
        );
    }
    let dur = args.dur_s.min(rec.duration_s - args.start_s);
    if dur <= 0.0 {
        bail!("duração do corte precisa ser maior que zero");
    }

    let fp = fingerprint(&path)?;
    std::fs::create_dir_all(&cfg.cache_dir)?;
    let store = Store::open(&cfg.db_path)?;
    store.upsert_recording(&fp, &rec)?;
    let resolved = resolve_webcam(cfg, &rec, &fp, &store)?;
    writeln!(out, "{}", describe(&resolved))?;
    if resolved.detection.is_none() {
        writeln!(out, "Aviso: sem webcam confiável: corte só do jogo")?;
    }

    let layout = plan_layout(rec.width, rec.height, resolved.detection.map(|d| d.rect));
    let target = match &args.out {
        Some(p) => p.clone(),
        None => {
            let dir = cfg.cache_dir.join("renders");
            std::fs::create_dir_all(&dir)?;
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "corte".into());
            dir.join(format!("{stem}_9x16_{:.0}s.mp4", args.start_s))
        }
    };
    render_9x16(&SystemRunner, &rec, &layout, args.start_s, dur, &target)
        .context("falha ao renderizar o corte")?;
    writeln!(out, "Corte 9:16 gravado em {}", target.display())?;
    Ok(())
}
