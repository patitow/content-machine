#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Gate de aceite com as gravações reais. Rodar com:
//! `cargo test -p cm-vision --test webcam_real -- --ignored --nocapture`

use std::path::{Path, PathBuf};

use cm_domain::Rect;
use cm_media::{SystemRunner, probe, sample_frames};
use cm_vision::{FaceDetector, WebcamKind, WebcamOutcome, detect_webcam};

fn contained_fraction(inner: &Rect, outer: &Rect) -> f64 {
    let ix = inner
        .right()
        .min(outer.right())
        .saturating_sub(inner.x.max(outer.x));
    let iy = inner
        .bottom()
        .min(outer.bottom())
        .saturating_sub(inner.y.max(outer.y));
    f64::from(ix) * f64::from(iy) / inner.area() as f64
}

#[test]
#[ignore = "usa as gravações reais em E:"]
fn real_recordings_meet_the_acceptance_criteria() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let gt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("tests/fixtures/webcam_gt.json")).unwrap())
            .unwrap();
    let model: PathBuf = root.join("../../models/face_detection_yunet_2023mar.onnx");
    let mut detector = FaceDetector::new(&model).unwrap();
    let mut failures = Vec::new();
    for (name, g) in gt.as_object().unwrap() {
        let get = |k: &str| g[k].as_u64().unwrap() as u32;
        let truth = Rect {
            x: get("x"),
            y: get("y"),
            w: get("w"),
            h: get("h"),
        };
        let bordered = g["kind"] == "bordered";
        let path = PathBuf::from(format!(r"E:\.VIDEOS GRAVADOS\OBS\{name}.mkv"));
        let rec = probe(&SystemRunner, &path).unwrap();
        let frames = sample_frames(&SystemRunner, &rec, 16).unwrap();
        let faces: Vec<_> = frames.iter().map(|f| detector.detect(f).unwrap()).collect();
        match detect_webcam(&frames, &faces) {
            WebcamOutcome::Found { detection, kind } => {
                let iou = detection.rect.iou(&truth);
                let contain = contained_fraction(&truth, &detection.rect);
                println!(
                    "{name}: {:?} {:?} conf={:.2} iou={iou:.3} contenção={contain:.3}",
                    kind, detection.rect, detection.confidence
                );
                let ok = if bordered {
                    kind == WebcamKind::Bordered && iou >= 0.9
                } else {
                    iou >= 0.75 && contain >= 0.95
                };
                if !ok {
                    failures.push(name.clone());
                }
            }
            WebcamOutcome::NotFound { reason } => {
                println!("{name}: NotFound ({reason})");
                failures.push(name.clone());
            }
        }
    }
    assert!(failures.is_empty(), "reprovadas: {failures:?}");
}
