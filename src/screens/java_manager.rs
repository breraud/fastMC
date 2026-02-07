use std::fs;
use std::path::PathBuf;

use config_manager::{FastmcConfig, JavaInstallationRecord};
use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, slider, text, text_editor,
    text_input,
};
use iced::{Alignment, Color, Element, Length, Task};
use java_manager::{
    DetectionSummary, InstallSource, JavaDetectionConfig, JavaInstallation, JavaLaunchSettings,
    detect_installations,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::instance_manager::{InstanceManager, InstanceMetadata};

const MIN_MEMORY_BOUND: u32 = 512;
const MAX_MEMORY_BOUND: u32 = 16384;

#[derive(Debug, Clone, PartialEq)]
pub enum JavaTarget {
    Global,
    Instance(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetOption {
    pub target: JavaTarget,
    pub display_name: String,
}

impl std::fmt::Display for TargetOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name)
    }
}

#[derive(Debug, Clone)]
pub enum OverrideField {
    JavaPath,
    MinMemory,
    MaxMemory,
    JvmArgs,
}

#[derive(Debug, Clone)]
pub enum Message {
    DetectJava,
    DetectionFinished(DetectionSummary),
    Tick,
    Resized(f32),
    ClearStatus(Instant),
    SelectInstallation(Uuid),
    RemoveInstallation(Uuid),
    ToggleCustomForm,
    MinMemoryChanged(f32),
    MaxMemoryChanged(f32),
    ExtraArgsEdited(text_editor::Action),
    SaveArgs,
    CustomPathChanged(String),
    BrowseForJava,
    BrowseFinished(Option<PathBuf>),
    UseCustomPath,
    // Instance-awareness messages
    TargetSelected(TargetOption),
    InstancesLoaded(Vec<InstanceMetadata>),
    ScopeToInstance(String, String),
    ClearOverride(OverrideField),
}

pub struct JavaManagerScreen {
    installations: Vec<JavaInstallation>,
    settings: JavaLaunchSettings,
    detection_in_progress: bool,
    detection_errors: Vec<String>,
    args_content: text_editor::Content,
    custom_path_input: String,
    show_custom_form: bool,
    is_wide: bool,
    status: Option<(String, Color, Instant)>,
    // Instance-awareness
    target: JavaTarget,
    available_targets: Vec<TargetOption>,
    global_settings: JavaLaunchSettings,
    instance_metadata: Option<InstanceMetadata>,
    instance_manager: InstanceManager,
}

impl Default for JavaManagerScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaManagerScreen {
    pub fn new() -> Self {
        let config = FastmcConfig::load().unwrap_or_default();
        let settings = JavaLaunchSettings::from(&config.java);
        let global_settings = settings.clone();
        let args_input = settings.extra_jvm_args.join(" ");
        let args_content = text_editor::Content::with_text(&args_input);
        let custom_path_input = settings
            .java_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let instance_manager = InstanceManager::new();
        let mut installations = map_records_to_installations(&settings.detected_installations);

        let available_targets = vec![TargetOption {
            target: JavaTarget::Global,
            display_name: "Global (Default)".to_string(),
        }];

        let mut screen = Self {
            installations: Vec::new(),
            settings,
            detection_in_progress: false,
            detection_errors: Vec::new(),
            args_content,
            custom_path_input,
            show_custom_form: false,
            is_wide: false,
            status: None,
            target: JavaTarget::Global,
            available_targets,
            global_settings,
            instance_metadata: None,
            instance_manager,
        };
        screen.installations.append(&mut installations);
        screen.ensure_selected_entry();
        screen
    }

    fn load_for_target(&mut self) {
        let config = FastmcConfig::load().unwrap_or_default();
        self.global_settings = JavaLaunchSettings::from(&config.java);

        match &self.target {
            JavaTarget::Global => {
                self.settings = self.global_settings.clone();
                self.instance_metadata = None;
                self.installations =
                    map_records_to_installations(&self.settings.detected_installations);
                self.ensure_selected_entry();
            }
            JavaTarget::Instance(id) => {
                if let Ok(meta) = self.instance_manager.load_instance(id) {
                    self.settings = JavaLaunchSettings {
                        java_path: meta
                            .java_path
                            .as_ref()
                            .map(PathBuf::from)
                            .or_else(|| self.global_settings.java_path.clone()),
                        auto_discover: meta
                            .auto_discover
                            .unwrap_or(self.global_settings.auto_discover),
                        min_memory_mb: meta
                            .min_memory_mb
                            .unwrap_or(self.global_settings.min_memory_mb),
                        max_memory_mb: meta
                            .max_memory_mb
                            .unwrap_or(self.global_settings.max_memory_mb),
                        extra_jvm_args: meta
                            .jvm_args
                            .clone()
                            .unwrap_or_else(|| self.global_settings.extra_jvm_args.clone()),
                        detected_installations: self
                            .global_settings
                            .detected_installations
                            .clone(),
                    };
                    self.instance_metadata = Some(meta);
                    self.installations =
                        map_records_to_installations(&self.settings.detected_installations);
                    self.ensure_selected_entry();
                }
            }
        }

        let args_input = self.settings.extra_jvm_args.join(" ");
        self.args_content = text_editor::Content::with_text(&args_input);
        self.custom_path_input = self
            .settings
            .java_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
    }

