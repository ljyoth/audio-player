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
    let controller = player.controller().clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1000));
        println!("{:?}", controller.position());
    });
    player.open(args.file)?;
    player.controller().play()?;
    let start = std::time::Instant::now();
    println!("start {:?} {:?}", start, player.controller().position());

    std::thread::sleep(Duration::from_millis(2000));
    let pause = std::time::Instant::now();
    println!("pause {:?} {:?}", pause, player.controller().position());
    player.controller().pause().unwrap();
    println!(
        "paused {:?} {:?}",
        std::time::Instant::now() - pause,
        player.controller().position()
    );
    std::thread::sleep(Duration::from_millis(1000));
    let play = std::time::Instant::now();
    println!(
        "play {:?} {:?}",
        play - pause,
        player.controller().position()
    );
    player.controller().play().unwrap();
    println!(
        "played {:?} {:?}",
        std::time::Instant::now() - play,
        player.controller().position()
    );

    std::thread::sleep(Duration::from_millis(2000));
    player.controller().seek(Duration::from_secs(255))?;
    println!("seeked {:?}", player.controller().position());

    player.wait_until_end()?;

    Ok(())
}
