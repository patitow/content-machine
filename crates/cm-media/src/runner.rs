use std::ffi::OsString;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("não foi possível iniciar `{program}`: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },
    #[error("`{program}` terminou com código {code}: {stderr}")]
    NonZeroExit {
        program: String,
        code: i32,
        stderr: String,
    },
    #[error("saída inesperada do ffprobe: {0}")]
    Parse(String),
    #[error("o arquivo não tem faixa de vídeo")]
    NoVideoStream,
    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: String,
}

/// Executa programas externos. Falha de início ou código de saída ≠ 0 viram `Err`.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[OsString]) -> Result<CommandOutput, MediaError>;
}

pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[OsString]) -> Result<CommandOutput, MediaError> {
        let out = Command::new(program)
            .args(args)
            .output()
            .map_err(|source| MediaError::Spawn {
                program: program.to_string(),
                source,
            })?;
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if !out.status.success() {
            return Err(MediaError::NonZeroExit {
                program: program.to_string(),
                code: out.status.code().unwrap_or(-1),
                stderr,
            });
        }
        Ok(CommandOutput {
            stdout: out.stdout,
            stderr,
        })
    }
}
