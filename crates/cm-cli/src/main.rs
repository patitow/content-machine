use std::io::stdout;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cm_cli::{
    Config, RenderSampleArgs, ingest_command, latest_command, render_sample_command, webcam_command,
};

#[derive(Parser)]
#[command(
    name = "cortes",
    about = "Cortes automáticos a partir das gravações do OBS"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Mostra o caminho da gravação mais recente da pasta do OBS
    Latest,
    /// Analisa uma gravação e extrai áudio e proxy de vídeo (use `latest` para a mais recente)
    Ingest { target: String },
    /// Acha o retângulo da webcam na gravação (use `latest` para a mais recente)
    Webcam { target: String },
    /// Renderiza um trecho 9:16 de teste (webcam em cima, jogo embaixo)
    RenderSample {
        target: String,
        /// Início do trecho, em segundos
        #[arg(long, default_value_t = 60.0)]
        start: f64,
        /// Duração do trecho, em segundos
        #[arg(long, default_value_t = 10.0)]
        dur: f64,
        /// Arquivo de saída (padrão: pasta renders do cache)
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let cfg = Config::load()?;
    let mut out = stdout().lock();
    match cli.command {
        Command::Latest => latest_command(&cfg, &mut out),
        Command::Ingest { target } => ingest_command(&cfg, &target, &mut out),
        Command::Webcam { target } => webcam_command(&cfg, &target, &mut out),
        Command::RenderSample {
            target,
            start,
            dur,
            out: file,
        } => render_sample_command(
            &cfg,
            &RenderSampleArgs {
                target,
                start_s: start,
                dur_s: dur,
                out: file,
            },
            &mut out,
        ),
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Erro: {e:#}");
            ExitCode::FAILURE
        }
    }
}
