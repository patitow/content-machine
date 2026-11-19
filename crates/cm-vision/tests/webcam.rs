#![allow(clippy::unwrap_used, clippy::expect_used)]

use cm_domain::Rect;
use cm_media::RgbFrame;
use cm_vision::{Face, WebcamKind, WebcamOutcome, detect_webcam};

const W: u32 = 960;
const H: u32 = 540;

/// "Jogo": fundo suave que muda a cada quadro, com um pouco de ruído.
fn game_pixel(x: u32, y: u32, i: u32) -> u8 {
    let t = i as f32;
    let v = 110.0 + 70.0 * (0.05 * x as f32 + 0.9 * t).sin() * (0.04 * y as f32 + 1.3 * t).cos();
    let noise = ((x.wrapping_mul(31) ^ y.wrapping_mul(17) ^ i.wrapping_mul(131)) % 7) as f32;
    (v + noise).clamp(0.0, 255.0) as u8
}

struct Scene {
    cam: Rect,
    border: bool,
    scale: u32,
}

fn frame(scene: &Scene, i: u32) -> RgbFrame {
    let (w, h) = (W * scene.scale, H * scene.scale);
    let s = scene.scale;
    let cam = Rect {
        x: scene.cam.x * s,
        y: scene.cam.y * s,
        w: scene.cam.w * s,
        h: scene.cam.h * s,
    };
    let mut data = vec![0u8; (w * h * 3) as usize];
    for y in 0..h {
        for x in 0..w {
            let inside = x >= cam.x && x < cam.right() && y >= cam.y && y < cam.bottom();
            let v = if inside {
                if scene.border {
                    225
                } else {
                    game_pixel(x / s, y / s, i)
                }
            } else {
                game_pixel(x / s, y / s, i)
            };
            let o = ((y * w + x) * 3) as usize;
            data[o..o + 3].fill(v);
        }
    }
    RgbFrame {
        width: w,
        height: h,
        time_s: f64::from(i),
        data,
    }
}

fn face_in(scene: &Scene) -> Face {
    let c = scene.cam;
    let s = scene.scale;
    Face {
        rect: Rect {
            x: (c.x + c.w * 3 / 10) * s,
            y: (c.y + c.h * 22 / 100) * s,
            w: (c.w * 4 / 10) * s,
            h: (c.h * 5 / 10) * s,
        },
        confidence: 0.92,
    }
}

fn run(scene: &Scene, n: u32, with_face: impl Fn(u32) -> bool) -> WebcamOutcome {
    let frames: Vec<RgbFrame> = (0..n).map(|i| frame(scene, i)).collect();
    let faces: Vec<Vec<Face>> = (0..n)
        .map(|i| {
            if with_face(i) {
                vec![face_in(scene)]
            } else {
                vec![]
            }
        })
        .collect();
    detect_webcam(&frames, &faces)
}

#[test]
fn bordered_overlay_in_the_middle_of_the_frame() {
    let scene = Scene {
        cam: Rect {
            x: 40,
            y: 200,
            w: 204,
            h: 142,
        },
        border: true,
        scale: 1,
    };
    match run(&scene, 12, |_| true) {
        WebcamOutcome::Found { detection, kind } => {
            assert_eq!(kind, WebcamKind::Bordered);
            assert!(
                detection.rect.iou(&scene.cam) >= 0.95,
                "{:?}",
                detection.rect
            );
            assert!(detection.confidence >= 0.6, "{}", detection.confidence);
        }
        other => panic!("esperava Found, veio {other:?}"),
    }
}

#[test]
fn bordered_overlay_glued_to_the_bottom_left_corner() {
    let scene = Scene {
        cam: Rect {
            x: 0,
            y: 398,
            w: 204,
            h: 142,
        },
        border: true,
        scale: 1,
    };
    match run(&scene, 12, |_| true) {
        WebcamOutcome::Found { detection, .. } => {
            assert!(
                detection.rect.iou(&scene.cam) >= 0.9,
                "{:?}",
                detection.rect
            );
        }
        other => panic!("esperava Found, veio {other:?}"),
    }
}

