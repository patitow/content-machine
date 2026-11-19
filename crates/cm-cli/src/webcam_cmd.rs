use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, bail};
use cm_domain::{Recording, WebcamDetection, WebcamSource};
use cm_media::{SystemRunner, fingerprint, probe, sample_frames};
use cm_store::Store;
use cm_vision::{FaceDetector, WebcamKind, WebcamOutcome, detect_webcam};

use crate::{Config, latest_recording};

/// Quadros de 1080p amostrados por gravação para achar a webcam.
const SAMPLE_COUNT: usize = 16;
/// Confiança mínima para aceitar uma detecção nova.
const MIN_CONFIDENCE: f64 = 0.6;
/// Fração dos quadros com rosto dentro do retângulo salvo para reaproveitá-lo.
const SAVED_MIN_FACE_FRACTION: f64 = 0.6;

#[derive(Debug)]
pub struct Resolved {
    pub detection: Option<WebcamDetection>,
    /// Só conhecido quando a detecção acabou de rodar.
    pub kind: Option<WebcamKind>,
    /// Quando já estava no banco (não precisou detectar).
    pub from_store: bool,
    /// Por que não achou (para avisar o usuário).
    pub reason: Option<String>,
}

fn resolved(detection: Option<WebcamDetection>) -> Resolved {
    Resolved {
        detection,
        kind: None,
        from_store: false,
        reason: None,
    }
}

/// Cadeia de decisão: configuração manual, banco, detecção, retângulo de gravação
/// parecida, e por fim nada (corte só do jogo; nunca um recorte chutado).
pub fn resolve_webcam(
    cfg: &Config,
    rec: &Recording,
    fp: &str,
    store: &Store,
) -> anyhow::Result<Resolved> {
    let stem = rec
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(rect) = cfg.webcam_overrides.get(&stem) {
        return Ok(resolved(Some(WebcamDetection {
            rect: *rect,
            confidence: 1.0,
            source: WebcamSource::Override,
        })));
    }
    if let Some(d) = store.find_webcam(fp)? {
        return Ok(Resolved {
            from_store: true,
            ..resolved(Some(d))
        });
    }

    let frames = sample_frames(&SystemRunner, rec, SAMPLE_COUNT)
        .context("falha ao amostrar quadros da gravação")?;
    if frames.is_empty() {
        return Ok(Resolved {
            reason: Some("não foi possível ler quadros do vídeo".into()),
            ..resolved(None)
        });
    }
    let mut detector = FaceDetector::new(&cfg.model_path)
        .with_context(|| format!("modelo de rosto em {}", cfg.model_path.display()))?;
    let mut faces = Vec::with_capacity(frames.len());
    for f in &frames {
        faces.push(detector.detect(f)?);
    }

    let reason = match detect_webcam(&frames, &faces) {
        WebcamOutcome::Found { detection, kind } if detection.confidence >= MIN_CONFIDENCE => {
            store.save_webcam(fp, &detection, rec.width, rec.height)?;
            return Ok(Resolved {
                kind: Some(kind),
                ..resolved(Some(detection))
            });
        }
        WebcamOutcome::Found { detection, .. } => Some(format!(
            "confiança baixa ({:.2} < {MIN_CONFIDENCE})",
            detection.confidence
        )),
        WebcamOutcome::NotFound { reason: r } => Some(r),
    };

    if let Some(rect) = store.last_webcam_for_resolution(rec.width, rec.height)? {
        let with_face = faces
            .iter()
            .filter(|fs| {
                fs.iter().any(|f| {
                    rect.contains_point(
                        f64::from(f.rect.x) + f64::from(f.rect.w) / 2.0,
                        f64::from(f.rect.y) + f64::from(f.rect.h) / 2.0,
                    )
                })
            })
            .count();
        if with_face as f64 >= SAVED_MIN_FACE_FRACTION * faces.len() as f64 {
            let d = WebcamDetection {
                rect,
                confidence: 0.5,
                source: WebcamSource::Saved,
            };
            store.save_webcam(fp, &d, rec.width, rec.height)?;
            return Ok(resolved(Some(d)));
        }
    }
    Ok(Resolved {
        reason,
        ..resolved(None)
    })
}

pub(crate) fn describe(r: &Resolved) -> String {
    let Some(d) = &r.detection else {
        return match &r.reason {
            Some(why) => format!("Webcam: não encontrada, corte será só do jogo\nMotivo: {why}"),
            None => "Webcam: não encontrada, corte será só do jogo".to_string(),
        };
    };
    let origin = match d.source {
        WebcamSource::Override => "manual, da configuração",
        WebcamSource::Saved => "reaproveitada de gravação parecida",
        WebcamSource::Detected if r.from_store => "já salva",
        WebcamSource::Detected => match r.kind {
            Some(WebcamKind::Bordered) => "detectada, com moldura",
            Some(WebcamKind::Cutout) => "detectada, recorte da pessoa",
            None => "detectada",
        },
    };
    format!(
        "Webcam: x={} y={} w={} h={} | confiança {:.2} | {origin}",
        d.rect.x, d.rect.y, d.rect.w, d.rect.h, d.confidence
    )
}

/// `target` é um arquivo ou a palavra `latest`.
pub fn webcam_command(cfg: &Config, target: &str, out: &mut dyn Write) -> anyhow::Result<()> {
    let path = if target == "latest" {
        latest_recording(
            &cfg.obs_dir,
            std::time::SystemTime::now(),
            std::time::Duration::from_secs(10),
        )?
        .with_context(|| format!("nenhuma gravação encontrada em {}", cfg.obs_dir.display()))?
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
    let r = resolve_webcam(cfg, &rec, &fp, &store)?;
    writeln!(out, "Gravação: {}", path.display())?;
    writeln!(out, "{}", describe(&r))?;
    Ok(())
}
