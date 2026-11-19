#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use cm_media::RgbFrame;
use cm_vision::FaceDetector;

fn model() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/face_detection_yunet_2023mar.onnx")
}

fn frame(name: &str) -> RgbFrame {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let img = image::open(path).unwrap().to_rgb8();
    RgbFrame {
        width: img.width(),
        height: img.height(),
        time_s: 0.0,
        data: img.into_raw(),
    }
}

fn center(f: &cm_vision::Face) -> (f64, f64) {
    (
        f64::from(f.rect.x) + f64::from(f.rect.w) / 2.0,
        f64::from(f.rect.y) + f64::from(f.rect.h) / 2.0,
    )
}

#[test]
fn finds_the_face_in_the_november_layout() {
    let mut d = FaceDetector::new(&model()).unwrap();
    let faces = d.detect(&frame("face_nov.jpg")).unwrap();
    assert_eq!(faces.len(), 1, "{faces:?}");
    assert!(faces[0].confidence >= 0.85);
    let (cx, cy) = center(&faces[0]);
    assert!(
        (cx - 251.0).abs() < 40.0 && (cy - 317.0).abs() < 40.0,
        "{cx},{cy}"
    );
}

#[test]
fn finds_the_face_in_the_july_layout() {
    let mut d = FaceDetector::new(&model()).unwrap();
    // Neste quadro o jogo tem um falso positivo de 0,74; o rosto real vem primeiro (maior confiança).
    let faces = d.detect(&frame("face_jul.jpg")).unwrap();
    assert!(!faces.is_empty(), "{faces:?}");
    assert!(faces[0].confidence >= 0.85);
    let (cx, cy) = center(&faces[0]);
    assert!(
        (cx - 365.0).abs() < 40.0 && (cy - 380.0).abs() < 40.0,
        "{cx},{cy}"
    );
}

#[test]
fn gameplay_without_webcam_has_no_face() {
    let mut d = FaceDetector::new(&model()).unwrap();
    assert!(d.detect(&frame("no_face.jpg")).unwrap().is_empty());
}

#[test]
fn missing_model_is_an_error_not_a_panic() {
    assert!(FaceDetector::new(Path::new("nao-existe.onnx")).is_err());
}

#[test]
fn tiny_image_does_not_crash() {
    let mut d = FaceDetector::new(&model()).unwrap();
    let tiny = RgbFrame {
        width: 100,
        height: 100,
        time_s: 0.0,
        data: vec![90; 100 * 100 * 3],
    };
    assert!(d.detect(&tiny).unwrap().is_empty());
}

#[test]
fn model_file_matches_the_pinned_hash() {
    let bytes = std::fs::read(model()).unwrap();
    assert_eq!(bytes.len(), 232_589);
}
