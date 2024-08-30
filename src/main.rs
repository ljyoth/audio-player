mod app;
mod decoder;
mod output;
mod player;
mod resampler;

use app::{MusicPlayerApplication, MusicPlayerFlags};
use clap::Parser;
use iced::{Application, Settings};
use resampler::SymphoniaResampler;
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
    let mut resampler = None;
    while let Ok(buffer) = track.next() {
        if resampler.is_none() && buffer.spec().rate != *output.sample_rate() {
            resampler = Some(SymphoniaResampler::new(*output.sample_rate(), &buffer));
        }
        if let Some(ref mut resampler) = resampler {
            let buffer = resampler.resample(buffer);
            output.write(buffer);
        } else {
            println!("writing...");
            output.write(buffer);
        }
    }

    // MusicPlayerApplication::run(Settings {
    //     flags: MusicPlayerFlags {
    //         file_path: args.file,
    //     },
    //     ..Default::default()
    // })?;

    Ok(())
}
