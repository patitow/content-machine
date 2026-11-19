//! Spike S1: YuNet (ONNX) via `ort` em quadros reais de gameplay.
//! Uso: s1-ort-yunet <modelo.onnx> <cpu|directml|cuda> <modo: letterbox|full> <imagem>...

use std::time::Instant;

use anyhow::{Context, Result, bail};
use ort::session::Session;
use ort::value::Tensor;

#[derive(Debug, Clone, Copy)]
struct Det {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    score: f32,
}

fn iou(a: &Det, b: &Det) -> f32 {
    let (x1, y1) = (a.x.max(b.x), a.y.max(b.y));
    let (x2, y2) = ((a.x + a.w).min(b.x + b.w), (a.y + a.h).min(b.y + b.h));
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    inter / (a.w * a.h + b.w * b.h - inter + 1e-9)
}

fn nms(mut dets: Vec<Det>, thr: f32) -> Vec<Det> {
    dets.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut keep: Vec<Det> = Vec::new();
    for d in dets {
        if keep.iter().all(|k| iou(k, &d) < thr) {
            keep.push(d);
        }
    }
    keep
}

/// Prepara a entrada: BGR, float 0..255, NCHW, imagem no canto superior esquerdo.
/// `letterbox` reduz para caber em 640x640; `full` usa o tamanho original arredondado para múltiplo de 32.
fn prepare(img: &image::RgbImage, mode: &str) -> (Vec<f32>, usize, usize, f32) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let (scale, tw, th) = if mode == "full" {
        (1.0f32, w.div_ceil(32) * 32, h.div_ceil(32) * 32)
    } else {
        (640.0 / w.max(h) as f32, 640, 640)
    };
    let resized = if (scale - 1.0).abs() > f32::EPSILON {
        image::imageops::resize(
            img,
            (w as f32 * scale) as u32,
            (h as f32 * scale) as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        img.clone()
    };
    let mut data = vec![0f32; 3 * tw * th];
    for (x, y, p) in resized.enumerate_pixels() {
        let (x, y) = (x as usize, y as usize);
        data[y * tw + x] = p[2] as f32;
        data[tw * th + y * tw + x] = p[1] as f32;
        data[2 * tw * th + y * tw + x] = p[0] as f32;
    }
    (data, tw, th, scale)
}

fn decode(outputs: &ort::session::SessionOutputs, tw: usize, th: usize, scale: f32) -> Result<Vec<Det>> {
    let mut dets = Vec::new();
    for stride in [8usize, 16, 32] {
        let (_, cls) = outputs[format!("cls_{stride}").as_str()].try_extract_tensor::<f32>()?;
        let (_, obj) = outputs[format!("obj_{stride}").as_str()].try_extract_tensor::<f32>()?;
        let (_, bbox) = outputs[format!("bbox_{stride}").as_str()].try_extract_tensor::<f32>()?;
        let (cols, rows) = (tw / stride, th / stride);
        for r in 0..rows {
            for c in 0..cols {
                let i = r * cols + c;
                let score = (cls[i].clamp(0.0, 1.0) * obj[i].clamp(0.0, 1.0)).sqrt();
                if score < 0.3 {
                    continue;
                }
                let s = stride as f32;
                let cx = (c as f32 + bbox[i * 4]) * s;
                let cy = (r as f32 + bbox[i * 4 + 1]) * s;
                let w = bbox[i * 4 + 2].exp() * s;
                let h = bbox[i * 4 + 3].exp() * s;
                dets.push(Det {
                    x: (cx - w / 2.0) / scale,
                    y: (cy - h / 2.0) / scale,
                    w: w / scale,
                    h: h / scale,
                    score,
                });
            }
        }
    }
    Ok(nms(dets, 0.3))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        bail!("uso: s1-ort-yunet <modelo.onnx> <cpu|directml|cuda> <letterbox|full> <imagem>...");
    }
    let (model, provider, mode) = (&args[1], args[2].as_str(), args[3].as_str());

    let env = ort::init().build().map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut builder = Session::builder(&env).map_err(|e| anyhow::anyhow!("{e}"))?;
    match provider {
        "cpu" => {}
        "directml" => {
            builder = builder
                .with_execution_providers([ort::ep::DirectML::default().build()])
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        "cuda" => bail!("CUDA não habilitado neste spike (exige cuDNN)"),
        other => bail!("provedor desconhecido: {other}"),
    }
    let mut session = builder.commit_from_file(model).context("carregando o modelo")?;
    println!("entradas do modelo: {:?}", session.inputs());

    for path in &args[4..] {
        let img = image::open(path)?.to_rgb8();
        let (data, tw, th, scale) = prepare(&img, mode);
        let make = || Tensor::from_array(([1usize, 3, th, tw], data.clone()));

        for _ in 0..2 {
            session.run(ort::inputs!["input" => make()?])?;
        }
        let n = 10;
        let t0 = Instant::now();
        let mut dets = Vec::new();
        for _ in 0..n {
            let outputs = session.run(ort::inputs!["input" => make()?])?;
            dets = decode(&outputs, tw, th, scale)?;
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("{path} [{provider}/{mode}] {ms:.1} ms/quadro (incl. cópia da entrada)");
        for d in dets.iter().take(3) {
            println!(
                "  rosto x={:.0} y={:.0} w={:.0} h={:.0} confiança={:.2}",
                d.x, d.y, d.w, d.h, d.score
            );
        }
        if dets.is_empty() {
            println!("  nenhum rosto detectado");
        }
    }
    Ok(())
}
