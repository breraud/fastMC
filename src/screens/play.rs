use iced::widget::{button, column, container, text};
use iced::{Alignment, Color, Element, Length, Task};
use launcher::LaunchAuth;
use std::process::Stdio;

#[derive(Debug, Clone)]
pub enum Message {
    Launch,
    LaunchStarted,
    LaunchFinished(Result<(), String>),
}

#[derive(Default)]
pub struct PlayScreen {
    is_launching: bool,
    error: Option<String>,
}

impl PlayScreen {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Launch => {
                self.is_launching = true;
                self.error = None;
                Task::done(Message::LaunchStarted)
            }
            Message::LaunchStarted => Task::none(),
            Message::LaunchFinished(result) => {
                self.is_launching = false;
                if let Err(e) = result {
                    self.error = Some(e);
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let title = text("Minecraft 1.21")
            .size(32)
            .style(|_| iced::widget::text::Style {
                color: Some(Color::WHITE),
            });

        let play_button = button(
            container(
                text(if self.is_launching {
                    "Launching..."
                } else {
                    "PLAY"
                })
                .size(20)
                .style(|_| iced::widget::text::Style {
                    color: Some(Color::WHITE),
                }),
            )
            .width(Length::Fill)
            .align_x(Alignment::Center),
        )
        .padding([16, 32])
        .width(Length::Fixed(200.0))
        .style(move |_theme, status| {
            let base = Color::from_rgb(0.13, 0.77, 0.36);
            let hover = Color::from_rgb(0.12, 0.61, 0.30);
            let disabled = Color::from_rgb(0.30, 0.30, 0.35);

            let bg = if self.is_launching {
                disabled
            } else {
                match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => hover,
                    _ => base,
                }
            };

            iced::widget::button::Style {
                background: Some(bg.into()),
                text_color: Color::WHITE,
                border: iced::Border {
                    radius: 8.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press_maybe(if self.is_launching {
            None
        } else {
            Some(Message::Launch)
        });

        let mut content = column![title, play_button]
            .spacing(40)
            .align_x(Alignment::Center);

        if let Some(error) = &self.error {
            content = content.push(text(format!("Error: {}", error)).style(|_| {
                iced::widget::text::Style {
                    color: Some(Color::from_rgb(0.96, 0.47, 0.47)),
                }
            }));
        }

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}
