use std::{path::PathBuf, time::Duration};

use iced::{
    alignment::Vertical,
    font, time,
    widget::{button, column, container, image, lazy, row, svg, text, Space},
    window, Alignment, Background, Border, Color, Element, Font, Length, Padding, Size,
    Subscription, Task, Theme,
};
use iced_aw::{
    menu::{self, Item, Menu},
    menu_bar, menu_items, SlideBar,
};
use rfd::FileDialog;

use crate::player::AudioPlayer;

pub(super) struct AudioPlayerApplication {
    player: AudioPlayer,
    seeking: bool,
    playback_position: f64,
}

#[derive(Debug, Clone)]
pub(super) enum Message {
    Play,
    Pause,
    SyncPosition,
    BeginSeek(f64),
    ConfirmSeek,
    OpenFilePicker,
    Stop,
    Resize(Size),
}

impl AudioPlayerApplication {
    pub(super) fn new(flags: AudioPlayerFlags) -> (Self, Task<Message>) {
        let mut player = AudioPlayer::new();
        // TODO handle error
        if let Some(p) = flags.file_path {
            player.open(p).expect("failed to open");
        }
        (
            Self {
                player,
                seeking: false,
                playback_position: 0.0,
            },
            Task::none(),
        )
    }

    pub(super) fn title(&self) -> String {
        if let Some(track) = self.player.current() {
            if let Some(title) = track.details().title() {
                return format!("Audio Player - {}", title);
            } else {
                return format!("Audio Player - {}", track.file_path().to_string_lossy());
            }
        }
        "Audio Player".into()
    }

