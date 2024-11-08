#![windows_subsystem = "windows"]

mod app;
mod player;

use app::{AudioPlayerApplication, AudioPlayerFlags};
use clap::Parser;
use color_eyre::eyre::Result;
use iced::advanced::graphics::core::window;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct CliArgs {
    #[arg(index(1))]
    file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = CliArgs::parse();

    iced::application(
        AudioPlayerApplication::title,
        AudioPlayerApplication::update,
        AudioPlayerApplication::view,
    )
    .subscription(AudioPlayerApplication::subscription)
    .theme(AudioPlayerApplication::theme)
    .window(window::Settings {
        icon: Some(window::icon::from_rgba(
            include_bytes!("../assets/icon.rgba").to_vec(),
            256,
            256,
        )?),
        ..Default::default()
    })
    .run_with(|| {
        AudioPlayerApplication::new(AudioPlayerFlags {
            file_path: args.file,
        })
    })?;

    Ok(())
}
