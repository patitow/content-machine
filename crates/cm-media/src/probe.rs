use std::ffi::OsString;
use std::path::Path;

use cm_domain::{AudioTrack, Recording, assign_roles};
use serde::Deserialize;

use crate::runner::{CommandRunner, MediaError};

#[derive(Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<Stream>,
    format: Format,
}

#[derive(Deserialize)]
struct Format {
    duration: Option<String>,
}

#[derive(Deserialize)]
struct Stream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    #[serde(default)]
    tags: Tags,
}

#[derive(Deserialize, Default)]
struct Tags {
    title: Option<String>,
}

fn parse_fraction(s: &str) -> Option<f64> {
    let (n, d) = s.split_once('/')?;
    let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
    (d != 0.0).then_some(n / d)
}

pub fn probe(runner: &dyn CommandRunner, path: &Path) -> Result<Recording, MediaError> {
    let args: Vec<OsString> = [
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
    ]
    .iter()
    .map(OsString::from)
    .chain([path.as_os_str().to_owned()])
    .collect();
    let out = runner.run("ffprobe", &args)?;
    let parsed: FfprobeOutput =
        serde_json::from_slice(&out.stdout).map_err(|e| MediaError::Parse(e.to_string()))?;

    let video = parsed
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"))
        .ok_or(MediaError::NoVideoStream)?;
    let titles: Vec<Option<String>> = parsed
        .streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("audio"))
        .map(|s| s.tags.title.clone())
        .collect();
    let roles = assign_roles(&titles);
    let audio_tracks = titles
        .into_iter()
        .zip(roles)
        .enumerate()
        .map(|(i, (title, role))| AudioTrack {
            index: i as u32,
            title,
            role,
        })
        .collect();

    Ok(Recording {
        path: path.to_path_buf(),
        duration_s: parsed
            .format
            .duration
            .as_deref()
            .and_then(|d| d.parse().ok())
            .unwrap_or(0.0),
        width: video.width.unwrap_or(0),
        height: video.height.unwrap_or(0),
        fps: video
            .r_frame_rate
            .as_deref()
            .and_then(parse_fraction)
            .unwrap_or(0.0),
        audio_tracks,
    })
}
