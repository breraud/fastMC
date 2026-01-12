mod screens;
use screens::{
    JavaManagerMessage, JavaManagerScreen, ModpacksMessage, ModpacksScreen, PlayMessage,
    PlayScreen, ServerMessage, ServerScreen, SettingsMessage, SettingsScreen,
};

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
        }

        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
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

        let left_stack = menu_items.into_iter().fold(
            iced::widget::Column::new()
                .spacing(8)
                .width(iced::Length::Fill)
                .align_x(iced::Alignment::Center),
            |col, (item, label, path)| {
                let icon = icon_from_path::<Message>(path);
                let is_active = self.selected_menu == item;
                let mut button =
                    menu_button(Some(icon), label, is_active).width(iced::Length::FillPortion(12));

                if !is_active {
                    button = button.on_press(Message::MenuItemSelected(item));
                }

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
        .title("FastMC Launcher")
        .theme(iced::Theme::Dracula)
        .run()
}