    pub(super) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Play => self.player.play(),
            Message::Pause => self.player.pause(),
            Message::SyncPosition => {
                self.playback_position = self.player.position().as_micros() as f64;
            }
            Message::BeginSeek(position) => {
                self.seeking = true;
                self.playback_position = position;
            }
            Message::ConfirmSeek => {
                self.player
                    .seek(Duration::from_micros(self.playback_position as u64));
                self.seeking = false;
            }
            Message::OpenFilePicker => {
                if let Some(file) = FileDialog::new().pick_file() {
                    self.player.stop();
                    // TODO handle error
                    self.player.open(file).expect("failed to open");
                }
            }
            Message::Stop => self.player.stop(),
            Message::Resize(size) => {
                return window::get_oldest().then(move |id| {
                    if let Some(id) = id {
                        return window::resize(id, size);
                    }
                    Task::none()
                });
            }
        };
        Task::none()
    }

    pub(super) fn subscription(&self) -> iced::Subscription<Message> {
        if self.player.playing() && !self.seeking {
            const FPS: u64 = 240;
            // TODO use stream update or something to avoid smol dependency
            return Subscription::batch([time::every(time::Duration::from_millis(1000u64 / FPS))
                .map(|_| Message::SyncPosition)]);
        }
        Subscription::none()
    }

    pub(super) fn view(&self) -> Element<Message> {
        fn menu_button_style(theme: &Theme, status: button::Status) -> button::Style {
            match status {
                button::Status::Active | button::Status::Disabled => button::Style {
                    text_color: theme.extended_palette().background.base.text,
                    background: Some(Color::TRANSPARENT.into()),
                    ..Default::default()
                },
                _ => Default::default(),
            }
        }
        let menu = Menu::new(menu_items![(button("Open")
            .style(menu_button_style)
            .on_press(Message::OpenFilePicker))(
            button("Close")
                .style(menu_button_style)
                .on_press(Message::Stop)
        )]);
        let menu_bar = menu_bar![(
            button("File").style(menu_button_style),
            menu.width(100).offset(5.0)
        )]
        .draw_path(menu::DrawPath::Backdrop);

        // TODO: requires https://github.com/iced-rs/iced/issues/36 to implement selectable text
        let track_description = container(lazy(self.player.current(), |&current| {
            match current {
                Some(track) => {
                    let file_path = text(track.file_path().to_string_lossy().to_string())
                        .shaping(text::Shaping::Advanced)
                        .font(Font {
                            weight: font::Weight::Light,
                            ..Default::default()
                        })
                        .size(16);
                    let title = match track.details().title() {
                        Some(title) => text(title.to_string())
                            .size(36)
                            .font(Font {
                                weight: font::Weight::Bold,
                                ..Default::default()
                            })
                            .shaping(text::Shaping::Advanced)
                            .into(),
                        None => Element::from(Space::with_height(0)),
                    };
                    let artist = match track.details().artist() {
                        Some(title) => text(title.to_string())
                            .size(24)
                            .shaping(text::Shaping::Advanced)
                            .into(),
                        None => Element::from(Space::with_height(0)),
                    };
                    let separator = Space::with_height(10);
                    let cover = match track.details().cover() {
                        Some(cover) => {
                            let handle = image::Handle::from_bytes(cover.data().clone());
                            image::Image::new(handle).height(Length::Fill).into()
                        }
                        None => Element::from(Space::with_height(Length::Fill)),
                    };
                    column![file_path, title, artist, separator, cover]
                        .align_x(Alignment::Center)
                        .into()
                }
                None => Element::from(Space::with_height(0)),
            }
        }))
        .height(Length::Fill)
        .align_y(Vertical::Center);

        let track_duration = match self.player.current() {
            Some(track) => match track.details().duration() {
                Some(duration) => duration.as_micros() as f64,
                None => 0.0,
            },
            None => 0.0,
        };
        let mut slide_bar = SlideBar::new(
            0.0..=track_duration,
            self.playback_position,
            Message::BeginSeek,
        )
        .on_release(Message::ConfirmSeek);
        slide_bar.color = self.theme().extended_palette().primary.base.color;
        let seek_progress = container(slide_bar)
            .padding(Padding {
                top: 0.0,
                right: 20.0,
                bottom: 0.0,
                left: 20.0,
            })
            .center_x(Length::Fill)
            .align_y(Vertical::Bottom);

        fn play_pause_svg_style(theme: &Theme, _: svg::Status) -> svg::Style {
            svg::Style {
                color: Some(theme.extended_palette().primary.strong.text),
                ..Default::default()
            }
        }
        fn play_pause_button_style(theme: &Theme, status: button::Status) -> button::Style {
            match status {
                _ => button::Style {
                    background: Some(Background::Color(
                        theme.extended_palette().primary.strong.color,
                    )),
                    border: Border {
                        radius: 45.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            }
        }
        let play_pause_button = lazy(self.player.playing(), |playing| match playing {
            false => {
                let play_svg = svg(svg::Handle::from_memory(include_bytes!(
                    "../assets/play.svg"
                )))
                .style(play_pause_svg_style);
                button(play_svg)
                    .style(play_pause_button_style)
                    .height(45)
                    .width(45)
                    .on_press(Message::Play)
            }
            true => {
                let pause_svg = svg(svg::Handle::from_memory(include_bytes!(
                    "../assets/pause.svg"
                )))
                .style(play_pause_svg_style);
                button(pause_svg)
                    .style(play_pause_button_style)
                    .height(45)
                    .width(45)
                    .on_press(Message::Pause)
            }
        });
        // let stop_button = button(text("Stop")).on_press(Message::Stop);
        let controls = container(
            row![play_pause_button
            // , stop_button
            ]
            .spacing(20),
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .align_y(Vertical::Bottom);

        column![
            menu_bar,
            container(
                column![track_description, seek_progress, controls]
                    .spacing(10)
                    .align_x(Alignment::Center),
            )
        ]
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 10.0,
            left: 0.0,
        })
        .height(Length::Fill)
        .into()
    }

    pub(super) fn theme(&self) -> iced::Theme {
        <iced::Theme as std::default::Default>::default()
    }
}

#[derive(Debug, Default)]
pub(super) struct AudioPlayerFlags {
    pub(super) file_path: Option<PathBuf>,
}
