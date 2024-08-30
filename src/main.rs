mod app;
mod decoder;
mod output;
mod player;

use app::{MusicPlayerApplication, MusicPlayerFlags};
use clap::Parser;
use iced::{Application, Settings};
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct CliArgs {
    #[arg(index(1))]
    file: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    let mut output = output::AudioOutputter::new().unwrap();
    let mut track = decoder::decode(&args.file).unwrap();
    while let Ok(buffer) = track.next() {
        output.write(buffer);
    }

    // MusicPlayerApplication::run(Settings {
    //     flags: MusicPlayerFlags {
    //         file_path: args.file,
    //     },
    //     ..Default::default()
    // })?;

    Ok(())
}
