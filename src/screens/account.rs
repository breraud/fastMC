use account_manager::{Account, AccountError, AccountKind, AccountService, AccountStore};
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Task};
use microsoft_auth::DeviceCodeInfo;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum Message {
    OfflineNameChanged(String),
    AddOffline,
    AddMicrosoft,
    MicrosoftCodeReady(Box<Result<DeviceCodeInfo, String>>),
    MicrosoftComplete,
    MicrosoftFinished(Box<Result<AccountStore, String>>),
    SelectAccount(Uuid),
    DeleteAccount(Uuid),
    BackToLauncher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountUpdate {
    None,
    EnterLauncher,
}

pub struct AccountScreen {
    store: AccountStore,
    offline_username: String,
    error: Option<String>,
    microsoft_client_id: Option<String>,
    device_code: Option<DeviceCodeInfo>,
    microsoft_in_progress: bool,
}

impl AccountScreen {
    pub fn new(microsoft_client_id: Option<String>) -> Self {
        let (mut store, error) = match AccountStore::load() {
            Ok(store) => (store, None),
            Err(err) => (AccountStore::default(), Some(err.to_string())),
        };

        if store.active.is_none()
            && let Some(first) = store.accounts.first()
        {
            store.active = Some(first.id);
            let _ = store.save();
        }

        Self {
            store,
            offline_username: String::new(),
            error,
            microsoft_client_id,
            device_code: None,
            microsoft_in_progress: false,
        }
    }

    pub fn has_accounts(&self) -> bool {
        !self.store.accounts.is_empty()
    }

    pub fn clone_store(&self) -> AccountStore {
        self.store.clone()
    }

    #[allow(dead_code)]
    pub fn get_microsoft_tokens(
        &self,
        account_id: &Uuid,
    ) -> Option<account_manager::MicrosoftSecrets> {
        self.store.microsoft_tokens(account_id).ok().flatten()
    }

    pub fn active_account(&self) -> Option<&Account> {
        self.store
            .active
            .and_then(|id| self.store.accounts.iter().find(|a| a.id == id))
            .or_else(|| self.store.accounts.first())
    }

    pub fn update(&mut self, message: Message) -> (AccountUpdate, Task<Message>) {
        match message {
            Message::OfflineNameChanged(name) => {
                self.offline_username = name;
                self.error = None;
                (AccountUpdate::None, Task::none())
            }
            Message::AddOffline => {
                let trimmed = self.offline_username.trim();
                if trimmed.is_empty() {
                    self.error = Some("Please enter a username.".to_string());
                    return (AccountUpdate::None, Task::none());
                }

                match self.store.add_offline(trimmed.to_string()) {
                    Ok(_) => {
                        self.offline_username.clear();
                        self.error = None;
                        (AccountUpdate::EnterLauncher, Task::none())
                    }
                    Err(err) => {
                        self.error = Some(err.to_string());
                        (AccountUpdate::None, Task::none())
                    }
                }
            }
            Message::AddMicrosoft => {
                if let Some(client_id) = &self.microsoft_client_id {
                    self.error = None;
                    self.microsoft_in_progress = true;
                    self.device_code = None;

                    let client_id = client_id.clone();
                    let task = Task::perform(
                        async move {
                            let service =
                                AccountService::new(client_id).map_err(|e| e.to_string())?;
                            service
                                .start_microsoft_device_code()
                                .await
                                .map_err(|e| e.to_string())
                        },
                        |result| Message::MicrosoftCodeReady(Box::new(result)),
                    );

                    (AccountUpdate::None, task)
                } else {
                    self.error = Some("Microsoft client id is not configured.".to_string());
                    (AccountUpdate::None, Task::none())
                }
            }
            Message::MicrosoftCodeReady(result) => {
                match *result {
                    Ok(code) => {
                        self.error = None;
                        self.device_code = Some(code);
                        // Trigger polling immediately
                        return (
                            AccountUpdate::None,
                            Task::perform(async {}, |_| Message::MicrosoftComplete),
                        );
                    }
                    Err(err) => {
                        self.microsoft_in_progress = false;
                        self.error = Some(err);
                        self.device_code = None;
                    }
                }

                (AccountUpdate::None, Task::none())
            }
            Message::MicrosoftComplete => {
                if self.device_code.is_none() {
                    self.error = Some("Start Microsoft login first.".to_string());
                    return (AccountUpdate::None, Task::none());
                }

                let code = self.device_code.clone().expect("checked above");
                let client_id = match &self.microsoft_client_id {
                    Some(id) => id.clone(),
                    None => {
                        self.error = Some("Microsoft client id is not configured.".to_string());
                        return (AccountUpdate::None, Task::none());
                    }
                };

                self.microsoft_in_progress = true;
                let task = Task::perform(
                    async move {
                        let mut service =
                            AccountService::new(client_id).map_err(|e| e.to_string())?;
                        service
                            .complete_microsoft_login(&code)
                            .await
                            .map_err(|e| e.to_string())?;
                        AccountStore::load().map_err(|e| e.to_string())
                    },
                    |result| Message::MicrosoftFinished(Box::new(result)),
                );

                (AccountUpdate::None, task)
            }
            Message::MicrosoftFinished(result) => {
                self.microsoft_in_progress = false;
                match *result {
                    Ok(store) => {
                        self.store = store;
                        self.device_code = None;
                        self.error = None;
                        (AccountUpdate::EnterLauncher, Task::none())
                    }
                    Err(err) => {
                        self.error = Some(err);
                        (AccountUpdate::None, Task::none())
                    }
                }
            }
            Message::SelectAccount(id) => match self.set_active(id) {
                Ok(_) => (AccountUpdate::EnterLauncher, Task::none()),
                Err(err) => {
                    self.error = Some(err.to_string());
                    (AccountUpdate::None, Task::none())
                }
            },
            Message::DeleteAccount(id) => match self.remove_account(id) {
                Ok(_) => {
                    self.error = None;
                    (AccountUpdate::None, Task::none())
                }
                Err(err) => {
                    self.error = Some(err.to_string());
                    (AccountUpdate::None, Task::none())
                }
            },
            Message::BackToLauncher => {
                if self.has_accounts() {
                    (AccountUpdate::EnterLauncher, Task::none())
                } else {
                    self.error = Some("Add an account before entering the launcher.".to_string());
                    (AccountUpdate::None, Task::none())
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let background = Color::from_rgb(0.12, 0.12, 0.14);
        let text_primary = Color::from_rgb(0.88, 0.89, 0.91);
        let text_muted = Color::from_rgb(0.63, 0.64, 0.67);
        let accent = Color::from_rgb(0.13, 0.77, 0.36);
        let surface = Color::from_rgb(0.14, 0.14, 0.17);

        let heading = text("Accounts")
            .size(28)
            .style(move |_| iced::widget::text::Style {
                color: Some(text_primary),
            });

        let description =
            text("Select an account to use with the launcher or add a new one below.")
                .size(16)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_muted),
                });

        let input_row = text_input("Enter offline username", &self.offline_username)
            .on_input(Message::OfflineNameChanged)
            .padding([12, 14])
            .size(16)
            .width(Length::Fill);

        let add_offline = button(
            text("Add offline").style(move |_| iced::widget::text::Style {
                color: Some(Color::WHITE),
            }),
        )
        .padding([12, 18])
        .style(move |_theme, status| {
            let hover = Color::from_rgb(0.12, 0.61, 0.30);
            iced::widget::button::Style {
                background: Some(
                    match status {
                        iced::widget::button::Status::Hovered
                        | iced::widget::button::Status::Pressed => hover,
                        _ => accent,
                    }
                    .into(),
                ),
                text_color: Color::WHITE,
                border: iced::Border {
                    radius: 12.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(Message::AddOffline);

        let add_microsoft =
            button(
                text("Add Microsoft").style(move |_| iced::widget::text::Style {
                    color: Some(Color::WHITE),
                }),
            )
            .padding([12, 18])
            .style(move |_theme, status| {
                let base = Color::from_rgb(0.23, 0.47, 0.91);
                let hover = Color::from_rgb(0.26, 0.52, 1.0);
                iced::widget::button::Style {
                    background: Some(
                        match status {
                            iced::widget::button::Status::Hovered
                            | iced::widget::button::Status::Pressed => hover,
                            _ => base,
                        }
                        .into(),
                    ),
                    text_color: Color::WHITE,
                    border: iced::Border {
                        radius: 12.0.into(),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            })
            .on_press(Message::AddMicrosoft);

        let microsoft_box: Element<'_, Message> = if let Some(code) = &self.device_code {
            container(
                column![
                    text("Waiting for your login...").size(18).style(move |_| {
                        iced::widget::text::Style {
                            color: Some(text_primary),
                        }
                    }),
                    text(format!(
                        "Use code {} at {}",
                        code.user_code, code.verification_uri
                    ))
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_muted),
                    }),
                    if let Some(full) = &code.verification_uri_complete {
                        text(full)
                            .size(14)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(text_muted),
                            })
                    } else {
                        text("").style(move |_| iced::widget::text::Style {
                            color: Some(text_muted),
                        })
                    },
                    text("The launcher will automatically connect once you finish.")
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(text_muted),
                        }),
                ]
                .spacing(10),
            )
            .padding(16)
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(surface.into()),
                border: iced::Border {
                    radius: 12.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            })
            .into()
        } else if self.microsoft_in_progress {
            container(text("Starting Microsoft login...").style(move |_| {
                iced::widget::text::Style {
                    color: Some(text_muted),
                }
            }))
            .padding(12)
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(surface.into()),
                border: iced::Border {
                    radius: 12.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            })
            .into()
        } else {
            container(iced::widget::Space::new()).into()
        };

        let error_banner = self.error.as_ref().map(|err| {
            container(
                text(err)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color::from_rgb(0.96, 0.47, 0.47)),
                    })
                    .size(14),
            )
            .padding([10, 12])
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(Color::from_rgb(0.24, 0.12, 0.12).into()),
                border: iced::Border {
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            })
        });

        let accounts_list: Element<'_, Message> = if self.store.accounts.is_empty() {
            column![
                text("No account yet").size(24).style(move |_| iced::widget::text::Style {
                    color: Some(text_primary),
                }),
                text("Add a Microsoft or offline account to start downloading and launching Minecraft.")
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_muted),
                    }),
            ]
            .align_x(Alignment::Center)
            .spacing(8)
            .padding(20)
            .into()
        } else {
            let header =
                text("Available accounts")
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_muted),
                    });

            let accounts = self.store.accounts.iter().fold(column![], |col, account| {
                col.push(self.account_row(account, text_primary, text_muted, surface))
            });

            let list = accounts.spacing(12);

            column![header, list].spacing(12).into()
        };

        let mut content = column![heading, description, accounts_list, microsoft_box]
            .spacing(20)
            .align_x(Alignment::Center)
            .max_width(680);

        if let Some(error) = error_banner {
            content = content.push(error);
        }

        let back_button =
            button(
                text("Back to launcher").style(move |_| iced::widget::text::Style {
                    color: Some(text_primary),
                }),
            )
            .padding([12, 18])
            .style(move |_theme, status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => Color::from_rgb(0.20, 0.20, 0.23),
                    _ => surface,
                };
                iced::widget::button::Style {
                    background: Some(bg.into()),
                    text_color: text_primary,
                    border: iced::Border {
                        radius: 12.0.into(),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            })
            .on_press(Message::BackToLauncher);

        let footer = column![
            input_row,
            row![add_offline, add_microsoft].spacing(12),
            back_button
        ]
        .spacing(20)
        .align_x(Alignment::Center)
        .max_width(680);

        let style_scroll = |_theme: &iced::Theme, status: scrollable::Status| {
            let accent = Color::from_rgb(0.13, 0.77, 0.36);
            let (_rail_bg, scroller_bg) = match status {
                scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => (
                    Option::<Background>::None,
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.45),
                ),
                _ => (
                    Option::<Background>::None,
                    Color::from_rgba(0.82, 0.84, 0.87, 0.18),
                ),
            };

            iced::widget::scrollable::Style {
                container: iced::widget::container::Style::default(),
                vertical_rail: iced::widget::scrollable::Rail {
                    background: None,
                    border: Border {
                        radius: 5.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    scroller: iced::widget::scrollable::Scroller {
                        background: Background::Color(scroller_bg),
                        border: Border {
                            radius: 8.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                    },
                },
                horizontal_rail: iced::widget::scrollable::Rail {
                    background: None,
                    border: Border::default(),
                    scroller: iced::widget::scrollable::Scroller {
                        background: Background::Color(Color::TRANSPARENT),
                        border: Border::default(),
                    },
                },
                gap: None,
                auto_scroll: iced::widget::scrollable::AutoScroll {
                    background: Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65)),
                    border: Border::default(),
                    shadow: Shadow::default(),
                    icon: Color::WHITE,
                },
            }
        };

        // Constrain the scrolling area to strictly fit the content width + padding.
        // using Fixed width ensures the scrollbar is exactly where we want it.
        // Inner container fills height and centers content vertically.
        let scrollable_area = scrollable(
            container(content)
                .padding(10)
                .align_x(Alignment::Center)
                .height(Length::Fill)
                .align_y(Alignment::Center),
        )
        .height(Length::Fill)
        .width(Length::Fixed(720.0))
        .style(style_scroll);

        let layout = column![
            container(scrollable_area)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center),
            container(footer)
                .width(Length::Fill)
                .align_x(Alignment::Center)
        ]
        .spacing(10)
        .align_x(Alignment::Center);

        container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .padding(24)
            .style(move |_| iced::widget::container::Style {
                background: Some(background.into()),
                ..iced::widget::container::Style::default()
            })
            .into()
    }

    fn account_row<'a>(
        &'a self,
        account: &'a Account,
        text_primary: Color,
        text_muted: Color,
        surface: Color,
    ) -> Element<'a, Message> {
        let is_active = self.store.active == Some(account.id);
        let badge_text = account
            .display_name
            .chars()
            .next()
            .unwrap_or('A')
            .to_string();

        let badge =
            container(
                text(badge_text)
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_primary),
                    }),
            )
            .width(Length::Fixed(42.0))
            .height(Length::Fixed(42.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| iced::widget::container::Style {
                background: Some(Color::from_rgb(0.18, 0.18, 0.21).into()),
                border: iced::Border {
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            });

        let subtitle = match &account.kind {
            AccountKind::Microsoft { username, .. } => format!("Microsoft • {username}"),
            AccountKind::Offline { username, .. } => format!("Offline • {username}"),
        };

        let details = column![
            text(&account.display_name)
                .size(18)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_primary),
                }),
            text(subtitle)
                .size(14)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_muted),
                }),
        ]
        .spacing(4);

        let select_button = if account.requires_login {
            // Need to clone badge and details for use here since they were created above
            // Actually, we can just rebuild the row or clone the content if easier.
            // Since Element isn't clone, we have to reconstruct the widgets or wrap them in a function.
            // But wait, `badge` and `details` are consumed by the else branch or this branch.
            // So we can just reuse them in both branches if we move the creation down or conditionally build the button content.

            // Let's reuse the badge/details logic.
            // We can just construct the row here.

            button(
                row![
                    badge,
                    details,
                    iced::widget::Space::new().width(Length::Fill),
                    container(text("Re-login").size(14).style(move |_| {
                        iced::widget::text::Style {
                            color: Some(Color::WHITE),
                        }
                    }))
                    .padding([6, 12])
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Color::from_rgb(0.8, 0.4, 0.0).into()),
                        border: iced::Border {
                            radius: 20.0.into(),
                            ..iced::Border::default()
                        },
                        ..iced::widget::container::Style::default()
                    })
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([12, 14])
            .width(Length::Fill)
            .style(move |_theme, status| {
                // Use standard surface colors for the row background,
                // so the orange button stands out as the ACTION.
                let base = surface;
                let hover = Color::from_rgb(0.20, 0.20, 0.23);
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
                        radius: 12.0.into(),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            })
            // Re-login just triggers the AddMicrosoft flow;
            // since we handle upsert, it will update the existing account entry by UUID match.
            .on_press(Message::AddMicrosoft)
        } else {
            button(row![badge, details].spacing(12).align_y(Alignment::Center))
                .padding([12, 14])
                .width(Length::Fill)
                .style(move |_theme, status| {
                    let base = if is_active {
                        Color::from_rgb(0.15, 0.27, 0.20)
                    } else {
                        surface
                    };
                    let hover = if is_active {
                        Color::from_rgb(0.16, 0.31, 0.22)
                    } else {
                        Color::from_rgb(0.20, 0.20, 0.23)
                    };
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
                            radius: 12.0.into(),
                            ..iced::Border::default()
                        },
                        ..iced::widget::button::Style::default()
                    }
                })
                .on_press(Message::SelectAccount(account.id))
        };

        let delete_button = button(text("Delete").style(move |_| iced::widget::text::Style {
            color: Some(Color::from_rgb(0.96, 0.47, 0.47)),
        }))
        .padding([10, 14])
        .style(move |_theme, status| {
            let base = Color::from_rgb(0.24, 0.12, 0.12);
            let hover = Color::from_rgb(0.28, 0.14, 0.14);
            iced::widget::button::Style {
                background: Some(
                    match status {
                        iced::widget::button::Status::Hovered
                        | iced::widget::button::Status::Pressed => hover,
                        _ => base,
                    }
                    .into(),
                ),
                text_color: Color::from_rgb(0.96, 0.47, 0.47),
                border: iced::Border {
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(Message::DeleteAccount(account.id));

        row![select_button, delete_button]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
    }

    fn set_active(&mut self, account_id: Uuid) -> Result<(), AccountError> {
        if self
            .store
            .accounts
            .iter()
            .any(|account| account.id == account_id)
        {
            self.store.active = Some(account_id);
            self.store.save()?;
        }
        Ok(())
    }

    fn remove_account(&mut self, account_id: Uuid) -> Result<(), AccountError> {
        if let Some(pos) = self
            .store
            .accounts
            .iter()
            .position(|account| account.id == account_id)
        {
            if matches!(self.store.accounts[pos].kind, AccountKind::Microsoft { .. }) {
                self.store.clear_microsoft_tokens(&account_id)?;
            }

            self.store.accounts.remove(pos);
            if self.store.active == Some(account_id) {
                self.store.active = self.store.accounts.first().map(|account| account.id);
            }

            self.store.save()?;
        }

        Ok(())
    }
}
