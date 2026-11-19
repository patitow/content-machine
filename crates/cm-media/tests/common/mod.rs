#![allow(dead_code)]
use std::path::Path;
use std::process::Command;

pub fn ffmpeg(args: &[&str]) {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-y"])
        .args(args)
        .output()
        .expect("ffmpeg precisa estar no PATH");
    assert!(
        out.status.success(),
        "ffmpeg falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Vídeo sintético 320x180@30 com uma faixa de áudio por título e quadro-chave a cada 2 s.
pub fn video_with_audio_tracks(path: &Path, titles: &[&str], secs: u32) {
    let mut args: Vec<String> = vec![
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        format!("testsrc=d={secs}:s=320x180:r=30"),
    ];
    for _ in titles {
        args.extend(["-f".into(), "lavfi".into(), "-i".into()]);
        args.push(format!("sine=f=440:d={secs}"));
    }
    for i in 0..=titles.len() {
        args.extend(["-map".into(), i.to_string()]);
    }
    for (i, title) in titles.iter().enumerate() {
        args.extend([format!("-metadata:s:a:{i}"), format!("title={title}")]);
    }
    args.extend(
        [
            "-c:v",
            "libx264",
            "-force_key_frames",
            "expr:gte(t,n_forced*2)",
            "-c:a",
            "aac",
        ]
        .map(String::from),
    );
    args.push(path.to_string_lossy().into_owned());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    ffmpeg(&refs);
}
