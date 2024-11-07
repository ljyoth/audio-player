use audio_player::AudioPlayer;
use clap::{ArgAction, Parser};
use color_eyre::eyre::{eyre, Result};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::{
    io::{stdout, Write},
    path::PathBuf,
    time::Duration,
};

#[derive(Debug, Parser)]
struct CliArgs {
    file: PathBuf,
    #[arg(short, long, default_value_t = true, action=ArgAction::SetFalse)]
    progress_bar: bool,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = CliArgs::parse();

    let mut player = AudioPlayer::new();
    let controller = player.controller().clone();
    let track = player.open(&args.file)?;
    let details = track.details().clone();
    player.queue(track)?;

    println!("File: {}", args.file.to_string_lossy());
    if let Some(title) = details.title() {
        println!("Title: {}", title);
    }
    if let Some(artist) = details.artist() {
        println!("Title: {}", artist);
    }
    const FPS: u64 = 15;
    let duration = details.duration().ok_or(eyre!("no duration"))?.as_millis();
    if args.progress_bar {
        let bar = ProgressBar::with_draw_target(
            Some(duration as u64),
            ProgressDrawTarget::stderr_with_hz(FPS as u8),
        );
        bar.set_style(ProgressStyle::with_template(&format!(
            "[{{msg:>12}}] {{wide_bar}} [{:02}:{:02}:{:.3}]",
            duration / 3600_000,
            (duration % 3600_000) / 60_000,
            (duration % 60_000) as f64 / 1000.0
        ))?);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(1000 / FPS));
            let position = controller.position().map(|d| d.as_millis()).unwrap_or(0);
            bar.set_position(position as u64);
            bar.set_message(format!(
                "{:02}:{:02}:{:.3}",
                position / 3600_000,
                (position % 3600_000) / 60_000,
                (position % 60_000) as f64 / 1000.0
            ));
        });
    } else {
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(1000 / FPS));
            let position = controller.position().map(|d| d.as_millis()).unwrap_or(0);
            print!("\x1b[2K\r");
            print!(
                "[{:02}:{:02}:{:.3} / {:02}:{:02}:{:.3}]",
                position / 3600_000,
                (position % 3600_000) / 60_000,
                (position % 60_000) as f64 / 1000.0,
                duration / 3600_000,
                (duration % 3600_000) / 60_000,
                (duration % 60_000) as f64 / 1000.0
            );
            stdout().flush().unwrap();
        });
    }

    player.controller().play();

    // let start = std::time::Instant::now();
    // println!("start {:?} {:?}", start, player.controller().position());

    // std::thread::sleep(Duration::from_millis(2000));
    // let pause = std::time::Instant::now();
    // println!("pause {:?} {:?}", pause, player.controller().position());
    // player.controller().pause();
    // println!(
    //     "paused {:?} {:?}",
    //     std::time::Instant::now() - pause,
    //     player.controller().position()
    // );
    // std::thread::sleep(Duration::from_millis(1000));
    // let play = std::time::Instant::now();
    // println!(
    //     "play {:?} {:?}",
    //     play - pause,
    //     player.controller().position()
    // );
    // player.controller().play();
    // println!(
    //     "played {:?} {:?}",
    //     std::time::Instant::now() - play,
    //     player.controller().position()
    // );

    // std::thread::sleep(Duration::from_millis(2000));
    // player.controller().seek(Duration::from_secs(255));
    // println!("seeked {:?}", player.controller().position());

    player.wait_until_end();

    Ok(())
}
