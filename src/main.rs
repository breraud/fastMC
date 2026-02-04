mod screens;
use screens::{
    AccountMessage, AccountScreen, AccountUpdate, InstancesMessage, InstancesScreen,
    JavaManagerMessage, JavaManagerScreen, LoadingScreen, ModpacksMessage, ModpacksScreen,
    PlayMessage, PlayScreen, ServerMessage, ServerScreen, SettingsMessage, SettingsScreen,
};

mod game;
mod theme;
use theme::{icon_from_path, menu_button};

pub mod assets;
pub mod instance_manager;

use account_manager::AccountKind;
use config_manager::FastmcConfig;
use iced::window;
use image as image_crate;

#[derive(Clone)]
pub enum Message {
    AccountScreen(Box<AccountMessage>),
    PlayScreen(PlayMessage),
    ServerScreen(ServerMessage),
    ModpacksScreen(ModpacksMessage),
    JavaManagerScreen(JavaManagerMessage),
    InstancesScreen(InstancesMessage),
    SettingsScreen(SettingsMessage),
    MenuItemSelected(MenuItem),
    AccountPressed,
    Resized(f32),
    Startup,
    AccountValidated(Result<String, String>),
    AssetsLoaded(assets::AssetStore),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Loading,
    AccountSetup,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    Play,
    Server,
    Modpacks,
    JavaManager,
    Instances,
    Settings,
}

struct App {
    stage: Stage,
    loading: LoadingScreen,
    assets: Option<assets::AssetStore>,
    // Store validation result while waiting for assets
    validation_result: Option<Result<String, String>>,
    selected_menu: MenuItem,
    account: AccountScreen,
    play: PlayScreen,
    server: ServerScreen,
    modpacks: ModpacksScreen,

    java_manager: JavaManagerScreen,
    instances: InstancesScreen,
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
        // Start in "Loading" stage now
        let stage = Stage::Loading;

