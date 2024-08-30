mod app;
mod player;

use app::{MusicPlayerApplication, MusicPlayerFlags};
use clap::Parser;
use core::time;
use iced::{Application, Settings};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};

#[derive(Debug, Parser)]
struct CliArgs {
    #[arg(index(1))]
    file: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    MusicPlayerApplication::run(Settings {
        flags: MusicPlayerFlags {
            file_path: args.file,
        },
        ..Default::default()
    })?;

    Ok(())
}
