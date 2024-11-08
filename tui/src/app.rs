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
    layout::{Constraint, Layout, Position, Rect},
    prelude::{Backend, CrosstermBackend},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Gauge, Paragraph, Widget},
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

    pub(super) fn run(self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
        const FPS: u64 = 240;
        self.player.controller().play();
        let mut seekbar_rect = None;
        let file_path = self
            .track
            .as_ref()
            .expect("todo")
            .file_path()
            .to_string_lossy();
        let track_title = self
            .track
            .as_ref()
            .expect("todo")
            .details()
            .title()
            .unwrap_or_default();
        let track_artist = self
            .track
            .as_ref()
            .expect("todo")
            .details()
            .artist()
            .unwrap_or_default();
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
                    // Need this to avoid percentage sign
                    .label("")
                    .gauge_style(Style {
                        fg: Some(Color::Gray),
                        bg: Some(Color::Black),
                        ..Default::default()
                    });

                // TODO: finish
                let track_info = Paragraph::new(Text::from(vec![
                    Line::from(format!("Title: {}", track_title)),
                    Line::from(format!("Artist: {}", track_artist)),
                    
                ]))
                .block(Block::new().title(format!("Playing: {}", file_path)));

                let progress_info = Text::raw(format!(
                    "[{:02}:{:02}:{:06.3} / {:02}:{:02}:{:06.3}]",
                    position.as_millis() / 3600_000,
                    (position.as_millis() % 3600_000) / 60_000,
                    (position.as_millis() % 60_000) as f64 / 1000.0,
                    duration.as_millis() / 3600_000,
                    (duration.as_millis() % 3600_000) / 60_000,
                    (duration.as_millis() % 60_000) as f64 / 1000.0
                ))
                .centered();

                let layout = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(2),
                    Constraint::Length(1),
                ])
                .split(frame.area());
                seekbar_rect = Some(layout[1]);
                frame.render_widget(track_info, layout[0]);
                frame.render_widget(progress_bar, layout[1]);
                frame.render_widget(progress_info, layout[2]);
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
                            if let Some(rect) = seekbar_rect {
                                if rect.contains(Position {
                                    x: mouse.column,
                                    y: mouse.row,
                                }) {
                                    // TODO: drag will update UI
                                    let seek_position = mouse.column as f64 / rect.width as f64
                                        * duration.as_secs_f64();
                                    self.player
                                        .controller()
                                        .seek(Duration::from_secs_f64(seek_position))
                                }
                            }
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
