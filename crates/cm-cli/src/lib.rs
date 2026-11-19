//! Biblioteca do CLI `cortes`: configuração, descoberta da última gravação e comandos.

mod commands;
mod config;
mod latest;
mod render_cmd;
mod webcam_cmd;

pub use commands::{ingest_command, latest_command};
pub use config::Config;
pub use latest::latest_recording;
pub use render_cmd::{RenderSampleArgs, render_sample_command};
pub use webcam_cmd::{Resolved, resolve_webcam, webcam_command};
