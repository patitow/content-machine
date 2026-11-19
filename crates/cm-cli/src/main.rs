use std::io::stdout;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cm_cli::{Config, ingest_command, latest_command};

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
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let cfg = Config::load()?;
    let mut out = stdout().lock();
    match cli.command {
        Command::Latest => latest_command(&cfg, &mut out),
        Command::Ingest { target } => ingest_command(&cfg, &target, &mut out),
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