        let app = Self {
            stage,
            loading: LoadingScreen,
            assets: None,
            validation_result: None,
            selected_menu: MenuItem::Play,
            account,
            play: PlayScreen::default(),
            server: ServerScreen,
            modpacks: ModpacksScreen,

            java_manager: JavaManagerScreen::new(),
            instances: InstancesScreen::new(),
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
                                    .await
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
                match play_message {
                    PlayMessage::LaunchStarted => {
                        let active_account = self.account.active_account().cloned();

                        // We need the active instance ID from the play screen
                        let instance_id = if let Some(meta) = self.play.active_instance() {
                            meta.id.clone()
                        } else {
                            return iced::Task::done(Message::PlayScreen(
                                PlayMessage::LaunchFinished(
                                    Err("No instance selected".to_string()),
                                ),
                            ));
                        };

                        if let Some(account) = active_account {
                            if account.requires_login {
                                self.stage = Stage::AccountSetup;
                                return iced::Task::none();
                            }

                            // Reuse the launch logic from InstancesScreen essentially
                            let active_account_store = self.account.clone_store();

                            iced::Task::perform(
                                async move {
                                    // 1. Get tokens
                                    let access_token =
                                        if let AccountKind::Microsoft { .. } = &account.kind {
                                            active_account_store
                                                .microsoft_tokens(&account.id)
                                                .ok()
                                                .flatten()
                                                .map(|s| s.access_token)
                                                .unwrap_or_default()
                                        } else {
                                            String::new()
                                        };

                                    // 2. Prepare Launch
                                    use directories::ProjectDirs;
                                    let dirs =
                                        ProjectDirs::from("com", "fastmc", "fastmc").unwrap();
                                    let instance_dir =
                                        dirs.data_local_dir().join("instances").join(&instance_id);
                                    let game_dir = instance_dir.join(".minecraft");
                                    let json_path = instance_dir.join("instance.json");

                                    // Load metadata
                                    let content =
                                        tokio::fs::read_to_string(&json_path).await.map_err(
                                            |e| format!("Failed to read instance config: {}", e),
                                        )?;
                                    let metadata: instance_manager::InstanceMetadata =
                                        serde_json::from_str(&content).map_err(|e| {
                                            format!("Invalid instance config: {}", e)
                                        })?;

                                    // Detect Java
                                    let config = FastmcConfig::load().unwrap_or_default();
                                    let java_settings =
                                        java_manager::JavaLaunchSettings::from(&config.java);
                                    let java_config = java_settings.detection_config();
                                    // Select Java based on version
                                    let target_version = "1.0"; // Hardcoded for testing legacy launch

                                    let summary = tokio::task::spawn_blocking(move || {
                                        java_manager::detect_installations(&java_config)
                                    })
                                    .await
                                    .map_err(|e| e.to_string())?;

                                    let java_path = summary
                                        .select_for_version(target_version)
                                        .map_err(|e| e.to_string())?;

                                    println!("Selected Java path: {:?}", java_path);

                                    let target_version = &metadata.game_version;

                                    let mut cmd = game::prepare_and_launch(
                                        &account,
                                        &access_token,
                                        java_path,
                                        game_dir,
                                        target_version,
                                    )
                                    .await?;

                                    let mut child = cmd
                                        .spawn()
                                        .map_err(|e| format!("Failed to start process: {}", e))?;

                                    // Wait for process to exit (blocking)
                                    tokio::task::spawn_blocking(move || {
                                        let _ = child.wait();
                                    })
                                    .await
                                    .map_err(|e| e.to_string())?;

                                    Ok(())
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
                }
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
            Message::InstancesScreen(instances_message) => {
                if let InstancesMessage::LaunchInstance(instance_id) = &instances_message {
                    let id = instance_id.clone();
                    let active_account = self.account.clone_store();

                    if let Some(account_id) = active_account.active {
                        let account = active_account
                            .accounts
                            .iter()
                            .find(|a| a.id == account_id)
                            .cloned();
                        if let Some(account) = account {
                            if account.requires_login {
                                self.stage = Stage::AccountSetup;
                                return iced::Task::none();
                            }

                            return iced::Task::perform(
                                async move {
                                    // 1. Get tokens (Async)
                                    let access_token =
                                        if let AccountKind::Microsoft { .. } = &account.kind {
                                            active_account
                                                .microsoft_tokens(&account.id)
                                                .ok()
                                                .flatten()
                                                .map(|s| s.access_token)
                                                .unwrap_or_default()
                                        } else {
                                            String::new()
                                        };

                                    // 2. Prepare Launch (Async)
                                    use directories::ProjectDirs;
                                    let dirs =
                                        ProjectDirs::from("com", "fastmc", "fastmc").unwrap();
                                    let instance_dir =
                                        dirs.data_local_dir().join("instances").join(&id);
                                    let game_dir = instance_dir.join(".minecraft");
                                    let json_path = instance_dir.join("instance.json");

                                    // Load metadata
                                    let content =
                                        tokio::fs::read_to_string(&json_path).await.map_err(
                                            |e| format!("Failed to read instance config: {}", e),
                                        )?;
                                    let metadata: instance_manager::InstanceMetadata =
                                        serde_json::from_str(&content).map_err(|e| {
                                            format!("Invalid instance config: {}", e)
                                        })?;

                                    // Detect Java (Blocking, but fast-ish, can wrap if needed)
                                    let config = FastmcConfig::load().unwrap_or_default();

                                    // Use settings to drive detection (respects user preference)
                                    let java_settings =
                                        java_manager::JavaLaunchSettings::from(&config.java);
                                    let java_config = java_settings.detection_config();

                                    let summary = tokio::task::spawn_blocking(move || {
                                        java_manager::detect_installations(&java_config)
                                    })
                                    .await
                                    .map_err(|e| e.to_string())?;

                                    let java_path =
                                        summary.select_for_version(&metadata.game_version)?;

                                    let mut cmd = game::prepare_and_launch(
                                        &account,
                                        &access_token,
                                        java_path,
                                        game_dir,
                                        &metadata.game_version,
                                    )
                                    .await?;

                                    let mut child = cmd
                                        .spawn()
                                        .map_err(|e| format!("Failed to spawn process: {}", e))?;

                                    // Wait for process to exit
                                    tokio::task::spawn_blocking(move || {
                                        let _ = child.wait();
                                    })
                                    .await
                                    .map_err(|e| e.to_string())?;

                                    Ok(())
                                },
                                |res| {
                                    Message::InstancesScreen(InstancesMessage::LaunchFinished(res))
                                },
                            );
                        } else {
                            return iced::Task::done(Message::InstancesScreen(
                                InstancesMessage::LaunchFinished(Err(
                                    "Active account not found".to_string()
                                )),
                            ));
                        }
                    } else {
                        return iced::Task::done(Message::InstancesScreen(
                            InstancesMessage::LaunchFinished(Err("No active account".to_string())),
                        ));
                    }
                }

                let task = self.instances.update(instances_message);
                task.map(Message::InstancesScreen)
            }
            Message::SettingsScreen(settings_message) => {
                self.settings.update(settings_message);
                iced::Task::none()
            }
            Message::MenuItemSelected(item) => {
                self.stage = Stage::Main;
                self.selected_menu = item;

                if item == MenuItem::Instances {
                    let task = self.instances.refresh();
                    return task.map(Message::InstancesScreen);
                }

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

                // Initial Refresh for Play Screen (Instances)
                let refresh_task = self.play.refresh().map(Message::PlayScreen);

                let validation_task = iced::Task::perform(
                    async move {
                        if let Some(cid) = client_id {
                            use account_manager::AccountService;
                            let mut service =
                                AccountService::new(cid).map_err(|e| e.to_string())?;
                            let account = service
                                .validate_active_account()
                                .await
                                .map_err(|e| e.to_string())?;
                            Ok(account.display_name.clone())
                        } else {
                            Ok("Offline/NoID".to_string())
                        }
                    },
                    Message::AccountValidated,
                );

                let assets_task =
                    iced::Task::perform(assets::AssetStore::load(), Message::AssetsLoaded);

                iced::Task::batch(vec![refresh_task, validation_task, assets_task])
            }
            Message::AssetsLoaded(store) => {
                self.assets = Some(store);

                // If we already have a validation result, we can try to transition
                if let Some(result) = self.validation_result.clone() {
                    return self.handle_startup_completion(result);
                }

                iced::Task::none()
            }
            Message::AccountValidated(result) => {
                self.validation_result = Some(result.clone());

                // Only switch if we have assets
                if self.assets.is_some() {
                    return self.handle_startup_completion(result);
                }

                iced::Task::none()
            }
        }
    }

    fn handle_startup_completion(&mut self, result: Result<String, String>) -> iced::Task<Message> {
        match result {
            Ok(_) => {
                self.stage = Stage::Main;
            }
            Err(e) => {
                println!("Account validation failed: {}", e);
                self.stage = Stage::AccountSetup;
                // Reload account screen
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

    fn view(&self) -> iced::Element<'_, Message> {
        match self.stage {
            Stage::Loading => self.loading.view().map(|_| Message::Startup), // Helper, actually message is ignored
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
            MenuItem::Play => self
                .play
                .view(self.assets.as_ref())
                .map(Message::PlayScreen),
            MenuItem::Server => self.server.view().map(Message::ServerScreen),
            MenuItem::Modpacks => self.modpacks.view().map(Message::ModpacksScreen),
            MenuItem::JavaManager => self.java_manager.view().map(Message::JavaManagerScreen),
            MenuItem::Instances => self.instances.view().map(Message::InstancesScreen),
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
            (
                MenuItem::Instances,
                "Instances",
                "assets/svg/package.svg", // Reusing package icon for now or use a new one
            ),
            (MenuItem::Settings, "Settings", "assets/svg/settings.svg"),
        ];

        let header: iced::Element<Message> = if let Some(store) = &self.assets {
            if let Some(handle) = store.get_image("wide_logo.png") {
                iced::widget::container(
                    iced::widget::image(handle)
                        .width(iced::Length::Fill)
                        .content_fit(iced::ContentFit::Contain),
                )
                .width(iced::Length::Fill)
                .align_x(iced::Alignment::Center)
                .into()
            } else {
                iced::widget::text("FastMC").size(24).into()
            }
        } else {
            iced::widget::text("FastMC").size(24).into()
        };

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
                // Try from store, fallback to path if needed (though store handles path internally currently)
                // We need to update icon_from_path to use store if possible.
                // For now, let's just stick to the old way for a second, then refactor the helper.
                // Actually, let's implement the store usage here.

                let icon = if let Some(store) = &self.assets {
                    // path is like "assets/svg/play.svg", we stored keys as "play.svg"
                    let filename = std::path::Path::new(path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    if let Some(handle) = store.get_icon(filename) {
                        iced::widget::svg(handle)
                            .width(iced::Length::Fixed(24.0))
                            .height(iced::Length::Fixed(24.0))
                            .into()
                    } else {
                        icon_from_path::<Message>(path)
                    }
                } else {
                    icon_from_path::<Message>(path)
                };

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
            .padding(iced::Padding {
                top: 10.0,
                right: 18.0,
                bottom: 20.0,
                left: 18.0,
            })
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

fn load_icon() -> Option<iced::window::Icon> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/favicon.png");
    let img = image_crate::open(path).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    let rgba = img.into_raw();
    iced::window::icon::from_rgba(rgba, width, height).ok()
}

pub fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("FastMC Launcher")
        .window(iced::window::Settings {
            icon: load_icon(),
            ..Default::default()
        })
        .theme(iced::Theme::Dracula)
        .subscription(|_| window::resize_events().map(|(_, size)| Message::Resized(size.width)))
        .run()
}
