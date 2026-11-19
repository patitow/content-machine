use std::ffi::OsString;
use std::path::Path;

use cm_domain::{Recording, Rect};

use crate::runner::{CommandRunner, MediaError};

const OUT_W: u32 = 1080;
const OUT_H: u32 = 1920;
/// A webcam ocupa no máximo 45% da altura (864 px).
const MAX_WEBCAM_H: u32 = 864;

/// Composição vertical: faixa da webcam em cima e recorte do jogo embaixo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout9x16 {
    /// Região da fonte que vira a faixa de cima (`None` = sem webcam).
    pub webcam_src: Option<Rect>,
    pub webcam_dst_h: u32,
    /// Região da fonte que vira a faixa de baixo; nunca intersecta a webcam.
    pub game_src: Rect,
    pub game_dst_h: u32,
}

fn even(v: u32) -> u32 {
    v & !1
}

/// Recorte de proporção `OUT_W : dst_h` com a maior altura possível, centrado.
fn centered_crop(src_w: u32, src_h: u32, dst_h: u32) -> Rect {
    let mut w = (f64::from(src_h) * f64::from(OUT_W) / f64::from(dst_h)).round() as u32;
    let mut h = src_h;
    if w > src_w {
        w = src_w;
        h = (f64::from(src_w) * f64::from(dst_h) / f64::from(OUT_W)).round() as u32;
    }
    Rect {
        x: (src_w - w) / 2,
        y: (src_h - h) / 2,
        w,
        h,
    }
}

/// Recorte de largura `crop_w` começando em `x` (altura e posição vertical herdadas de `base`).
fn with_x(base: &Rect, x: u32) -> Rect {
    Rect { x, ..*base }
}

/// Ajusta o recorte do jogo para nunca cobrir a webcam: desloca para o lado livre
/// mais próximo; se nenhum comporta, estreita para o maior lado livre.
fn avoid(crop: Rect, cam: &Rect, src_w: u32, src_h: u32, dst_h: u32) -> Rect {
    if !crop.intersects(cam) {
        return crop;
    }
    let right = (cam.right() + crop.w <= src_w).then(|| with_x(&crop, cam.right()));
    let left = (cam.x >= crop.w).then(|| with_x(&crop, cam.x - crop.w));
    let shift = |c: &Rect| c.x.abs_diff(crop.x);
    match (left, right) {
        (Some(l), Some(r)) => {
            if shift(&l) <= shift(&r) {
                l
            } else {
                r
            }
        }
        (Some(c), None) | (None, Some(c)) => c,
        (None, None) => {
            let free_left = cam.x;
            let free_right = src_w - cam.right().min(src_w);
            let (x, w) = if free_left >= free_right {
                (0, free_left)
            } else {
                (cam.right(), free_right)
            };
            let w = w.max(2);
            let h =
                ((f64::from(w) * f64::from(dst_h) / f64::from(OUT_W)).round() as u32).min(src_h);
            Rect {
                x,
                y: (src_h - h) / 2,
                w,
                h,
            }
        }
    }
}

pub fn plan_layout(src_w: u32, src_h: u32, webcam: Option<Rect>) -> Layout9x16 {
    let Some(cam) = webcam else {
        return Layout9x16 {
            webcam_src: None,
            webcam_dst_h: 0,
            game_src: centered_crop(src_w, src_h, OUT_H),
            game_dst_h: OUT_H,
        };
    };
    let natural = (f64::from(OUT_W) * f64::from(cam.h) / f64::from(cam.w)).round() as u32;
    let (webcam_src, webcam_dst_h) = if natural > MAX_WEBCAM_H {
        // Corta o excesso vertical, mantendo mais da parte de cima (onde fica o rosto).
        let h = (f64::from(cam.w) * f64::from(MAX_WEBCAM_H) / f64::from(OUT_W)).round() as u32;
        let y = cam.y + ((cam.h - h) as f64 * 0.3) as u32;
        (Rect { y, h, ..cam }, MAX_WEBCAM_H)
    } else {
        (cam, even(natural))
    };
    let game_dst_h = OUT_H - webcam_dst_h;
    let game_src = avoid(
        centered_crop(src_w, src_h, game_dst_h),
        &cam,
        src_w,
        src_h,
        game_dst_h,
    );
    Layout9x16 {
        webcam_src: Some(webcam_src),
        webcam_dst_h,
        game_src,
        game_dst_h,
    }
}

fn crop(r: &Rect) -> String {
    format!("crop={}:{}:{}:{}", r.w, r.h, r.x, r.y)
}

fn filter_graph(layout: &Layout9x16) -> String {
    let game = format!(
        "{},scale={OUT_W}:{}:flags=lanczos",
        crop(&layout.game_src),
        layout.game_dst_h
    );
    match &layout.webcam_src {
        None => format!("[0:v]{game}[v]"),
        Some(cam) => format!(
            "[0:v]{},scale={OUT_W}:{}:flags=lanczos[cam];[0:v]{game}[game];[cam][game]vstack=inputs=2[v]",
            crop(cam),
            layout.webcam_dst_h
        ),
    }
}

fn args(
    rec: &Recording,
    layout: &Layout9x16,
    start_s: f64,
    dur_s: f64,
    out: &Path,
    video_codec: &[&str],
) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-v", "error", "-y", "-ss"].map(OsString::from).into();
    a.push(format!("{start_s:.3}").into());
    a.extend(["-t".into(), format!("{dur_s:.3}").into(), "-i".into()]);
    a.push(rec.path.as_os_str().to_owned());
    a.extend(["-filter_complex".into(), filter_graph(layout).into()]);
    a.extend(["-map", "[v]"].map(OsString::from));
    if !rec.audio_tracks.is_empty() {
        a.extend(["-map", "0:a:0"].map(OsString::from));
    }
    a.extend(video_codec.iter().map(OsString::from));
    a.extend(["-pix_fmt", "yuv420p"].map(OsString::from));
    if !rec.audio_tracks.is_empty() {
        a.extend(["-c:a", "aac", "-b:a", "192k"].map(OsString::from));
    }
    a.extend(["-movflags", "+faststart"].map(OsString::from));
    a.push(out.as_os_str().to_owned());
    a
}

/// Renderiza `dur_s` segundos a partir de `start_s` em 1080x1920. Usa NVENC; se o
/// encoder de hardware não abrir (por exemplo, sessões ocupadas pelo OBS), refaz em x264.
pub fn render_9x16(
    runner: &dyn CommandRunner,
    rec: &Recording,
    layout: &Layout9x16,
    start_s: f64,
    dur_s: f64,
    out: &Path,
) -> Result<(), MediaError> {
    let nvenc = [
        "-c:v",
        "h264_nvenc",
        "-preset",
        "p5",
        "-cq",
        "21",
        "-b:v",
        "0",
        "-maxrate",
        "14M",
        "-bufsize",
        "28M",
    ];
    match runner.run("ffmpeg", &args(rec, layout, start_s, dur_s, out, &nvenc)) {
        Ok(_) => Ok(()),
        Err(MediaError::NonZeroExit { .. }) => {
            let x264 = [
                "-c:v", "libx264", "-preset", "medium", "-crf", "20", "-maxrate", "14M",
                "-bufsize", "28M",
            ];
            runner
                .run("ffmpeg", &args(rec, layout, start_s, dur_s, out, &x264))
                .map(|_| ())
        }
        Err(e) => Err(e),
    }
}
