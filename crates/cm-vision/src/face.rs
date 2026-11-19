use std::path::Path;

use cm_domain::Rect;
use cm_media::RgbFrame;
use ort::session::Session;
use ort::value::Tensor;

const INPUT: usize = 640;
const SCORE_THRESHOLD: f32 = 0.6;
const NMS_THRESHOLD: f32 = 0.3;

#[derive(Debug, thiserror::Error)]
pub enum VisionError {
    #[error("não foi possível carregar o modelo de rosto: {0}")]
    Model(String),
    #[error("falha ao detectar rostos: {0}")]
    Inference(String),
    #[error("quadro inválido: {0}")]
    Frame(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Face {
    pub rect: Rect,
    pub confidence: f64,
}

/// YuNet em CPU via ONNX Runtime. Entrada fixa 640x640 (letterbox no canto superior esquerdo).
pub struct FaceDetector {
    session: Session,
    _env: ort::environment::Environment,
}

#[derive(Clone, Copy)]
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

fn nms(mut dets: Vec<Det>) -> Vec<Det> {
    dets.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut keep: Vec<Det> = Vec::new();
    for d in dets {
        if keep.iter().all(|k| iou(k, &d) < NMS_THRESHOLD) {
            keep.push(d);
        }
    }
    keep
}

impl FaceDetector {
    pub fn new(model: &Path) -> Result<FaceDetector, VisionError> {
        let err = |e: ort::Error| VisionError::Model(e.to_string());
        let env = ort::init().build().map_err(err)?;
        let session = Session::builder(&env)
            .map_err(err)?
            .commit_from_file(model)
            .map_err(err)?;
        Ok(FaceDetector { session, _env: env })
    }

    pub fn detect(&mut self, frame: &RgbFrame) -> Result<Vec<Face>, VisionError> {
        let (w, h) = (frame.width as usize, frame.height as usize);
        if w == 0 || h == 0 || frame.data.len() != w * h * 3 {
            return Err(VisionError::Frame(format!(
                "{w}x{h} com {} bytes",
                frame.data.len()
            )));
        }
        let scale = INPUT as f32 / w.max(h) as f32;
        let img = image::RgbImage::from_raw(frame.width, frame.height, frame.data.clone())
            .ok_or_else(|| VisionError::Frame("buffer incompatível".into()))?;
        let (rw, rh) = (
            ((w as f32 * scale).round() as u32).max(1),
            ((h as f32 * scale).round() as u32).max(1),
        );
        let resized = image::imageops::resize(&img, rw, rh, image::imageops::FilterType::Triangle);
        let plane = INPUT * INPUT;
        let mut input = vec![0f32; 3 * plane];
        for (x, y, p) in resized.enumerate_pixels() {
            let i = y as usize * INPUT + x as usize;
            input[i] = f32::from(p[2]);
            input[plane + i] = f32::from(p[1]);
            input[2 * plane + i] = f32::from(p[0]);
        }
        let infer = |e: ort::Error| VisionError::Inference(e.to_string());
        let tensor = Tensor::from_array(([1usize, 3, INPUT, INPUT], input)).map_err(infer)?;
        let outputs = self
            .session
            .run(ort::inputs!["input" => tensor])
            .map_err(infer)?;

        let mut dets = Vec::new();
        for stride in [8usize, 16, 32] {
            let get = |name: String| -> Result<Vec<f32>, VisionError> {
                let (_, data) = outputs[name.as_str()]
                    .try_extract_tensor::<f32>()
                    .map_err(infer)?;
                Ok(data.to_vec())
            };
            let cls = get(format!("cls_{stride}"))?;
            let obj = get(format!("obj_{stride}"))?;
            let bbox = get(format!("bbox_{stride}"))?;
            let cols = INPUT / stride;
            let s = stride as f32;
            for i in 0..cols * cols {
                let score = (cls[i].clamp(0.0, 1.0) * obj[i].clamp(0.0, 1.0)).sqrt();
                if score < SCORE_THRESHOLD {
                    continue;
                }
                let (r, c) = ((i / cols) as f32, (i % cols) as f32);
                let (cx, cy) = ((c + bbox[i * 4]) * s, (r + bbox[i * 4 + 1]) * s);
                let (bw, bh) = (bbox[i * 4 + 2].exp() * s, bbox[i * 4 + 3].exp() * s);
                dets.push(Det {
                    x: (cx - bw / 2.0) / scale,
                    y: (cy - bh / 2.0) / scale,
                    w: bw / scale,
                    h: bh / scale,
                    score,
                });
            }
        }
        Ok(nms(dets)
            .into_iter()
            .filter_map(|d| {
                let x = d.x.max(0.0).min(w as f32);
                let y = d.y.max(0.0).min(h as f32);
                let right = (d.x + d.w).min(w as f32);
                let bottom = (d.y + d.h).min(h as f32);
                (right > x && bottom > y).then(|| Face {
                    rect: Rect {
                        x: x as u32,
                        y: y as u32,
                        w: (right - x) as u32,
                        h: (bottom - y) as u32,
                    },
                    confidence: f64::from(d.score),
                })
            })
            .collect())
    }
}
