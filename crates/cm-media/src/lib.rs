//! Acesso a mídia: FFprobe/FFmpeg como processos externos atrás de um trait.

mod fingerprint;
mod frames;
mod ingest;
mod probe;
mod runner;

pub use fingerprint::fingerprint;
pub use frames::{RgbFrame, sample_frames};
pub use ingest::{INGEST_VERSION, IngestOutput, ingest, ingest_with_version};
pub use probe::probe;
pub use runner::{CommandOutput, CommandRunner, MediaError, SystemRunner};