    fn rebuild_target_options(&mut self, instances: &[InstanceMetadata]) {
        let mut options = vec![TargetOption {
            target: JavaTarget::Global,
            display_name: "Global (Default)".to_string(),
        }];
        for inst in instances {
            options.push(TargetOption {
                target: JavaTarget::Instance(inst.id.clone()),
                display_name: format!("{} ({})", inst.name, inst.game_version),
            });
        }
        self.available_targets = options;
    }

    fn current_target_option(&self) -> TargetOption {
        self.available_targets
            .iter()
            .find(|opt| opt.target == self.target)
            .cloned()
            .unwrap_or(TargetOption {
                target: JavaTarget::Global,
                display_name: "Global (Default)".to_string(),
            })
    }

    fn is_field_overridden(&self, field: &OverrideField) -> bool {
        if matches!(self.target, JavaTarget::Global) {
            return false;
        }
        match (&self.instance_metadata, field) {
            (Some(meta), OverrideField::JavaPath) => meta.java_path.is_some(),
            (Some(meta), OverrideField::MinMemory) => meta.min_memory_mb.is_some(),
            (Some(meta), OverrideField::MaxMemory) => meta.max_memory_mb.is_some(),
            (Some(meta), OverrideField::JvmArgs) => meta.jvm_args.is_some(),
            _ => false,
        }
    }

