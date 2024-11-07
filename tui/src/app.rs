use std::{
    io::Stdout,
    path::{Path, PathBuf},
    time::Duration,
};

use audio_player::{AudioPlayer, TrackDetails};
use color_eyre::eyre::{Ok, Result};
use ratatui::{
    crossterm::{
        event::{self, Event, KeyCode, KeyEventKind, MouseButton},
        terminal,
    },
    layout::{Constraint, Layout, Rect},
    prelude::{Backend, CrosstermBackend},
    widgets::{Block, Gauge},
    Terminal,
};

pub(super) struct AudioPlayerApplication {
    player: AudioPlayer,
    track: Option<Track>,
}

impl AudioPlayerApplication {
    pub(super) fn new() -> Self {
        let player = AudioPlayer::new();
        Self {
            player,
            track: None,
        }
    }

    pub(super) fn open<P: AsRef<Path>>(&mut self, file_path: P) -> Result<()> {
        let track = self.player.open(file_path.as_ref())?;
        self.track = Some(Track {
            file_path: file_path.as_ref().to_path_buf(),
            details: track.details().clone(),
        });
        self.player.queue(track)?;
        Ok(())
    }

    pub(super) fn run(self, mut terminal: Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        self.player.controller().play();
        const FPS: u64 = 240;
        loop {
            let position = self
                .player
                .controller()
                .position()
                .unwrap_or(Duration::from_secs(0));
            let duration = self
                .track
                .as_ref()
                // TODO: do not unwrap
                .unwrap()
                .details()
                .duration()
                .cloned()
                .unwrap_or(Duration::from_secs(0));
            terminal.draw(|frame| {
                let progress_bar = Gauge::default()
                    .ratio(position.as_micros() as f64 / duration.as_micros() as f64)
                    .use_unicode(true)
                    .block(Block::new().title("Progress"))
                    .label(format!(
                        "[{:02}:{:02}:{:06.3} / {:02}:{:02}:{:06.3}]",
                        position.as_millis() / 3600_000,
                        (position.as_millis() % 3600_000) / 60_000,
                        (position.as_millis() % 60_000) as f64 / 1000.0,
                        duration.as_millis() / 3600_000,
                        (duration.as_millis() % 3600_000) / 60_000,
                        (duration.as_millis() % 60_000) as f64 / 1000.0
                    ));
                let layout = Layout::vertical([Constraint::Length(2)]).split(frame.area());
                frame.render_widget(progress_bar, layout[0]);
            })?;

            if event::poll(Duration::from_millis(1000 / FPS))? {
                match event::read()? {
                    Event::Key(key) => match key.kind {
                        KeyEventKind::Press => match key.code {
                            KeyCode::Char('q') => break Ok(()),
                            KeyCode::Char(' ') => {
                                if self.player.controller().playing() {
                                    self.player.controller().pause();
                                } else {
                                    self.player.controller().play();
                                }
                            }
                            _ => (),
                        },
                        _ => (),
                    },
                    Event::Mouse(mouse) => match mouse.kind {
                        event::MouseEventKind::Up(MouseButton::Left) => {
                            eprintln!("{}, {}", mouse.row, mouse.column)
                        }
                        _ => (),
                    },
                    _ => (),
                };
            };
        }
    }
}

pub(super) struct Track {
    file_path: PathBuf,
    details: TrackDetails,
}

impl Track {
    pub(super) fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    pub(super) fn details(&self) -> &TrackDetails {
        &self.details
    }
}
