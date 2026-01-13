mod screens;
use screens::{
    JavaManagerMessage, JavaManagerScreen, ModpacksMessage, ModpacksScreen, PlayMessage,
    PlayScreen, ServerMessage, ServerScreen, SettingsMessage, SettingsScreen,
};

mod account;
mod theme;
use theme::{icon_from_path, menu_button};

#[derive(Clone)]
pub enum Message {
    PlayScreen(PlayMessage),
    ServerScreen(ServerMessage),
    ModpacksScreen(ModpacksMessage),
    JavaManagerScreen(JavaManagerMessage),
    SettingsScreen(SettingsMessage),
    MenuItemSelected(MenuItem),
    AccountPressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Play,
    Server,
    Modpacks,
    JavaManager,
    Settings,
}

struct App {
    selected_menu: MenuItem,
    play: PlayScreen,
    server: ServerScreen,
    modpacks: ModpacksScreen,
    java_manager: JavaManagerScreen,
    settings: SettingsScreen,
}

impl Default for App {
    fn default() -> Self {
        Self {
            selected_menu: MenuItem::Play,
            play: PlayScreen::default(),
            server: ServerScreen,
            modpacks: ModpacksScreen,
            java_manager: JavaManagerScreen,
            settings: SettingsScreen,
        }
    }
}

