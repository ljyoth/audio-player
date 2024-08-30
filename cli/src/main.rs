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

    // let play = Arc::new((Mutex::new(true), Condvar::new()));
    // let play_clone = play.clone();
    // std::thread::spawn(move || {
    //     std::thread::sleep(Duration::from_millis(1000));
    //     let mut playing = play_clone.0.lock().unwrap();
    //     *playing = false;
    //     play_clone.1.notify_all();
    //     drop(playing);
    //     println!("{:?}", std::time::Instant::now());
    //     std::thread::sleep(Duration::from_millis(1000));
    //     let mut playing = play_clone.0.lock().unwrap();
    //     *playing = true;
    //     play_clone.1.notify_all();
    //     drop(playing);
    //     println!("{:?}", std::time::Instant::now());
    // });

    let mut player = AudioPlayer::new()?;
    player.play()?;
    player.open(args.file)?;
    player.seek(Duration::from_secs(60))?;

    println!("{:?}", std::time::Instant::now());

    Ok(())
}
