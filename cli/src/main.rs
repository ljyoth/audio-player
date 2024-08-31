use audio_player::AudioPlayer;
use clap::{ArgAction, Parser};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::{io::{stdout, Write}, path::PathBuf, time::Duration};

#[derive(Debug, Parser)]
struct CliArgs {
    file: PathBuf,
    #[arg(short, long, default_value_t = true, action=ArgAction::SetFalse)]
    progress_bar: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = CliArgs::parse();

    let mut player = AudioPlayer::new();
    let controller = player.controller().clone();
    player.open(args.file)?;

    // TODO: avoid sleep
    std::thread::sleep(Duration::from_millis(1000));
    let duration = controller.duration().unwrap().as_millis();
    const FPS: u64 = 15;
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
            let position = controller.position().unwrap().as_millis();
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

            print!("\x1b[2K\r");
            let position = controller.position().unwrap().as_millis();
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
            // print!("\r");
        });
    }

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
