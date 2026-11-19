use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub obs_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub db_path: PathBuf,
}

#[derive(Deserialize, Default)]
struct RawConfig {
    obs_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    db_path: Option<PathBuf>,
}

impl Config {
    /// `CORTES_CONFIG` ou `%APPDATA%\content-machine\config.toml`; arquivo ausente = padrões.
    pub fn load() -> anyhow::Result<Config> {
        let path = match std::env::var_os("CORTES_CONFIG") {
            Some(p) => PathBuf::from(p),
            None => match std::env::var_os("APPDATA") {
                Some(a) => PathBuf::from(a).join("content-machine").join("config.toml"),
                None => return Config::load_from(Path::new("")),
            },
        };
        Config::load_from(&path)
    }

    pub fn load_from(path: &Path) -> anyhow::Result<Config> {
        let raw = match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str::<RawConfig>(&text)
                .with_context(|| format!("configuração inválida em {}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => RawConfig::default(),
            Err(e) => {
                return Err(e).with_context(|| format!("não foi possível ler {}", path.display()));
            }
        };
        let cache_dir = raw
            .cache_dir
            .unwrap_or_else(|| PathBuf::from(r"E:\.VIDEOS GRAVADOS\content-machine-cache"));
        Ok(Config {
            obs_dir: raw
                .obs_dir
                .unwrap_or_else(|| PathBuf::from(r"E:\.VIDEOS GRAVADOS\OBS")),
            db_path: raw.db_path.unwrap_or_else(|| cache_dir.join("cortes.db")),
            cache_dir,
        })
    }
}
