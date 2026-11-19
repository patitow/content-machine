#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::cell::RefCell;
use std::ffi::OsString;
use std::path::PathBuf;

use cm_domain::{Recording, Rect};
use cm_media::{
    CommandOutput, CommandRunner, MediaError, SystemRunner, plan_layout, probe, render_9x16,
};

fn r(x: u32, y: u32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

#[test]
fn november_layout_stacks_webcam_over_a_centered_game_crop() {
    let l = plan_layout(1920, 1080, Some(r(0, 438, 408, 284)));
    assert_eq!(l.webcam_dst_h, 752);
    assert_eq!(l.game_dst_h, 1168);
    assert_eq!(l.webcam_dst_h + l.game_dst_h, 1920);
    assert_eq!(l.webcam_src, Some(r(0, 438, 408, 284)));
    assert!(!l.game_src.intersects(&r(0, 438, 408, 284)));
    assert_eq!(l.game_src.h, 1080);
    assert!(l.game_src.right() <= 1920);
}

#[test]
fn cutout_layout_shifts_the_game_crop_away_from_the_person() {
    let cam = r(160, 780, 385, 300);
    let l = plan_layout(1920, 1080, Some(cam));
    assert!(!l.game_src.intersects(&cam), "{:?}", l.game_src);
    assert!(l.game_src.right() <= 1920);
}

#[test]
fn central_webcam_never_intersects_the_game_crop() {
    let cam = r(760, 400, 400, 300);
    let l = plan_layout(1920, 1080, Some(cam));
    assert!(!l.game_src.intersects(&cam), "{:?}", l.game_src);
    assert!(l.game_src.w > 0 && l.game_src.h > 0);
    assert!(l.game_src.right() <= 1920 && l.game_src.bottom() <= 1080);
}

#[test]
fn square_webcam_is_capped_at_864_and_cropped_vertically() {
    let cam = r(40, 300, 400, 400);
    let l = plan_layout(1920, 1080, Some(cam));
    assert_eq!(l.webcam_dst_h, 864);
    let src = l.webcam_src.unwrap();
    assert_eq!(src.w, 400);
    assert!(src.h < 400);
    assert!(src.y >= cam.y && src.bottom() <= cam.bottom());
    assert_eq!(l.game_dst_h, 1056);
}

#[test]
fn without_webcam_the_game_fills_the_whole_vertical_frame() {
    let l = plan_layout(1920, 1080, None);
    assert_eq!(l.webcam_src, None);
    assert_eq!(l.webcam_dst_h, 0);
    assert_eq!(l.game_dst_h, 1920);
    assert_eq!(l.game_src.h, 1080);
    assert_eq!(l.game_src.w, 608);
    assert_eq!(l.game_src.x, 656);
}

#[test]
fn layout_is_the_same_in_ratio_at_other_resolutions() {
    let a = plan_layout(1920, 1080, Some(r(0, 438, 408, 284)));
    let b = plan_layout(1280, 720, Some(r(0, 292, 272, 189)));
    assert!(a.game_dst_h.abs_diff(b.game_dst_h) <= 4);
    let c = plan_layout(2560, 1440, Some(r(0, 584, 544, 379)));
    assert!(a.game_dst_h.abs_diff(c.game_dst_h) <= 4);
}

struct FailingNvenc {
    calls: RefCell<Vec<Vec<OsString>>>,
}

impl CommandRunner for FailingNvenc {
    fn run(&self, _p: &str, args: &[OsString]) -> Result<CommandOutput, MediaError> {
        self.calls.borrow_mut().push(args.to_vec());
        if args.iter().any(|a| a == "h264_nvenc") {
            return Err(MediaError::NonZeroExit {
                program: "ffmpeg".into(),
                code: 1,
                stderr: "OpenEncodeSessionEx failed".into(),
            });
        }
        Ok(CommandOutput {
            stdout: vec![],
            stderr: String::new(),
        })
    }
}

#[test]
fn falls_back_to_x264_when_nvenc_is_unavailable() {
    let rec = Recording {
        path: PathBuf::from("a.mkv"),
        duration_s: 20.0,
        width: 1920,
        height: 1080,
        fps: 60.0,
        audio_tracks: vec![],
    };
    let runner = FailingNvenc {
        calls: RefCell::new(vec![]),
    };
    let layout = plan_layout(1920, 1080, None);
    render_9x16(&runner, &rec, &layout, 1.0, 5.0, &PathBuf::from("out.mp4")).unwrap();
    let calls = runner.calls.borrow();
    assert_eq!(calls.len(), 2);
    assert!(calls[1].iter().any(|a| a == "libx264"));
    // Taxa limitada: cortes para postar não podem passar de ~14 Mbps.
    assert!(calls[0].iter().any(|a| a == "-maxrate"));
    assert!(calls[1].iter().any(|a| a == "-maxrate"));
}

fn red_on_green_source(dir: &std::path::Path, with_audio: bool) -> PathBuf {
    let path = dir.join("fonte teste.mkv");
    let mut args = vec![
        "-f",
        "lavfi",
        "-i",
        "color=c=0x00b400:s=1920x1080:r=30:d=3,drawbox=x=0:y=438:w=408:h=284:color=0xdc0000:t=fill",
    ];
    if with_audio {
        args.extend(["-f", "lavfi", "-i", "sine=f=440:d=3", "-c:a", "aac"]);
    }
    args.extend(["-c:v", "libx264", "-pix_fmt", "yuv420p"]);
    let p = path.to_string_lossy().into_owned();
    args.push(&p);
    common::ffmpeg(&args);
    path
}

fn mean_color(video: &std::path::Path, crop: &str) -> (u8, u8, u8) {
    let out = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-ss", "1", "-i"])
        .arg(video)
        .args([
            "-frames:v",
            "1",
            "-vf",
            &format!("{crop},scale=1:1"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .output()
        .unwrap();
    (out.stdout[0], out.stdout[1], out.stdout[2])
}

#[test]
fn real_render_has_webcam_on_top_and_game_below() {
    let d = tempfile::tempdir().unwrap();
    let src = red_on_green_source(d.path(), true);
    let rec = probe(&SystemRunner, &src).unwrap();
    let layout = plan_layout(rec.width, rec.height, Some(r(0, 438, 408, 284)));
    let out = d.path().join("saída acentuada.mp4");
    render_9x16(&SystemRunner, &rec, &layout, 0.0, 3.0, &out).unwrap();

    let produced = probe(&SystemRunner, &out).unwrap();
    assert_eq!((produced.width, produced.height), (1080, 1920));
    assert!(produced.duration_s > 2.5 && produced.duration_s < 3.5);
    assert_eq!(produced.audio_tracks.len(), 1);

    let top = mean_color(&out, "crop=1080:500:0:0");
    assert!(
        top.0 > 180 && top.1 < 80,
        "topo {top:?} deveria ser vermelho"
    );
    let bottom = mean_color(&out, "crop=1080:500:0:1300");
    assert!(
        bottom.1 > 120 && bottom.0 < 60,
        "base {bottom:?} deveria ser verde, sem vermelho da webcam"
    );
}

#[test]
fn real_render_without_audio_and_without_webcam() {
    let d = tempfile::tempdir().unwrap();
    let src = red_on_green_source(d.path(), false);
    let rec = probe(&SystemRunner, &src).unwrap();
    let layout = plan_layout(rec.width, rec.height, None);
    let out = d.path().join("solo.mp4");
    render_9x16(&SystemRunner, &rec, &layout, 0.0, 2.0, &out).unwrap();
    let produced = probe(&SystemRunner, &out).unwrap();
    assert_eq!((produced.width, produced.height), (1080, 1920));
    assert!(produced.audio_tracks.is_empty());
}
