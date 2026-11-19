use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Gravação mais recente da pasta: só `.mkv`/`.mp4`, não vazia, parada há pelo menos `settle`
/// (ainda sendo gravada não conta), sem entrar em subpastas. Com o mesmo nome base em
/// `.mkv` e `.mp4`, vale o `.mkv`.
pub fn latest_recording(
    dir: &Path,
    now: SystemTime,
    settle: Duration,
) -> io::Result<Option<PathBuf>> {
    let mut by_stem: HashMap<String, (PathBuf, SystemTime, bool)> = HashMap::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let path = entry.path();
        if !meta.is_file() || meta.len() == 0 {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase);
        let is_mkv = match ext.as_deref() {
            Some("mkv") => true,
            Some("mp4") => false,
            _ => continue,
        };
        let mtime = meta.modified()?;
        match now.duration_since(mtime) {
            Ok(age) if age >= settle => {}
            _ => continue,
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let replace = match by_stem.get(&stem) {
            None => true,
            Some((_, _, existing_is_mkv)) => is_mkv && !existing_is_mkv,
        };
        if replace {
            by_stem.insert(stem, (path, mtime, is_mkv));
        }
    }
    Ok(by_stem
        .into_values()
        .max_by_key(|(_, mtime, _)| *mtime)
        .map(|(path, _, _)| path))
}
