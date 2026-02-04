use crate::instance_manager::{InstanceManager, InstanceMetadata};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Color, Element, Length, Task};

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    Loaded(Vec<InstanceMetadata>),
    SelectInstance(String),
    Launch,
    LaunchInstance(String),
    LaunchStarted,
    LaunchFinished(Result<(), String>),
    OpenSettings(String), // Instance ID
}

pub struct PlayScreen {
    instances: Vec<InstanceMetadata>,
    manager: InstanceManager,
    active_instance_id: Option<String>,
    is_launching: bool,
    error: Option<String>,
}

impl Default for PlayScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayScreen {
    pub fn new() -> Self {
        let manager = InstanceManager::new();
        // Ensure directory exists
        let _ = manager.init();

        Self {
            instances: Vec::new(),
            manager,
            active_instance_id: None,
            is_launching: false,
            error: None,
        }
    }

    pub fn refresh(&self) -> Task<Message> {
        let manager = self.manager.clone();
        Task::perform(async move { manager.list_instances() }, Message::Loaded)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Refresh => self.refresh(),
            Message::Loaded(instances) => {
                self.instances = instances;
                // If no active instance, select the first one or one marked as "last played" (future)
                if self.active_instance_id.is_none() && !self.instances.is_empty() {
                    self.active_instance_id = Some(self.instances[0].id.clone());
                }
                Task::none()
            }
            Message::SelectInstance(id) => {
                self.active_instance_id = Some(id);
                Task::none()
            }
            Message::Launch => {
                if let Some(_id) = &self.active_instance_id {
                    self.is_launching = true;
                    self.error = None;
                    Task::done(Message::LaunchStarted)
                } else {
                    Task::none()
                }
            }
            Message::LaunchInstance(id) => {
                self.active_instance_id = Some(id);
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
            Message::OpenSettings(_id) => {
                // Placeholder for now
                Task::none()
            }
        }
    }

    pub fn active_instance(&self) -> Option<&InstanceMetadata> {
        self.active_instance_id
            .as_ref()
            .and_then(|id| self.instances.iter().find(|i| &i.id == id))
    }

    pub fn view(&self, assets: Option<&crate::assets::AssetStore>) -> Element<'_, Message> {
        let hero_section = self.view_hero(assets);
        let profiles_list = self.view_profiles_list(assets);

        let mut content = column![hero_section, profiles_list]
            .spacing(20)
            .width(Length::Fill)
            .padding(iced::Padding {
                top: 0.0,
                right: 20.0,
                bottom: 20.0,
                left: 20.0,
            }); // Top padding 0 to merge with header if needed, or uniform

        if let Some(error) = &self.error {
            content = content.push(
                container(
                    text(format!("Error: {}", error)).color(Color::from_rgb(0.96, 0.47, 0.47)),
                )
                .padding(10)
                .style(|_| iced::widget::container::Style {
                    background: Some(Color::from_rgb(0.2, 0.1, 0.1).into()),
                    border: iced::Border {
                        radius: 8.0.into(),
                        ..iced::Border::default()
                    },
                    ..iced::widget::container::Style::default()
                }),
            );
        }