impl App {
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::PlayScreen(play_message) => {
                self.play.update(play_message);
            }
            Message::ServerScreen(server_message) => {
                self.server.update(server_message);
            }
            Message::ModpacksScreen(modpacks_message) => {
                self.modpacks.update(modpacks_message);
            }
            Message::JavaManagerScreen(java_manager_message) => {
                self.java_manager.update(java_manager_message);
            }
            Message::SettingsScreen(settings_message) => {
                self.settings.update(settings_message);
            }
            Message::MenuItemSelected(item) => {
                self.selected_menu = item;
            }
            Message::AccountPressed => {}
        }

        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let sidebar_background = iced::Color::from_rgb(0.10, 0.10, 0.12);
        let text_primary = iced::Color::from_rgb(0.88, 0.89, 0.91);
        let text_muted = iced::Color::from_rgb(0.63, 0.64, 0.67);
        let accent = iced::Color::from_rgb(0.13, 0.77, 0.36);
        let divider_color = iced::Color::from_rgb(0.18, 0.18, 0.21);

        let content = match self.selected_menu {
            MenuItem::Play => self.play.view().map(Message::PlayScreen),
            MenuItem::Server => self.server.view().map(Message::ServerScreen),
            MenuItem::Modpacks => self.modpacks.view().map(Message::ModpacksScreen),
            MenuItem::JavaManager => self.java_manager.view().map(Message::JavaManagerScreen),
            MenuItem::Settings => self.settings.view().map(Message::SettingsScreen),
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
            (MenuItem::Play, "Play", "assets/svg/play.svg"),
            (MenuItem::Server, "Server", "assets/svg/server.svg"),
            (MenuItem::Modpacks, "Modpacks", "assets/svg/package.svg"),
            (
                MenuItem::JavaManager,
                "Java Manager",
                "assets/svg/coffee.svg",
            ),
            (MenuItem::Settings, "Settings", "assets/svg/settings.svg"),
        ];

        let badge = iced::widget::container(iced::widget::text("MC").size(18).style(move |_| {
            iced::widget::text::Style {
                color: Some(iced::Color::WHITE),
            }
        }))
        .padding([10, 14])
        .width(iced::Length::Fixed(52.0))
        .height(iced::Length::Fixed(52.0))
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center)
        .style(move |_| iced::widget::container::Style {
            background: Some(accent.into()),
            border: iced::Border {
                radius: 14.0.into(),
                ..iced::Border::default()
            },
            ..iced::widget::container::Style::default()
        });

        let header = iced::widget::row![
            badge,
            iced::widget::column![
                iced::widget::text("Minecraft").size(20).style(move |_| {
                    iced::widget::text::Style {
                        color: Some(text_primary),
                    }
                }),
                iced::widget::text("Launcher")
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_muted),
                    }),
            ]
            .spacing(2)
        ]
        .spacing(14)
        .align_y(iced::Alignment::Center);

        let top_divider = iced::widget::container(
            iced::widget::Space::new()
                .width(iced::Length::Fill)
                .height(iced::Length::Fixed(1.0)),
        )
        .style(move |_| iced::widget::container::Style {
            background: Some(divider_color.into()),
            ..iced::widget::container::Style::default()
        });

        let menu_list = menu_items.into_iter().fold(
            iced::widget::Column::new()
                .spacing(12)
                .width(iced::Length::Fill),
            |col, (item, label, path)| {
                let icon = icon_from_path::<Message>(path);
                let is_active = self.selected_menu == item;
                let mut button =
                    menu_button(Some(icon), label, is_active).width(iced::Length::Fill);

                if !is_active {
                    button = button.on_press(Message::MenuItemSelected(item));
                }

                col.push(button)
            },
        );

        let menu_section = iced::widget::container(menu_list)
            .padding([8, 4])
            .width(iced::Length::Fill);

        let account_avatar =
            iced::widget::container(iced::widget::text("P").size(16).style(move |_| {
                iced::widget::text::Style {
                    color: Some(text_primary),
                }
            }))
            .width(iced::Length::Fixed(44.0))
            .height(iced::Length::Fixed(44.0))
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Color::from_rgb(0.15, 0.15, 0.18).into()),
                border: iced::Border {
                    radius: 12.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            });

        let account_info = iced::widget::column![
            iced::widget::text("Steve_Miner")
                .size(16)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_primary),
                }),
            iced::widget::text("Premium Account")
                .size(13)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_muted),
                }),
        ]
        .spacing(2);

        let account_button = iced::widget::button(
            iced::widget::row![account_avatar, account_info]
                .align_y(iced::Alignment::Center)
                .spacing(12),
        )
        .padding([12, 14])
        .width(iced::Length::Fill)
        .on_press(Message::AccountPressed)
        .style(move |_theme, status| {
            let base = iced::Color::from_rgb(0.15, 0.15, 0.18);
            let hover = iced::Color::from_rgb(0.19, 0.19, 0.22);
            iced::widget::button::Style {
                background: Some(
                    match status {
                        iced::widget::button::Status::Hovered
                        | iced::widget::button::Status::Pressed => hover,
                        _ => base,
                    }
                    .into(),
                ),
                text_color: text_primary,
                border: iced::Border {
                    radius: 16.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        });

        let bottom_divider = iced::widget::container(
            iced::widget::Space::new()
                .width(iced::Length::Fill)
                .height(iced::Length::Fixed(1.0)),
        )
        .style(move |_| iced::widget::container::Style {
            background: Some(divider_color.into()),
            ..iced::widget::container::Style::default()
        });

        let sidebar_content = iced::widget::column![
            header,
            top_divider,
            menu_section,
            iced::widget::Space::new().height(iced::Length::Fill),
            bottom_divider,
            account_button
        ]
        .spacing(18)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill);

        let menu_container = iced::widget::container(sidebar_content)
            .padding([20, 18])
            .width(iced::Length::Fixed(280.0))
            .height(iced::Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(sidebar_background.into()),
                ..iced::widget::container::Style::default()
            });

        let separator = iced::widget::container(
            iced::widget::Space::new()
                .width(iced::Length::Fixed(1.0))
                .height(iced::Length::Fill),
        )
        .style(move |_| iced::widget::container::Style {
            background: Some(divider_color.into()),
            ..iced::widget::container::Style::default()
        });

        iced::widget::row![menu_container, separator, content_area,]
            .height(iced::Length::Fill)
            .width(iced::Length::Fill)
            .align_y(iced::Alignment::Center)
            .into()
    }
}

pub fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("FastMC Launcher")
        .theme(iced::Theme::Dracula)
        .run()
}
