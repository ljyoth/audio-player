use std::{path::PathBuf, time::Duration};

use iced::{
    alignment::Vertical,
    executor, theme, time,
    widget::{button, column, container, image, lazy, row, svg, Space},
    window, Alignment, Application, Background, Border, Color, Command, Element, Length, Padding,
    Size, Subscription, Theme,
};
use iced_aw::{
    menu::{self, Item, Menu},
    menu_bar, menu_items, SlideBar,
};
use rfd::FileDialog;

use crate::player::AudioPlayer;

pub(super) struct MusicPlayerApplication {
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

impl Application for MusicPlayerApplication {
    type Message = Message;
    type Executor = executor::Default;
    type Theme = theme::Theme;
    type Flags = MusicPlayerFlags;

    fn new(flags: Self::Flags) -> (Self, iced::Command<Message>) {
        let mut player = AudioPlayer::new().unwrap();
        player.open(flags.file_path).unwrap();
        (
            Self {
                player,
                seeking: false,
                playback_position: 0.0,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        if let Some(track) = self.player.current() {
            if let Some(title) = track.title() {
                return format!("Music Player - {}", title);
            }
        }
        "Music Player".into()
    }

    fn update(&mut self, message: Message) -> iced::Command<Message> {
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
                    .seek(Duration::from_micros(self.playback_position as u64))
                    .unwrap();
                self.seeking = false;
            }
            Message::OpenFilePicker => {
                if let Some(file) = FileDialog::new().pick_file() {
                    self.player.stop();
                    self.player.open(file).unwrap();
                }
            }
            Message::Stop => self.player.stop(),
            Message::Resize(size) => {
                return window::resize(window::Id::MAIN, size);
            }
        };
        Command::none()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        if self.player.playing() && !self.seeking {
            const FPS: u64 = 240;
            // TODO use stream update or something to avoid smol dependency
            return Subscription::batch([time::every(time::Duration::from_millis(1000u64 / FPS))
                .map(|_| Message::SyncPosition)]);
        }
        Subscription::none()
    }

    fn view(&self) -> Element<Message> {
        let menu = Menu::new(menu_items![(button("Open")
            .style(theme::Button::custom(MenuButtonStyle {}))
            .on_press(Message::OpenFilePicker))(
            button("Close")
                .style(theme::Button::custom(MenuButtonStyle {}))
                .on_press(Message::Stop)
        )]);
        let menu_bar = menu_bar![(
            button("File").style(theme::Button::custom(MenuButtonStyle {})),
            menu.width(100).offset(5.0)
        )]
        .draw_path(menu::DrawPath::Backdrop);

        let cover_image = container(lazy(
            self.player.current().as_ref().map(|&track| track.title()),
            |_| {
                if let Some(track) = self.player.current() {
                    if let Some(cover) = track.cover() {
                        // TODO: avoid clones
                        let handle = image::Handle::from_memory(cover.data.clone());
                        return image::viewer(handle).into();
                    }
                }
                Element::from(Space::with_height(0))
            },
        ))
        .height(Length::Fill)
        .align_y(Vertical::Center);

        let track_duration = match self.player.current() {
            Some(track) => match track.duration() {
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
            .center_x()
            .align_y(Vertical::Bottom);

        let play_pause_button = match self.player.playing() {
            false => {
                let play_svg = svg(svg::Handle::from_memory(include_bytes!(
                    "../assets/play.svg"
                )))
                .height(30.0)
                .width(30.0)
                .style(theme::Svg::custom_fn(|style| svg::Appearance {
                    color: Some(style.extended_palette().primary.strong.text),
                }));
                button(play_svg)
                    .style(theme::Button::custom(PlayPauseButtonStyle))
                    .on_press(Message::Play)
            }
            true => {
                let pause_svg = svg(svg::Handle::from_memory(include_bytes!(
                    "../assets/pause.svg"
                )))
                .height(30.0)
                .width(30.0)
                .style(theme::Svg::custom_fn(|style| svg::Appearance {
                    color: Some(style.extended_palette().primary.strong.text),
                }));
                button(pause_svg)
                    .style(theme::Button::custom(PlayPauseButtonStyle))
                    .on_press(Message::Pause)
            }
        };
        // let stop_button = button(text("Stop")).on_press(Message::Stop);
        let controls = container(
            row![play_pause_button
            // , stop_button
            ]
            .spacing(20),
        )
        .width(Length::Fill)
        .center_x()
        .align_y(Vertical::Bottom);

        column![
            menu_bar,
            container(
                column![cover_image, seek_progress, controls]
                    .spacing(10)
                    .align_items(Alignment::Center),
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
}

#[derive(Debug, Default)]
pub(super) struct MusicPlayerFlags {
    pub(super) file_path: PathBuf,
}

struct MenuButtonStyle;

impl button::StyleSheet for MenuButtonStyle {
    type Style = Theme;

    fn active(&self, style: &Self::Style) -> button::Appearance {
        button::Appearance {
            text_color: style.extended_palette().background.base.text,
            background: Some(Color::TRANSPARENT.into()),
            ..Default::default()
        }
    }

    fn disabled(&self, style: &Self::Style) -> button::Appearance {
        self.active(style)
    }
}

struct PlayPauseButtonStyle;

impl button::StyleSheet for PlayPauseButtonStyle {
    type Style = Theme;

    fn active(&self, style: &Self::Style) -> button::Appearance {
        button::Appearance {
            background: Some(Background::Color(
                style.extended_palette().primary.strong.color,
            )),
            border: Border {
                radius: [30.0, 30.0, 30.0, 30.0].into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
