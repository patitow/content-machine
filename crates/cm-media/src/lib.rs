//! Acesso a mídia: FFprobe/FFmpeg como processos externos atrás de um trait.

mod fingerprint;
mod frames;
mod ingest;
mod probe;
mod render;
mod runner;

pub use fingerprint::fingerprint;
pub use frames::{RgbFrame, sample_frames};
pub use ingest::{INGEST_VERSION, IngestOutput, ingest, ingest_with_version};
pub use probe::probe;
pub use render::{Layout9x16, plan_layout, render_9x16};
pub use runner::{CommandOutput, CommandRunner, MediaError, SystemRunner};
