mod screens;
use screens::{
    AccountMessage, AccountScreen, AccountUpdate, JavaManagerMessage, JavaManagerScreen,
    ModpacksMessage, ModpacksScreen, PlayMessage, PlayScreen, ServerMessage, ServerScreen,
    SettingsMessage, SettingsScreen,
};

mod game;
mod theme;
use theme::{icon_from_path, menu_button};

use account_manager::AccountKind;
use fastmc_config::FastmcConfig;
use iced::window;

#[derive(Clone)]
pub enum Message {
    AccountScreen(Box<AccountMessage>),
    PlayScreen(PlayMessage),
    ServerScreen(ServerMessage),
    ModpacksScreen(ModpacksMessage),
    JavaManagerScreen(JavaManagerMessage),
    SettingsScreen(SettingsMessage),
    MenuItemSelected(MenuItem),
    AccountPressed,
    Resized(f32),
    Startup,
    AccountValidated(Result<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    AccountSetup,
    Main,
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
    stage: Stage,
    selected_menu: MenuItem,
    account: AccountScreen,
    play: PlayScreen,
    server: ServerScreen,
    modpacks: ModpacksScreen,
    java_manager: JavaManagerScreen,
    settings: SettingsScreen,
}

const DEV_MICROSOFT_CLIENT_ID: Option<&str> = Some("f9bf1dc0-bf65-42d6-a1af-f0aa35386a85");

impl App {
    fn new() -> (Self, iced::Task<Message>) {
        let config = FastmcConfig::load().unwrap_or_default();
        let client_id = config
            .accounts
            .microsoft_client_id
            .clone()
            .or_else(|| DEV_MICROSOFT_CLIENT_ID.map(|s| s.to_string()));

        let account = AccountScreen::new(client_id);
        let stage = if account.has_accounts() {
            Stage::Main
        } else {
            Stage::AccountSetup
        };

        let app = Self {
            stage,
            selected_menu: MenuItem::Play,
            account,
            play: PlayScreen::default(),
            server: ServerScreen,
            modpacks: ModpacksScreen,
            java_manager: JavaManagerScreen::new(),
            settings: SettingsScreen,
        };

        (app, iced::Task::done(Message::Startup))
    }
}

impl App {
    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::AccountScreen(account_message) => {
                let (action, task) = self.account.update(*account_message);

                let navigation_task = if matches!(action, AccountUpdate::EnterLauncher)
                    && self.account.has_accounts()
                {
                    let config = FastmcConfig::load().unwrap_or_default();
                    let client_id = config
                        .accounts
                        .microsoft_client_id
                        .clone()
                        .or_else(|| DEV_MICROSOFT_CLIENT_ID.map(|s| s.to_string()));

                    iced::Task::perform(
                        async move {
                            if let Some(cid) = client_id {
                                use account_manager::AccountService;
                                // We re-instantiate service here for validation.
                                // In a real app we might want shared state, but this is safe for now.
                                let mut service =
                                    AccountService::new(cid).map_err(|e| e.to_string())?;
                                let account = service
                                    .validate_active_account()
                                    .map_err(|e| e.to_string())?;
                                Ok(account.display_name.clone())
                            } else {
                                Ok("Offline/NoID".to_string())
                            }
                        },
                        Message::AccountValidated,
                    )
                } else {
                    iced::Task::none()
                };

                iced::Task::batch(vec![
                    task.map(|msg| Message::AccountScreen(Box::new(msg))),
                    navigation_task,
                ])
            }
            Message::PlayScreen(play_message) => {
                let task = match play_message {
                    PlayMessage::LaunchStarted => {
                        let active_account = self.account.active_account().cloned();
                        if let Some(account) = active_account {
                            if account.requires_login {
                                // Prevent launch if login required
                                // Ideally we would switch to AccountSetup stage here too
                                self.stage = Stage::AccountSetup;
                                return iced::Task::none();
                            }

                            // Prepare secrets
                            let secrets_result =
                                if let AccountKind::Microsoft { .. } = &account.kind {
                                    self.account.get_microsoft_tokens(&account.id)
                                } else {
                                    None
                                };

                            iced::Task::perform(
                                async move {
                                    use directories::ProjectDirs;
                                    let dirs =
                                        ProjectDirs::from("com", "fastmc", "fastmc").unwrap();
                                    let game_dir = dirs.data_dir().join("minecraft");

                                    // Detect Java
                                    let java_config = java_manager::JavaDetectionConfig::default();
                                    let summary = java_manager::detect_installations(&java_config);

                                    println!(
                                        "Found {} Java installations:",
                                        summary.installations.len()
                                    );
                                    for install in &summary.installations {
                                        println!(
                                            "- Path: {:?}, Version: {:?}",
                                            install.path, install.version
                                        );
                                    }

                                    let java_path = summary
                                        .installations
                                        .iter()
                                        .find(|i| {
                                            i.version
                                                .as_ref()
                                                .map(|v| {
                                                    v.starts_with("21")
                                                        || v.starts_with("22")
                                                        || v.starts_with("23")
                                                })
                                                .unwrap_or(false)
                                        })
                                        .map(|i| i.path.clone())
                                        .or_else(|| {
                                            // Fallback to searching for *any* Java if 21 isn't explicitly found,
                                            // essentially trusting the user's PATH or JAVA_HOME might be newer than what capture suggests,
                                            // or picking the "best" available.
                                            // For now, let's look for the highest version we can parse.
                                            summary
                                                .installations
                                                .iter()
                                                .max_by_key(|i| {
                                                    i.version
                                                        .as_ref()
                                                        .and_then(|v| {
                                                            v.split(|c: char| !c.is_numeric())
                                                                .next()
                                                        }) // Grab first numeric component
                                                        .and_then(|s| s.parse::<i32>().ok())
                                                        .unwrap_or(0)
                                                })
                                                .map(|i| i.path.clone())
                                        })
                                        .unwrap_or_else(|| std::path::PathBuf::from("java"));

                                    println!("Selected Java path: {:?}", java_path);

                                    let access_token =
                                        secrets_result.map(|s| s.access_token).unwrap_or_default();

                                    match game::prepare_and_launch(
                                        &account,
                                        &access_token,
                                        java_path,
                                        game_dir,
                                    ) {
                                        Ok(mut cmd) => match cmd.spawn() {
                                            Ok(_) => Ok(()),
                                            Err(e) => {
                                                Err(format!("Failed to start process: {}", e))
                                            }
                                        },
                                        Err(e) => Err(e),
                                    }
                                },
                                |res| Message::PlayScreen(PlayMessage::LaunchFinished(res)),
                            )
                        } else {
                            iced::Task::done(Message::PlayScreen(PlayMessage::LaunchFinished(Err(
                                "No active account".to_string(),
                            ))))
                        }
                    }
                    _ => self.play.update(play_message).map(Message::PlayScreen),
                };
                task
            }
            Message::ServerScreen(server_message) => {
                self.server.update(server_message);
                iced::Task::none()
            }
            Message::ModpacksScreen(modpacks_message) => {
                self.modpacks.update(modpacks_message);
                iced::Task::none()
            }
            Message::JavaManagerScreen(java_manager_message) => {
                let task = self.java_manager.update(java_manager_message);
                task.map(Message::JavaManagerScreen)
            }
            Message::SettingsScreen(settings_message) => {
                self.settings.update(settings_message);
                iced::Task::none()
            }
            Message::MenuItemSelected(item) => {
                self.stage = Stage::Main;
                self.selected_menu = item;
                iced::Task::none()
            }
            Message::AccountPressed => {
                self.stage = Stage::AccountSetup;
                iced::Task::none()
            }
            Message::Resized(width) => {
                let task = self.java_manager.update(JavaManagerMessage::Resized(width));
                task.map(Message::JavaManagerScreen)
            }
            Message::Startup => {
                let config = FastmcConfig::load().unwrap_or_default();
                let client_id = config
                    .accounts
                    .microsoft_client_id
                    .clone()
                    .or_else(|| DEV_MICROSOFT_CLIENT_ID.map(|s| s.to_string()));

                iced::Task::perform(
                    async move {
                        if let Some(cid) = client_id {
                            use account_manager::AccountService;
                            let mut service =
                                AccountService::new(cid).map_err(|e| e.to_string())?;
                            let account = service
                                .validate_active_account()
                                .map_err(|e| e.to_string())?;
                            Ok(account.display_name.clone())
                        } else {
                            // No client ID means we can't validate Microsoft accounts,
                            // but we might not have one active.
                            // For now, just assume success or return a distinctive message.
                            Ok("Offline/NoID".to_string())
                        }
                    },
                    Message::AccountValidated,
                )
            }
            Message::AccountValidated(result) => {
                match result {
                    Ok(_) => {
                        self.stage = Stage::Main;
                    }
                    Err(e) => {
                        println!("Account validation failed: {}", e);
                        // If validation fails, force back to account setup
                        self.stage = Stage::AccountSetup;
                        // Reload account screen to pick up the "requires_login" state change from disk
                        // Since AccountScreen loads from disk on `new`, we can just re-init it or add a reload method.
                        // For simplicity let's re-init.
                        let config = FastmcConfig::load().unwrap_or_default();
                        let client_id = config
                            .accounts
                            .microsoft_client_id
                            .clone()
                            .or_else(|| DEV_MICROSOFT_CLIENT_ID.map(|s| s.to_string()));
                        self.account = AccountScreen::new(client_id);
                    }
                }
                iced::Task::none()
            }
        }
    }

    fn view(&self) -> iced::Element<'_, Message> {
        match self.stage {
            Stage::AccountSetup => self
                .account
                .view()
                .map(|msg| Message::AccountScreen(Box::new(msg))),
            Stage::Main => self.main_view(),
        }
    }

    fn main_view(&self) -> iced::Element<'_, Message> {
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

        let (account_title, account_subtitle, account_badge_text) =
            if let Some(account) = self.account.active_account() {
                let badge = account
                    .display_name
                    .chars()
                    .next()
                    .unwrap_or('A')
                    .to_string();
                let subtitle = match &account.kind {
                    AccountKind::Microsoft { username, .. } => {
                        format!("Microsoft • {username}")
                    }
                    AccountKind::Offline { username, .. } => format!("Offline • {username}"),
                };

                (account.display_name.clone(), subtitle, badge)
            } else {
                (
                    "Add account".to_string(),
                    "Connect your Microsoft profile".to_string(),
                    "+".to_string(),
                )
            };

        let account_avatar =
            iced::widget::container(iced::widget::text(account_badge_text).size(16).style(
                move |_| iced::widget::text::Style {
                    color: Some(text_primary),
                },
            ))
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
            iced::widget::text(account_title)
                .size(16)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_primary),
                }),
            iced::widget::text(account_subtitle)
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
    iced::application(App::new, App::update, App::view)
        .title("FastMC Launcher")
        .theme(iced::Theme::Dracula)
        .subscription(|_| window::resize_events().map(|(_, size)| Message::Resized(size.width)))
        .run()
}
