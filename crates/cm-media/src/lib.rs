//! Acesso a mídia: FFprobe/FFmpeg como processos externos atrás de um trait.

mod fingerprint;
mod probe;
mod runner;

pub use fingerprint::fingerprint;
pub use probe::probe;
pub use runner::{CommandOutput, CommandRunner, MediaError, SystemRunner};
