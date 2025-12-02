mod screens;
use screens::{PlayMessage, PlayScreen};

mod theme;
use theme::{icon_from_path, menu_button};

#[derive(Clone)]
pub enum Message {
    PlayScreen(PlayMessage),
    MenuButtonPressed,
}

enum Screen {
    PlayScreen(PlayScreen),
}

impl Default for Screen {
    fn default() -> Self {
        Screen::PlayScreen(PlayScreen::default())
    }
}

#[derive(Default)]
struct App {
    screen: Screen,
}

impl App {
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match (&mut self.screen, message) {
            (Screen::PlayScreen(screen), Message::PlayScreen(play_message)) => {
                screen.update(play_message);
            }
            (Screen::PlayScreen(_), Message::MenuButtonPressed) => {}
        }

        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let content = match &self.screen {
            Screen::PlayScreen(screen) => screen.view().map(Message::PlayScreen),
        };
        let content_area = iced::widget::container(content)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .center(iced::Length::Fill)
            .padding(20)
            .style(|_| {
                iced::widget::container::Style::default().background(iced::Color::from_rgb(
                    9.0 / 255.0,
                    9.0 / 255.0,
                    11.0 / 255.0,
                ))
            });

        let menu_items = [
            ("Play", "assets/svg/play.svg"),
            ("Server", "assets/svg/server.svg"),
            ("Package", "assets/svg/package.svg"),
            ("Java Manager", "assets/svg/coffee.svg"),
            ("Settings", "assets/svg/settings.svg"),
        ];

        let left_stack = menu_items.into_iter().fold(
            iced::widget::Column::new()
                .spacing(8)
                .width(iced::Length::Fill)
                .align_x(iced::Alignment::Center),
            |col, (label, path)| {
                let icon = icon_from_path::<Message>(path);
                let button = menu_button(Some(icon), label)
                    .width(iced::Length::FillPortion(12))
                    .on_press(Message::MenuButtonPressed);

                let padded = iced::widget::row![
                    iced::widget::Space::new().width(iced::Length::FillPortion(1)),
                    button,
                    iced::widget::Space::new().width(iced::Length::FillPortion(1)),
                ]
                .width(iced::Length::Fill);

                col.push(padded)
            },
        );

        let menu_container = iced::widget::container(left_stack)
            .padding(12)
            .width(iced::Length::Fixed(255.0))
            .height(iced::Length::Fill)
            .style(|_| {
                iced::widget::container::Style::default().background(iced::Color::from_rgb(
                    24.0 / 255.0,
                    24.0 / 255.0,
                    27.0 / 255.0,
                ))
            });

        let separator = iced::widget::rule::vertical(1).style(iced::widget::rule::weak);

        iced::widget::row![menu_container, separator, content_area,]
            .height(iced::Length::Fill)
            .width(iced::Length::Fill)
            .align_y(iced::Alignment::Center)
            .into()
    }
}

pub fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("Test rust")
        .theme(iced::Theme::Dracula)
        .run()
}
