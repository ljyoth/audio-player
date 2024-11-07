#![windows_subsystem = "windows"]

mod app;
mod player;

use app::{MusicPlayerApplication, MusicPlayerFlags};
use clap::Parser;
use color_eyre::eyre::Result;
use iced::{advanced::graphics::core::window, Application, Settings};
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
        window: window::Settings {
            icon: Some(window::icon::from_rgba(
                include_bytes!("../assets/icon.rgba").to_vec(),
                256,
                256,
            )?),
            ..Default::default()
        },
        ..Default::default()
    })?;

    Ok(())
}
