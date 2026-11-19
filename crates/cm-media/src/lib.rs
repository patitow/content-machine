//! Acesso a mídia: FFprobe/FFmpeg como processos externos atrás de um trait.

mod probe;
mod runner;

pub use probe::probe;
pub use runner::{CommandOutput, CommandRunner, MediaError, SystemRunner};
