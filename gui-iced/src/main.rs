mod app;
mod player;

use app::{MusicPlayerApplication, MusicPlayerFlags};
use clap::Parser;
use color_eyre::eyre::Result;
use iced::{Application, Settings};
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct CliArgs {
    #[arg(index(1))]
    file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = CliArgs::parse();

    MusicPlayerApplication::run(Settings {
        flags: MusicPlayerFlags {
            file_path: args.file,
        },
        ..Default::default()
    })?;

    Ok(())
}
