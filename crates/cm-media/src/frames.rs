use std::ffi::OsString;

use cm_domain::Recording;

use crate::runner::{CommandRunner, MediaError};

/// Quadro em resolução cheia, RGB24 linha a linha.
#[derive(Debug, Clone, PartialEq)]
pub struct RgbFrame {
    pub width: u32,
    pub height: u32,
    pub time_s: f64,
    pub data: Vec<u8>,
}

/// Amostra `count` quadros igualmente espaçados entre 5% e 95% da duração.
/// Uma chamada do ffmpeg por quadro (busca por quadro-chave, barata no HDD);
/// quadros que o ffmpeg não entregou inteiros são descartados.
pub fn sample_frames(
    runner: &dyn CommandRunner,
    rec: &Recording,
    count: usize,
) -> Result<Vec<RgbFrame>, MediaError> {
    let expected = rec.width as usize * rec.height as usize * 3;
    let mut frames = Vec::with_capacity(count);
    for i in 0..count {
        let fraction = if count == 1 {
            0.5
        } else {
            0.05 + 0.9 * i as f64 / (count - 1) as f64
        };
        let time_s = rec.duration_s * fraction;
        let mut args: Vec<OsString> = ["-v", "error", "-ss"].map(OsString::from).into();
        args.push(format!("{time_s:.3}").into());
        args.push("-i".into());
        args.push(rec.path.as_os_str().to_owned());
        args.extend(
            [
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "pipe:1",
            ]
            .map(OsString::from),
        );
        let out = runner.run("ffmpeg", &args)?;
        if out.stdout.len() == expected {
            frames.push(RgbFrame {
                width: rec.width,
                height: rec.height,
                time_s,
                data: out.stdout,
            });
        }
    }
    Ok(frames)
}