        content.into()
    }

    fn view_hero(&self, assets: Option<&crate::assets::AssetStore>) -> Element<'_, Message> {
        let (name, version, _last_played) = if let Some(instance) = self.active_instance() {
            (
                instance.name.clone(),
                format!("{} • {:?}", instance.game_version, instance.loader),
                "Last played: Never", // Placeholder
            )
        } else {
            (
                "No Profile Selected".to_string(),
                "Create a profile to play".to_string(),
                "",
            )
        };

        // Hero Content (Text & Buttons)
        let status_badge = container(text("Ready to Play").size(12).color(Color::WHITE))
            .padding([4, 8])
            .style(|_| container::Style {
                background: Some(Color::from_rgb(0.13, 0.77, 0.36).into()), // Green
                border: iced::Border {
                    radius: 4.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            });

        let title = text(name).size(42).style(|_| iced::widget::text::Style {
            color: Some(Color::WHITE),
        });

        let subtitle = text(version).size(14).style(|_| iced::widget::text::Style {
            color: Some(Color::from_rgb(0.9, 0.9, 0.9)),
        });

        // Launch Button
        let launch_btn = button(
            row![
                text(if self.is_launching {
                    "Playing"
                } else {
                    "Launch Game"
                })
                .size(16)
                .color(Color::WHITE)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([12, 24])
        .on_press_maybe(if self.is_launching || self.active_instance().is_none() {
            None
        } else {
            Some(Message::Launch)
        })
        .style(|_theme, status| {
            let base = Color::from_rgb(0.13, 0.77, 0.36);
            let hover = Color::from_rgb(0.15, 0.85, 0.40);
            let disabled = Color::from_rgb(0.3, 0.3, 0.3);

            let bg = match status {
                _ if self.is_launching => disabled,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed => {
                    hover
                }
                _ => base,
            };

            iced::widget::button::Style {
                background: Some(bg.into()),
                border: iced::Border {
                    radius: 6.0.into(),
                    ..iced::Border::default()
                },
                text_color: Color::WHITE,
                ..iced::widget::button::Style::default()
            }
        });

        // Change Profile Button (Placeholder logic for now)
        let profile_btn = button(text("Edit Profile").size(14))
            .padding([12, 20])
            .style(iced::widget::button::secondary);

        let actions = row![launch_btn, profile_btn].spacing(12);

        let hero_content = column![
            status_badge,
            title,
            subtitle,
            iced::widget::Space::new().height(20),
            actions
        ]
        .spacing(10)
        .padding(40);

        // Background Image
        let bg_image = if let Some(assets) = assets {
            if let Some(handle) = assets.get_image("instances_images/default.jpg") {
                iced::widget::image(handle)
            } else {
                iced::widget::image("assets/instances_images/default.jpg")
            }
        } else {
            iced::widget::image("assets/instances_images/default.jpg")
        }
        .content_fit(iced::ContentFit::Cover)
        .width(Length::Fill)
        .height(Length::Fill)
        .border_radius(16.0);

        // Gradient Overlay (No radius, relies on parent clip)
        let overlay = container(hero_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
                ..container::Style::default()
            });

        // Parent Stack Container with Clipping
        container(iced::widget::stack![bg_image, overlay])
            .width(Length::Fill)
            .height(Length::Fixed(300.0))
            .clip(true)
            .style(|_| container::Style {
                background: Some(Color::from_rgb(0.08, 0.08, 0.08).into()),
                border: iced::Border {
                    radius: 16.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into()
    }

    fn view_profiles_list(
        &self,
        assets: Option<&crate::assets::AssetStore>,
    ) -> Element<'_, Message> {
        let header = row![
            text("Your Profiles").size(18).color(Color::WHITE),
            iced::widget::Space::new().width(Length::Fill),
            button(text("+ New Profile").size(14))
                .padding([8, 16])
                .style(|_theme, _status| {
                    let base = iced::widget::button::Style::default();
                    iced::widget::button::Style {
                        background: Some(iced::Color::TRANSPARENT.into()),
                        border: iced::Border {
                            color: Color::from_rgb(0.5, 0.5, 0.5),
                            width: 1.0,
                            radius: 12.0.into(),
                        },
                        text_color: Color::WHITE,
                        ..base
                    }
                })
                .on_press(Message::OpenSettings("new".to_string()))
        ]
        .align_y(Alignment::Center);

        let list: Element<'_, Message> = if self.instances.is_empty() {
            Element::from(
                container(
                    text("No profiles found. Create one to start playing!")
                        .color(Color::from_rgb(0.5, 0.5, 0.5)),
                )
                .width(Length::Fill)
                .padding(20)
                .center_x(Length::Fill),
            )
        } else {
            Element::from(
                column(
                    self.instances
                        .iter()
                        .map(|inst| {
                            let is_active = self.active_instance_id.as_deref() == Some(&inst.id);

                            let icon_widget = if let Some(assets) = assets {
                                // TODO: Use instance specific icon if available
                                if let Some(handle) =
                                    assets.get_image("instances_images/default.jpg")
                                {
                                    iced::widget::image(handle)
                                } else {
                                    iced::widget::image("assets/instances_images/default.jpg")
                                }
                            } else {
                                iced::widget::image("assets/instances_images/default.jpg")
                            };

                            let icon_placeholder = container(
                                icon_widget
                                    .content_fit(iced::ContentFit::Cover)
                                    .width(Length::Fill)
                                    .height(Length::Fill)
                                    .border_radius(12.0),
                            )
                            .width(Length::Fixed(48.0))
                            .height(Length::Fixed(48.0))
                            // .clip(true) // Removed as image handles radius
                            .style(|_| container::Style {
                                // background: Some(Color::from_rgb(0.2, 0.2, 0.2).into()),
                                border: iced::Border {
                                    radius: 12.0.into(),
                                    ..iced::Border::default()
                                },
                                ..container::Style::default()
                            });

                            let info = column![
                                text(&inst.name).size(16).color(Color::WHITE),
                                text(format!("{} • {:?}", inst.game_version, inst.loader))
                                    .size(12)
                                    .color(Color::from_rgb(0.6, 0.6, 0.6))
                            ]
                            .spacing(4);

                            let (btn_text, btn_action) = if self.is_launching {
                                if is_active {
                                    ("Playing", None)
                                } else {
                                    ("▶", None)
                                }
                            } else {
                                ("▶", Some(Message::LaunchInstance(inst.id.clone())))
                            };

                            let is_launching = self.is_launching;

                            let play_btn = button(text(btn_text).size(14).color(Color::WHITE))
                                .padding(10)
                                .on_press_maybe(btn_action)
                                .style(move |_theme, status| {
                                    let base = Color::from_rgb(0.13, 0.77, 0.36);
                                    let hover = Color::from_rgb(0.15, 0.85, 0.40);
                                    let disabled = Color::from_rgb(0.3, 0.3, 0.3);

                                    iced::widget::button::Style {
                                        background: Some(
                                            match status {
                                                iced::widget::button::Status::Hovered => hover,
                                                _ => {
                                                    if is_launching && !is_active {
                                                        disabled
                                                    } else {
                                                        base
                                                    }
                                                }
                                            }
                                            .into(),
                                        ),
                                        border: iced::Border {
                                            radius: 20.0.into(), // Round circleish
                                            ..iced::Border::default()
                                        },
                                        ..iced::widget::button::Style::default()
                                    }
                                });

                            // Settings cog
                            let settings_btn =
                                button(text("⚙").size(14).color(Color::from_rgb(0.7, 0.7, 0.7)))
                                    .padding(10)
                                    .style(iced::widget::button::text)
                                    .on_press(Message::OpenSettings(inst.id.clone()));

                            let content = row![
                                icon_placeholder,
                                info,
                                iced::widget::Space::new().width(Length::Fill),
                                settings_btn,
                                play_btn
                            ]
                            .spacing(16)
                            .align_y(Alignment::Center);

                            let bg_color = if is_active {
                                Color::from_rgb(0.1, 0.3, 0.15)
                            } else {
                                Color::from_rgb(0.11, 0.11, 0.12)
                            };

                            button(content)
                                .on_press_maybe(if is_launching {
                                    None
                                } else {
                                    Some(Message::SelectInstance(inst.id.clone()))
                                })
                                .padding(12)
                                .width(Length::Fill)
                                .style(move |_theme, status| {
                                    let bg = match status {
                                        iced::widget::button::Status::Hovered => {
                                            Color::from_rgb(0.15, 0.15, 0.16)
                                        }
                                        _ => bg_color,
                                    };

                                    // If launching and not active, dim it
                                    let border_color = if is_active {
                                        Color::from_rgb(0.13, 0.77, 0.36)
                                    } else {
                                        Color::TRANSPARENT
                                    };

                                    let opacity_factor =
                                        if is_launching && !is_active { 0.5 } else { 1.0 };

                                    let mut style = iced::widget::button::Style {
                                        background: Some(bg.into()),
                                        border: iced::Border {
                                            radius: 12.0.into(),
                                            width: if is_active { 1.0 } else { 0.0 },
                                            color: border_color,
                                            ..iced::Border::default()
                                        },
                                        text_color: Color {
                                            a: opacity_factor,
                                            ..Color::WHITE
                                        },
                                        ..iced::widget::button::Style::default()
                                    };

                                    // Apply opacity to background if possible, or just imply it by logic
                                    if is_launching && !is_active {
                                        if let Some(iced::Background::Color(c)) = style.background {
                                            style.background =
                                                Some(iced::Background::Color(Color {
                                                    a: 0.5,
                                                    ..c
                                                }));
                                        }
                                    }

                                    style
                                })
                                .into() // Cast button to Element
                        })
                        .collect::<Vec<_>>(),
                )
                .spacing(8)
                .padding(iced::Padding {
                    right: 20.0,
                    ..iced::Padding::default()
                }),
            )
            .into()
        };

        column![
            header,
            scrollable(list)
                .height(Length::Fill)
                .style(move |_theme, status| {
                    let accent = Color::from_rgb(0.13, 0.77, 0.36);
                    let (rail_bg, scroller_bg) = match status {
                        scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => {
                            (None, Color::from_rgba(accent.r, accent.g, accent.b, 0.45))
                        }
                        _ => (None, Color::from_rgba(0.82, 0.84, 0.87, 0.18)),
                    };

                    iced::widget::scrollable::Style {
                        container: iced::widget::container::Style::default(),
                        vertical_rail: iced::widget::scrollable::Rail {
                            background: rail_bg.map(iced::Background::Color),
                            border: iced::Border {
                                radius: 6.0.into(),
                                width: 0.0,
                                color: Color::TRANSPARENT,
                            },
                            scroller: iced::widget::scrollable::Scroller {
                                background: iced::Background::Color(scroller_bg),
                                border: iced::Border {
                                    radius: 8.0.into(),
                                    width: 0.0,
                                    color: Color::TRANSPARENT,
                                },
                            },
                        },
                        horizontal_rail: iced::widget::scrollable::Rail {
                            background: None,
                            border: iced::Border::default(),
                            scroller: iced::widget::scrollable::Scroller {
                                background: iced::Background::Color(Color::TRANSPARENT),
                                border: iced::Border::default(),
                            },
                        },
                        gap: None,
                        auto_scroll: iced::widget::scrollable::AutoScroll {
                            background: iced::Background::Color(Color::from_rgba(
                                0.0, 0.0, 0.0, 0.65,
                            )),
                            border: iced::Border::default(),
                            shadow: iced::Shadow::default(),
                            icon: Color::WHITE,
                        },
                    }
                })
        ]
        .spacing(16)
        .height(Length::Fill)
        .into()
    }
}