#[test]
fn overlay_without_any_border_is_a_cutout_around_the_face() {
    let scene = Scene {
        cam: Rect {
            x: 80,
            y: 340,
            w: 200,
            h: 200,
        },
        border: false,
        scale: 1,
    };
    match run(&scene, 12, |_| true) {
        WebcamOutcome::Found { detection, kind } => {
            assert_eq!(kind, WebcamKind::Cutout);
            assert!(detection.confidence <= 0.7);
            let face = face_in(&scene).rect;
            assert!(detection.rect.x <= face.x && detection.rect.right() >= face.right());
            assert!(detection.rect.y <= face.y && detection.rect.bottom() >= face.bottom());
        }
        other => panic!("esperava Found, veio {other:?}"),
    }
}

#[test]
fn face_in_only_a_few_frames_is_not_found() {
    let scene = Scene {
        cam: Rect {
            x: 40,
            y: 200,
            w: 204,
            h: 142,
        },
        border: true,
        scale: 1,
    };
    assert!(matches!(
        run(&scene, 12, |i| i < 3),
        WebcamOutcome::NotFound { .. }
    ));
}

#[test]
fn face_filling_the_frame_is_a_talking_video_not_an_overlay() {
    let scene = Scene {
        cam: Rect {
            x: 0,
            y: 0,
            w: 960,
            h: 540,
        },
        border: false,
        scale: 1,
    };
    let frames: Vec<RgbFrame> = (0..12).map(|i| frame(&scene, i)).collect();
    let big = Face {
        rect: Rect {
            x: 300,
            y: 40,
            w: 360,
            h: 380,
        },
        confidence: 0.95,
    };
    let faces = vec![vec![big]; 12];
    match detect_webcam(&frames, &faces) {
        WebcamOutcome::NotFound { reason } => assert!(reason.contains("grande"), "{reason}"),
        other => panic!("esperava NotFound, veio {other:?}"),
    }
}

#[test]
fn no_faces_at_all_is_not_found() {
    let scene = Scene {
        cam: Rect {
            x: 40,
            y: 200,
            w: 204,
            h: 142,
        },
        border: true,
        scale: 1,
    };
    assert!(matches!(
        run(&scene, 12, |_| false),
        WebcamOutcome::NotFound { .. }
    ));
}

#[test]
fn same_scene_at_double_resolution_gives_the_same_rect_scaled() {
    let scene = Scene {
        cam: Rect {
            x: 40,
            y: 200,
            w: 204,
            h: 142,
        },
        border: true,
        scale: 2,
    };
    match run(&scene, 10, |_| true) {
        WebcamOutcome::Found { detection, .. } => {
            let expected = Rect {
                x: 80,
                y: 400,
                w: 408,
                h: 284,
            };
            assert!(detection.rect.iou(&expected) >= 0.9, "{:?}", detection.rect);
        }
        other => panic!("esperava Found, veio {other:?}"),
    }
}

/// Overlay colado na borda esquerda do quadro, com uma tira escura de 6 px por dentro
/// (como o suporte do microfone na gravação real): a borda do quadro é que vale.
#[test]
fn internal_edge_near_the_frame_border_snaps_to_the_frame_edge() {
    let scene = Scene {
        cam: Rect {
            x: 0,
            y: 200,
            w: 204,
            h: 142,
        },
        border: true,
        scale: 1,
    };
    let frames: Vec<RgbFrame> = (0..12)
        .map(|i| {
            let mut f = frame(&scene, i);
            for y in 200..342u32 {
                for x in 0..6u32 {
                    let o = ((y * W + x) * 3) as usize;
                    f.data[o..o + 3].fill(20);
                }
            }
            f
        })
        .collect();
    let faces = vec![vec![face_in(&scene)]; 12];
    match detect_webcam(&frames, &faces) {
        WebcamOutcome::Found { detection, .. } => {
            assert_eq!(detection.rect.x, 0, "{:?}", detection.rect);
            assert!(
                detection.rect.iou(&scene.cam) >= 0.94,
                "{:?}",
                detection.rect
            );
        }
        other => panic!("esperava Found, veio {other:?}"),
    }
}
