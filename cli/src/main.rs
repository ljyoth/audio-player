use audio_player::AudioPlayer;
use clap::Parser;
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

    let mut player = AudioPlayer::new();
    println!("start {:?}", std::time::Instant::now());
    player.open(args.file)?;
    player.controller().play()?;

    std::thread::sleep(Duration::from_millis(2000));
    let pause = std::time::Instant::now();
    println!("pause {:?}", pause);
    player.controller().pause().unwrap();
    println!("paused {:?}", std::time::Instant::now() - pause);
    std::thread::sleep(Duration::from_millis(1000));
    let play = std::time::Instant::now();
    println!("play {:?}", play - pause);
    player.controller().play().unwrap();
    println!("played {:?}", std::time::Instant::now() - play);

    std::thread::sleep(Duration::from_millis(2000));
    player.controller().seek(Duration::from_secs(200)).unwrap();

    player.wait_until_end()?;

    Ok(())
}