    fn mark_field_overridden(&mut self, field: &OverrideField) {
        if let Some(meta) = &mut self.instance_metadata {
            match field {
                OverrideField::JavaPath => {
                    meta.java_path = self
                        .settings
                        .java_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned());
                }
                OverrideField::MinMemory => {
                    meta.min_memory_mb = Some(self.settings.min_memory_mb);
                }
                OverrideField::MaxMemory => {
                    meta.max_memory_mb = Some(self.settings.max_memory_mb);
                }
                OverrideField::JvmArgs => {
                    meta.jvm_args = Some(self.settings.extra_jvm_args.clone());
                }
            }
        }
    }

    fn inherited_indicator<'a>(
        &self,
        field: OverrideField,
        is_overridden: bool,
    ) -> Element<'a, Message> {
        if matches!(self.target, JavaTarget::Global) {
            return Space::new().into();
        }

        if is_overridden {
            button(
                text("Reset to default")
                    .size(11)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color::from_rgb(0.96, 0.47, 0.47)),
                    }),
            )
            .padding([4, 8])
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
                        radius: 6.0.into(),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            })
            .on_press(Message::ClearOverride(field))
            .into()
        } else {
            container(
                text("Inherited")
                    .size(11)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(Color::from_rgb(0.50, 0.50, 0.55)),
                    }),
            )
            .padding([4, 8])
            .style(move |_| iced::widget::container::Style {
                background: Some(Color::from_rgb(0.16, 0.16, 0.19).into()),
                border: iced::Border {
                    radius: 6.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            })
            .into()
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let background = Color::from_rgb(0.12, 0.12, 0.14);
        let text_primary = Color::from_rgb(0.88, 0.89, 0.91);
        let text_muted = Color::from_rgb(0.63, 0.64, 0.67);
        let accent = Color::from_rgb(0.13, 0.77, 0.36);
        let surface = Color::from_rgb(0.14, 0.14, 0.17);
        let surface_subtle = Color::from_rgb(0.10, 0.10, 0.12);

        let heading_text = match &self.target {
            JavaTarget::Global => "Java Runtime Manager".to_string(),
            JavaTarget::Instance(_) => {
                let name = self
                    .instance_metadata
                    .as_ref()
                    .map(|m| m.name.as_str())
                    .unwrap_or("Instance");
                format!("Java Settings — {}", name)
            }
        };

        let heading =
            text(heading_text)
                .size(28)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_primary),
                });

        // Target selector
        let target_picker = pick_list(
            std::borrow::Cow::Owned(self.available_targets.clone()),
            Some(self.current_target_option()),
            Message::TargetSelected,
        )
        .width(Length::Fixed(300.0));

        let target_section = container(
            row![
                text("Configuring:")
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_primary),
                    }),
                target_picker,
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(surface.into()),
            border: iced::Border {
                radius: 10.0.into(),
                ..iced::Border::default()
            },
            ..iced::widget::container::Style::default()
        });

        let status_banner = self.status.as_ref().map(|(msg, tone, _)| {
            container(
                row![
                    text(msg.as_str())
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(Color::WHITE)
                        })
                ]
                .align_y(Alignment::Center),
            )
            .padding([10, 14])
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some((*tone).into()),
                border: iced::Border {
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            })
        });

        let info_text = if matches!(self.target, JavaTarget::Global) {
            "Configure the default Java runtime and memory settings for all instances."
        } else {
            "Override Java settings for this instance. Inherited values use global defaults."
        };

        let info = container(
            column![
                text("Java Configuration")
                    .size(16)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_primary),
                    }),
                text(info_text)
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_muted),
                    })
            ]
            .spacing(4),
        )
        .padding(14)
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(Color::from_rgb(0.07, 0.16, 0.36).into()),
            border: iced::Border {
                radius: 10.0.into(),
                ..iced::Border::default()
            },
            ..iced::widget::container::Style::default()
        });

        let detect_button = button(
            text(if self.detection_in_progress {
                "Detecting..."
            } else {
                "Detect existing Java"
            })
            .style(move |_| iced::widget::text::Style {
                color: Some(Color::WHITE),
            }),
        )
        .padding([10, 14])
        .style(move |_theme, status| {
            let base = Color::from_rgb(0.13, 0.77, 0.36);
            let hover = Color::from_rgb(0.12, 0.61, 0.30);
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
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(Message::DetectJava);

        let toggle_custom = button(
            text(if self.show_custom_form {
                "Hide custom input"
            } else {
                "Add custom Java"
            })
            .style(move |_| iced::widget::text::Style {
                color: Some(Color::WHITE),
            }),
        )
        .padding([10, 14])
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
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(Message::ToggleCustomForm);

        let actions = row![toggle_custom, detect_button]
            .spacing(12)
            .align_y(Alignment::Center);

        let java_path_overridden = self.is_field_overridden(&OverrideField::JavaPath);
        let java_path_indicator = self.inherited_indicator(OverrideField::JavaPath, java_path_overridden);

        let title_row: Element<'_, Message> = row![
            text("Select Java for launcher")
                .size(20)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_primary),
                }),
            java_path_indicator,
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into();

        let install_header: Element<'_, Message> = if self.is_wide {
            row![title_row, Space::new().width(Length::Fill), actions]
                .spacing(12)
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
        } else {
            column![title_row, actions]
                .spacing(10)
                .align_x(Alignment::Start)
                .width(Length::Fill)
                .into()
        };

        let installations: Element<'_, Message> = if self.installations.is_empty() {
            container(
                column![
                    text("No Java installations detected yet.").size(16).style(
                        move |_theme: &iced::Theme| iced::widget::text::Style {
                            color: Some(text_primary),
                        }
                    ),
                    text("Press \"Detect existing Java\" to scan common locations.")
                        .size(14)
                        .style(move |_theme: &iced::Theme| iced::widget::text::Style {
                            color: Some(text_muted),
                        })
                ]
                .spacing(6),
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
        } else {
            let list = self.installations.iter().fold(column![], |col, install| {
                col.push(self.installation_card(install, text_primary, text_muted, surface, accent))
            });

            scrollable(list.spacing(10))
                .height(Length::Shrink)
                .style(move |_theme, status| {
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
                .into()
        };

        let custom_path_input: iced::widget::TextInput<'_, Message> =
            text_input("Enter custom java path", &self.custom_path_input)
                .on_input(Message::CustomPathChanged)
                .padding([10, 12])
                .size(15)
                .width(Length::FillPortion(3));

        let browse_button = button(text("Browse...").style(move |_theme: &iced::Theme| {
            iced::widget::text::Style {
                color: Some(Color::WHITE),
            }
        }))
        .padding([10, 14])
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
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(Message::BrowseForJava);

        let use_custom_button =
            button(text("Use path").style(move |_| iced::widget::text::Style {
                color: Some(Color::WHITE),
            }))
            .padding([10, 14])
            .style(move |_theme, status| {
                let base = Color::from_rgb(0.12, 0.61, 0.30);
                let hover = Color::from_rgb(0.11, 0.53, 0.26);
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
                        radius: 10.0.into(),
                        ..iced::Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            })
            .on_press(Message::UseCustomPath);

        let custom_path: Element<'_, Message> = if self.show_custom_form {
            container(
                column![
                    text("Use a custom Java path").size(18).style(move |_| {
                        iced::widget::text::Style {
                            color: Some(text_primary),
                        }
                    }),
                    text("Select a Java binary from your system or paste its path.")
                        .size(14)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(text_muted),
                        }),
                    row![custom_path_input, browse_button, use_custom_button]
                        .spacing(8)
                        .align_y(Alignment::Center)
                ]
                .spacing(8),
            )
            .padding(14)
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
            Space::new().into()
        };

        let detection_errors = if self.detection_errors.is_empty() {
            None
        } else {
            let list = self.detection_errors.iter().fold(
                column![text("Detection issues").size(14).style(move |_| {
                    iced::widget::text::Style {
                        color: Some(text_primary),
                    }
                })]
                .spacing(6),
                |col, err| {
                    col.push(
                        text(err)
                            .size(13)
                            .style(move |_| iced::widget::text::Style {
                                color: Some(Color::from_rgb(0.96, 0.47, 0.47)),
                            }),
                    )
                },
            );

            Some(
                container(list.spacing(4))
                    .padding(12)
                    .width(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Color::from_rgb(0.20, 0.10, 0.10).into()),
                        border: iced::Border {
                            radius: 10.0.into(),
                            ..iced::Border::default()
                        },
                        ..iced::widget::container::Style::default()
                    }),
            )
        };

        let (min_mem, max_mem) = (self.settings.min_memory_mb, self.settings.max_memory_mb);
        let min_mem_overridden = self.is_field_overridden(&OverrideField::MinMemory);
        let max_mem_overridden = self.is_field_overridden(&OverrideField::MaxMemory);
        let min_mem_indicator = self.inherited_indicator(OverrideField::MinMemory, min_mem_overridden);
        let max_mem_indicator = self.inherited_indicator(OverrideField::MaxMemory, max_mem_overridden);

        let min_label_color = if !matches!(self.target, JavaTarget::Global) && !min_mem_overridden {
            text_muted
        } else {
            text_primary
        };
        let max_label_color = if !matches!(self.target, JavaTarget::Global) && !max_mem_overridden {
            text_muted
        } else {
            text_primary
        };

        let memory_controls = container(
            column![
                text("Memory Allocation")
                    .size(20)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_primary),
                    }),
                text("Adjust how much RAM Java can use for the launcher.")
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_muted),
                    }),
                Space::new().height(Length::Fixed(10.0)),
                column![
                    row![
                        text(format!("Minimum Memory (RAM): {} MB", min_mem)).style(move |_| {
                            iced::widget::text::Style {
                                color: Some(min_label_color),
                            }
                        }),
                        min_mem_indicator,
                        Space::new().width(Length::Fill)
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                    slider(
                        MIN_MEMORY_BOUND as f32..=MAX_MEMORY_BOUND as f32,
                        min_mem as f32,
                        Message::MinMemoryChanged
                    )
                    .step(128.0),
                    row![
                        text(format!("Maximum Memory (RAM): {} MB", max_mem)).style(move |_| {
                            iced::widget::text::Style {
                                color: Some(max_label_color),
                            }
                        }),
                        max_mem_indicator,
                        Space::new().width(Length::Fill)
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                    slider(
                        MIN_MEMORY_BOUND as f32..=MAX_MEMORY_BOUND as f32,
                        max_mem as f32,
                        Message::MaxMemoryChanged
                    )
                    .step(128.0),
                    text(format!("Total allocated: {} MB - {} MB", min_mem, max_mem))
                        .size(13)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(text_muted),
                        })
                ]
                .spacing(12)
            ]
            .spacing(8),
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
        });

        let args_editor = text_editor(&self.args_content)
            .on_action(Message::ExtraArgsEdited)
            .placeholder("Custom JVM arguments (space separated)")
            .height(Length::Fixed(120.0));

        let save_args = button(
            text("Save JVM args").style(move |_| iced::widget::text::Style {
                color: Some(Color::WHITE),
            }),
        )
        .padding([10, 14])
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
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(Message::SaveArgs);

        let jvm_args_overridden = self.is_field_overridden(&OverrideField::JvmArgs);
        let jvm_args_indicator = self.inherited_indicator(OverrideField::JvmArgs, jvm_args_overridden);

        let args_title_color = if !matches!(self.target, JavaTarget::Global) && !jvm_args_overridden
        {
            text_muted
        } else {
            text_primary
        };

        let args_section = container(
            column![
                row![
                    text("Advanced JVM Arguments")
                        .size(20)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(args_title_color),
                        }),
                    jvm_args_indicator,
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                text("Custom Java arguments for advanced users. Use with caution.")
                    .size(14)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_muted),
                    }),
                args_editor,
                row![save_args].align_y(Alignment::Center)
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
        });

        let layout = column![
            heading,
            target_section,
            info,
            container(column![install_header, installations, custom_path].spacing(10))
                .padding(14)
                .width(Length::Fill)
                .style(move |_| iced::widget::container::Style {
                    background: Some(surface_subtle.into()),
                    border: iced::Border {
                        radius: 12.0.into(),
                        ..iced::Border::default()
                    },
                    ..iced::widget::container::Style::default()
                }),
            memory_controls,
            args_section
        ]
        .spacing(14)
        .align_x(Alignment::Center)
        .max_width(1280);

        let mut content = column![layout]
            .spacing(10)
            .max_width(1360)
            .align_x(Alignment::Center);

        if let Some(banner) = status_banner {
            content = column![banner, content]
                .spacing(10)
                .max_width(1360)
                .align_x(Alignment::Center);
        }

        if let Some(errors) = detection_errors {
            content = content.push(errors);
        }

        let scroll = scrollable(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme, status| {
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
                        background: iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65)),
                        border: iced::Border::default(),
                        shadow: iced::Shadow::default(),
                        icon: Color::WHITE,
                    },
                }
            });

        container(scroll)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([20, 28])
            .style(move |_| iced::widget::container::Style {
                background: Some(background.into()),
                ..iced::widget::container::Style::default()
            })
            .into()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TargetSelected(option) => {
                self.target = option.target;
                self.load_for_target();
                Task::none()
            }
            Message::ScopeToInstance(id, _name) => {
                self.target = JavaTarget::Instance(id);
                let instances = self.instance_manager.list_instances();
                self.rebuild_target_options(&instances);
                self.load_for_target();
                Task::none()
            }
            Message::InstancesLoaded(instances) => {
                self.rebuild_target_options(&instances);
                Task::none()
            }
            Message::ClearOverride(field) => {
                if let Some(meta) = &mut self.instance_metadata {
                    match field {
                        OverrideField::MinMemory => {
                            meta.min_memory_mb = None;
                            self.settings.min_memory_mb = self.global_settings.min_memory_mb;
                        }
                        OverrideField::MaxMemory => {
                            meta.max_memory_mb = None;
                            self.settings.max_memory_mb = self.global_settings.max_memory_mb;
                        }
                        OverrideField::JavaPath => {
                            meta.java_path = None;
                            self.settings.java_path = self.global_settings.java_path.clone();
                            self.custom_path_input = self
                                .settings
                                .java_path
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                        }
                        OverrideField::JvmArgs => {
                            meta.jvm_args = None;
                            self.settings.extra_jvm_args =
                                self.global_settings.extra_jvm_args.clone();
                            self.args_content = text_editor::Content::with_text(
                                &self.settings.extra_jvm_args.join(" "),
                            );
                        }
                    }
                    self.persist_settings("Override cleared — using global default")
                } else {
                    Task::none()
                }
            }
            Message::DetectJava => {
                self.detection_in_progress = true;
                self.detection_errors.clear();
                self.status = None;
                let detection_config = self.settings.detection_config();
                Task::perform(
                    async move { detect_installations(&detection_config) },
                    Message::DetectionFinished,
                )
            }
            Message::Tick => Task::none(),
            Message::ClearStatus(at) => {
                if let Some((_, _, stored_at)) = &self.status
                    && *stored_at == at
                {
                    self.status = None;
                }
                Task::none()
            }
            Message::Resized(width) => {
                self.is_wide = width >= 1200.0;

                if let Some((msg, tone, at)) = &self.status {
                    let elapsed = Instant::now() - *at;
                    if elapsed.as_secs_f32() > 5.0 {
                        self.status = None;
                    } else {
                        self.status = Some((msg.clone(), *tone, *at));
                    }
                }

                Task::none()
            }
            Message::DetectionFinished(summary) => {
                self.detection_in_progress = false;
                self.detection_errors = summary.errors;

                let mut merged = summary.installations;
                let mut custom_existing: Vec<JavaInstallation> = self
                    .installations
                    .iter()
                    .filter(|inst| matches!(inst.source, InstallSource::UserProvided))
                    .cloned()
                    .collect();

                merged.retain(|inst| inst.source != InstallSource::UserProvided);

                for custom in custom_existing.drain(..) {
                    let normalized = normalize_path(&custom.path);
                    if let Some(existing) = merged
                        .iter_mut()
                        .find(|inst| normalize_path(&inst.path) == normalized)
                    {
                        existing.source = InstallSource::UserProvided;
                        existing.id = custom.id;
                        if existing.version.is_none() {
                            existing.version = custom.version.clone();
                        }
                        if existing.vendor.is_none() {
                            existing.vendor = custom.vendor.clone();
                        }
                    } else {
                        merged.push(custom);
                    }
                }

                self.installations = merged;
                self.sync_detected_records();
                self.ensure_selected_entry();
                if self.installations.is_empty() && !self.detection_errors.is_empty() {
                    return self.push_status("No Java found", Color::from_rgb(0.24, 0.12, 0.12));
                }
                Task::none()
            }
            Message::SelectInstallation(id) => {
                if let Some(install) = self.installations.iter().find(|inst| inst.id == id) {
                    self.settings.java_path = Some(install.path.clone());
                    self.custom_path_input = install.path.display().to_string();
                    if matches!(self.target, JavaTarget::Instance(_)) {
                        self.mark_field_overridden(&OverrideField::JavaPath);
                    }
                    return self.persist_settings("Java selection saved");
                }
                Task::none()
            }
            Message::RemoveInstallation(id) => {
                let removed_path = self
                    .installations
                    .iter()
                    .find(|inst| inst.id == id)
                    .map(|inst| inst.path.clone());
                self.installations.retain(|inst| inst.id != id);

                if let Some(path) = removed_path {
                    if self
                        .settings
                        .java_path
                        .as_ref()
                        .map(|p| p == &path)
                        .unwrap_or(false)
                    {
                        self.settings.java_path = None;
                        self.sync_detected_records();
                        return self.persist_settings("Cleared Java selection");
                    }
                    self.sync_detected_records();
                }
                Task::none()
            }
            Message::MinMemoryChanged(value) => {
                let mut min = clamp_memory_value(value);
                if min > self.settings.max_memory_mb {
                    self.settings.max_memory_mb = min;
                    if matches!(self.target, JavaTarget::Instance(_)) {
                        self.mark_field_overridden(&OverrideField::MaxMemory);
                    }
                }
                if min < MIN_MEMORY_BOUND {
                    min = MIN_MEMORY_BOUND;
                }
                self.settings.min_memory_mb = min;
                if matches!(self.target, JavaTarget::Instance(_)) {
                    self.mark_field_overridden(&OverrideField::MinMemory);
                }
                self.persist_settings("Memory settings updated")
            }
            Message::MaxMemoryChanged(value) => {
                let mut max = clamp_memory_value(value);
                if max < self.settings.min_memory_mb {
                    self.settings.min_memory_mb = max;
                    if matches!(self.target, JavaTarget::Instance(_)) {
                        self.mark_field_overridden(&OverrideField::MinMemory);
                    }
                }
                if max > MAX_MEMORY_BOUND {
                    max = MAX_MEMORY_BOUND;
                }
                self.settings.max_memory_mb = max;
                if matches!(self.target, JavaTarget::Instance(_)) {
                    self.mark_field_overridden(&OverrideField::MaxMemory);
                }
                self.persist_settings("Memory settings updated")
            }
            Message::ExtraArgsEdited(action) => {
                self.args_content.perform(action);
                Task::none()
            }
            Message::SaveArgs => {
                let text = self.args_content.text();
                self.settings.extra_jvm_args = parse_args(&text);
                self.custom_path_input = self
                    .settings
                    .java_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.sync_detected_records();
                if matches!(self.target, JavaTarget::Instance(_)) {
                    self.mark_field_overridden(&OverrideField::JvmArgs);
                }
                self.persist_settings("JVM arguments saved")
            }
            Message::CustomPathChanged(input) => {
                self.custom_path_input = input;
                Task::none()
            }
            Message::BrowseForJava => Task::perform(
                async { rfd::FileDialog::new().pick_file() },
                Message::BrowseFinished,
            ),
            Message::BrowseFinished(path) => {
                if let Some(path) = path {
                    self.custom_path_input = path.display().to_string();
                    self.settings.java_path = Some(path.clone());
                    self.ensure_selected_entry();
                    if matches!(self.target, JavaTarget::Instance(_)) {
                        self.mark_field_overridden(&OverrideField::JavaPath);
                    }
                    let status = self.persist_settings("Custom Java selected");
                    let cfg = JavaDetectionConfig {
                        auto_discover: false,
                        preferred_path: Some(path),
                    };
                    self.detection_in_progress = true;
                    let detection = Task::perform(
                        async move { detect_installations(&cfg) },
                        Message::DetectionFinished,
                    );
                    return Task::batch([status, detection]);
                }
                Task::none()
            }
            Message::UseCustomPath => {
                let path = PathBuf::from(self.custom_path_input.trim());
                if !self.custom_path_input.trim().is_empty() {
                    self.settings.java_path = Some(path);
                    self.ensure_selected_entry();
                    if matches!(self.target, JavaTarget::Instance(_)) {
                        self.mark_field_overridden(&OverrideField::JavaPath);
                    }
                    let status = self.persist_settings("Custom Java selected");
                    let cfg = JavaDetectionConfig {
                        auto_discover: false,
                        preferred_path: self.settings.java_path.clone(),
                    };
                    self.detection_in_progress = true;
                    let detection = Task::perform(
                        async move { detect_installations(&cfg) },
                        Message::DetectionFinished,
                    );
                    Task::batch([status, detection])
                } else {
                    self.push_status(
                        "Enter a valid Java path first.",
                        Color::from_rgb(0.24, 0.12, 0.12),
                    )
                }
            }
            Message::ToggleCustomForm => {
                self.show_custom_form = !self.show_custom_form;
                Task::none()
            }
        }
    }

    fn installation_card(
        &self,
        install: &JavaInstallation,
        text_primary: Color,
        text_muted: Color,
        surface: Color,
        accent: Color,
    ) -> Element<'_, Message> {
        let selected = self
            .settings
            .java_path
            .as_ref()
            .map(|path| normalize_path(path) == normalize_path(&install.path))
            .unwrap_or(false);

        let title = install
            .version
            .as_ref()
            .map(|v| format!("Java {}", v))
            .unwrap_or_else(|| "Java (unknown version)".to_string());

        let vendor = install
            .vendor
            .clone()
            .unwrap_or_else(|| "Unknown vendor".to_string());

        let path = install.path.display().to_string();

        let badge = container(
            text("J")
                .size(18)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_primary),
                }),
        )
        .padding([10, 12])
        .width(Length::Fixed(48.0))
        .height(Length::Fixed(48.0))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| iced::widget::container::Style {
            background: Some(Color::from_rgb(0.18, 0.18, 0.21).into()),
            border: iced::Border {
                radius: 12.0.into(),
                ..iced::Border::default()
            },
            ..iced::widget::container::Style::default()
        });

        let info = column![
            row![
                text(title)
                    .size(18)
                    .style(move |_| iced::widget::text::Style {
                        color: Some(text_primary),
                    }),
                if selected {
                    text("Selected")
                        .size(12)
                        .style(move |_| iced::widget::text::Style {
                            color: Some(accent),
                        })
                } else {
                    text("").style(move |_| iced::widget::text::Style {
                        color: Some(Color::TRANSPARENT),
                    })
                }
            ]
            .spacing(8),
            text(vendor.to_string())
                .size(14)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_muted),
                }),
            text(path)
                .size(13)
                .style(move |_| iced::widget::text::Style {
                    color: Some(text_muted),
                }),
        ]
        .spacing(6);

        let select_button = button(
            text(if selected {
                "Using for launcher"
            } else {
                "Use for launcher"
            })
            .style(move |_| iced::widget::text::Style {
                color: Some(Color::WHITE),
            }),
        )
        .padding([10, 14])
        .style(move |_theme, status| {
            let base = if selected {
                Color::from_rgb(0.13, 0.77, 0.36)
            } else {
                Color::from_rgb(0.12, 0.61, 0.30)
            };
            let hover = Color::from_rgb(0.11, 0.53, 0.26);
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
                    radius: 10.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .on_press(Message::SelectInstallation(install.id));

        let remove_button = button(text("Remove").style(move |_| iced::widget::text::Style {
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
        .on_press(Message::RemoveInstallation(install.id));

        let background = if selected {
            Color::from_rgb(0.12, 0.22, 0.16)
        } else {
            surface
        };

        container(
            row![
                badge,
                info,
                Space::new().width(Length::Fill),
                column![select_button, remove_button]
                    .spacing(8)
                    .align_x(Alignment::End)
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        )
        .padding(14)
        .width(Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: Some(background.into()),
            border: iced::Border {
                radius: 12.0.into(),
                width: if selected { 1.5 } else { 0.0 },
                color: if selected { accent } else { Color::TRANSPARENT },
            },
            ..iced::widget::container::Style::default()
        })
        .into()
    }

    fn ensure_selected_entry(&mut self) {
        if let Some(path) = &self.settings.java_path {
            let normalized = normalize_path(path);
            let exists = self
                .installations
                .iter()
                .any(|inst| normalize_path(&inst.path) == normalized);
            if !exists {
                let id = Uuid::new_v5(
                    &Uuid::NAMESPACE_OID,
                    normalized.to_string_lossy().as_bytes(),
                );
                let mut install = JavaInstallation {
                    id,
                    path: normalized.clone(),
                    version: None,
                    vendor: Some("Configured path".to_string()),
                    source: java_manager::InstallSource::UserProvided,
                };

                let detection = detect_installations(&JavaDetectionConfig {
                    auto_discover: false,
                    preferred_path: Some(normalized.clone()),
                });

                if let Some(found) = detection
                    .installations
                    .into_iter()
                    .find(|inst| normalize_path(&inst.path) == normalized)
                {
                    install.version = found.version.or(install.version);
                    install.vendor = found.vendor.or(install.vendor);
                }

                self.installations.push(install);
            }
        }
        self.sync_detected_records();
    }

    fn sync_detected_records(&mut self) {
        self.settings.detected_installations = self
            .installations
            .iter()
            .map(record_from_installation)
            .collect();
    }

    fn persist_settings(&mut self, success: &str) -> Task<Message> {
        match self.save_settings() {
            Ok(_) => self.push_status(success, Color::from_rgb(0.12, 0.61, 0.30)),
            Err(err) => {
                self.detection_errors.push(err.clone());
                self.push_status(&err, Color::from_rgb(0.24, 0.12, 0.12))
            }
        }
    }

    fn save_settings(&mut self) -> Result<(), String> {
        match &self.target {
            JavaTarget::Global => {
                let mut config = FastmcConfig::load().map_err(|e| e.to_string())?;
                config.java = self.settings.to_config();
                config.save().map_err(|e| e.to_string())
            }
            JavaTarget::Instance(_) => {
                if let Some(meta) = &self.instance_metadata {
                    self.instance_manager
                        .save_instance(meta)
                        .map_err(|e| e.to_string())
                } else {
                    Err("No instance metadata loaded".to_string())
                }
            }
        }
    }

    fn push_status(&mut self, message: &str, tone: Color) -> Task<Message> {
        let at = Instant::now();
        self.status = Some((message.to_string(), tone, at));
        Task::perform(
            async move {
                std::thread::sleep(Duration::from_secs(5));
                Message::ClearStatus(at)
            },
            |msg| msg,
        )
    }
}

fn parse_args(input: &str) -> Vec<String> {
    input.split_whitespace().map(|s| s.to_string()).collect()
}

fn normalize_path(path: &PathBuf) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.clone())
}

fn clamp_memory_value(value: f32) -> u32 {
    value
        .round()
        .clamp(MIN_MEMORY_BOUND as f32, MAX_MEMORY_BOUND as f32) as u32
}

fn record_from_installation(install: &JavaInstallation) -> JavaInstallationRecord {
    let source = match install.source {
        InstallSource::UserProvided => Some("UserProvided".to_string()),
        InstallSource::JavaHome => Some("JavaHome".to_string()),
        InstallSource::PathEntry => Some("PathEntry".to_string()),
        InstallSource::SystemLocation => Some("SystemLocation".to_string()),
    };

    JavaInstallationRecord {
        path: install.path.display().to_string(),
        version: install.version.clone(),
        vendor: install.vendor.clone(),
        source,
    }
}

fn map_records_to_installations(records: &[JavaInstallationRecord]) -> Vec<JavaInstallation> {
    records
        .iter()
        .map(|rec| {
            let source = match rec.source.as_deref() {
                Some("UserProvided") => InstallSource::UserProvided,
                Some("JavaHome") => InstallSource::JavaHome,
                Some("PathEntry") => InstallSource::PathEntry,
                Some("SystemLocation") => InstallSource::SystemLocation,
                _ => InstallSource::UserProvided,
            };

            JavaInstallation {
                id: Uuid::new_v5(&Uuid::NAMESPACE_OID, rec.path.as_bytes()),
                path: PathBuf::from(&rec.path),
                version: rec.version.clone(),
                vendor: rec.vendor.clone(),
                source,
            }
        })
        .collect()
}
