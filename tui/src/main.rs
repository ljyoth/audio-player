mod app;

use app::AudioPlayerApplication;
use clap::{ArgAction, Parser};
use color_eyre::eyre::Result;
use ratatui::crossterm;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct CliArgs {
    file: PathBuf,
    #[arg(short, long, default_value_t = true, action=ArgAction::SetFalse)]
    progress_bar: bool,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = CliArgs::parse();

    let mut app = AudioPlayerApplication::new();
    app.open(args.file)?;

    let mut terminal = ratatui::init_with_options(ratatui::TerminalOptions {
        viewport: ratatui::Viewport::Fullscreen,
        // viewport: ratatui::Viewport::Inline(10),
    });
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::EnterAlternateScreen
    )?;
    let result = app.run(&mut terminal);
    ratatui::restore();
    crossterm::execute!(terminal.backend_mut())?;
    result
}
