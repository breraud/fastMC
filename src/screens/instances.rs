use crate::instance_manager::{InstanceManager, InstanceMetadata, ModLoader, ALL_LOADERS};
use iced::widget::{
    button, checkbox, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Alignment, Color, Element, Length, Task};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    Loaded(Vec<InstanceMetadata>),
    CreateNameChanged(String),
    CreateInstance,
    InstanceCreated(Result<InstanceMetadata, String>),
    DeleteInstance(String),
    InstanceDeleted(Result<String, String>),
    VersionsLoaded(Result<Vec<version_manager::VanillaVersion>, String>),
    VersionSelected(Option<String>),
    ToggleSnapshots(bool),
    LaunchInstance(String),
    LaunchFinished(Result<(), String>),
    OpenJavaSettings(String, String),
    // Loader messages
    LoaderSelected(String, ModLoader),
    LoaderVersionSelected(String, String),
    InstallLoader(String),
    LoaderInstalled(Result<String, String>),
    LoaderVersionsLoaded(String, Result<Vec<String>, String>),
}

pub struct InstancesScreen {
    instances: Vec<InstanceMetadata>,
    manager: InstanceManager,
    create_name: String,
    available_versions: Vec<version_manager::VanillaVersion>,
    selected_version: Option<String>,
    show_snapshots: bool,
    status_msg: Option<String>,
    // Loader state
    pending_loader: HashMap<String, ModLoader>,
    pending_loader_version: HashMap<String, Option<String>>,
    available_loader_versions: HashMap<String, Vec<String>>,
    installing: HashSet<String>,
}

impl InstancesScreen {
    pub fn new() -> Self {
        let manager = InstanceManager::new();
        let _ = manager.init();

        Self {
            instances: Vec::new(),
            manager,
            create_name: String::new(),
            available_versions: Vec::new(),
            selected_version: None,
            show_snapshots: false,
            status_msg: None,
            pending_loader: HashMap::new(),
            pending_loader_version: HashMap::new(),
            available_loader_versions: HashMap::new(),
            installing: HashSet::new(),
        }
    }

    pub fn fetch_versions(&self) -> Task<Message> {
        Task::perform(
            async {
                version_manager::fetch_vanilla_versions()
                    .await
                    .map_err(|e| e.to_string())
            },
            |res| Message::VersionsLoaded(res),
        )
    }

    pub fn refresh(&self) -> Task<Message> {
        let manager = self.manager.clone();
        Task::batch(vec![
            Task::perform(async move { manager.list_instances() }, Message::Loaded),
            self.fetch_versions(),
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Refresh => self.refresh(),
            Message::Loaded(instances) => {
                self.instances = instances;
                Task::none()
            }
            Message::CreateNameChanged(name) => {
                self.create_name = name;
                Task::none()
            }
            Message::CreateInstance => {
                if self.create_name.trim().is_empty() {
                    return Task::none();
                }

                let name = self.create_name.clone();
                let version = self
                    .selected_version
                    .clone()
                    .unwrap_or_else(|| "1.21".to_string());
                let manager = self.manager.clone();

                self.status_msg = Some("Creating instance...".to_string());

                Task::perform(
                    async move {
                        manager
                            .create_instance(name, version)
                            .map_err(|e| e.to_string())
                    },
                    Message::InstanceCreated,
                )
            }
            Message::InstanceCreated(result) => match result {
                Ok(_) => {
                    self.create_name.clear();
                    self.status_msg = Some("Instance created!".to_string());
                    self.refresh()
                }
                Err(e) => {
                    self.status_msg = Some(format!("Error: {}", e));
                    Task::none()
                }
            },
            Message::DeleteInstance(id) => {
                let manager = self.manager.clone();
                Task::perform(
                    async move {
                        manager.delete_instance(&id).map_err(|e| e.to_string())?;
                        Ok(id)
                    },
                    Message::InstanceDeleted,
                )
            }
            Message::InstanceDeleted(result) => match result {
                Ok(_) => {
                    self.status_msg = Some("Instance deleted.".to_string());
                    self.refresh()
                }
                Err(e) => {
                    self.status_msg = Some(format!("Delete error: {}", e));
                    Task::none()
                }
            },
            Message::LaunchInstance(_) => Task::none(),
            Message::OpenJavaSettings(_, _) => Task::none(),
            Message::LaunchFinished(result) => {
                match result {
                    Ok(_) => {
                        self.status_msg = Some("Instance launched!".to_string());
                    }
                    Err(e) => {
                        self.status_msg = Some(format!("Launch failed: {}", e));
                    }
                }
                Task::none()
            }
            Message::VersionsLoaded(result) => {
                match result {
                    Ok(versions) => {
                        self.available_versions = versions;
                        if let Some(latest) = self
                            .available_versions
                            .iter()
                            .find(|v| v.type_ == version_manager::VersionType::Release)
                        {
                            if self.selected_version.is_none() {
                                self.selected_version = Some(latest.id.clone());
                            }
                        }
                    }
                    Err(e) => {
                        self.status_msg = Some(format!("Failed to fetch versions: {}", e));
                    }
                }
                Task::none()
            }
            Message::VersionSelected(version) => {
                self.selected_version = version;
                Task::none()
            }
            Message::ToggleSnapshots(show) => {
                self.show_snapshots = show;
                Task::none()
            }
            // Loader handling
            Message::LoaderSelected(instance_id, loader) => {
                self.pending_loader
                    .insert(instance_id.clone(), loader.clone());
                self.pending_loader_version.remove(&instance_id);
                self.available_loader_versions.remove(&instance_id);

                if loader == ModLoader::Vanilla {
                    return Task::none();
                }

                // Fetch available loader versions
                let id = instance_id.clone();
                let game_version = self
                    .instances
                    .iter()
                    .find(|i| i.id == instance_id)
                    .map(|i| i.game_version.clone())
                    .unwrap_or_default();

                Task::perform(
                    async move {
                        let versions = match loader {
                            ModLoader::Fabric => {
                                version_manager::fabric::fetch_compatible_loaders(&game_version)
                                    .await
                                    .map(|v| v.into_iter().map(|l| l.version).collect())
                                    .map_err(|e| e.to_string())
                            }
                            ModLoader::Quilt => {
                                version_manager::quilt::fetch_quilt_loaders()
                                    .await
                                    .map(|v| v.into_iter().map(|l| l.version).collect())
                            }
                            ModLoader::Forge => {
                                version_manager::forge::fetch_forge_versions(&game_version).await
                            }
                            ModLoader::NeoForge => {
                                version_manager::neoforge::fetch_neoforge_versions(&game_version)
                                    .await
                            }
                            ModLoader::Vanilla => Ok(vec![]),
                        };
                        (id, versions)
                    },
                    |(id, res)| Message::LoaderVersionsLoaded(id, res),
                )
            }
            Message::LoaderVersionsLoaded(instance_id, result) => {
                match result {
                    Ok(versions) => {
                        self.available_loader_versions
                            .insert(instance_id, versions);
                    }
                    Err(e) => {
                        self.status_msg =
                            Some(format!("Failed to fetch loader versions: {}", e));
                    }
                }
                Task::none()
            }
            Message::LoaderVersionSelected(instance_id, version) => {
                self.pending_loader_version
                    .insert(instance_id, Some(version));
                Task::none()
            }
            Message::InstallLoader(_instance_id) => {
                // Handled by parent (main.rs)
                Task::none()
            }
            Message::LoaderInstalled(result) => {
                match result {
                    Ok(ref id) => {
                        self.installing.remove(id);
                        self.pending_loader.remove(id);
                        self.pending_loader_version.remove(id);
                        self.available_loader_versions.remove(id);
                        self.status_msg = Some("Loader installed successfully!".to_string());
                        return self.refresh();
                    }
                    Err(ref e) => {
                        self.status_msg = Some(format!("Loader install failed: {}", e));
                        // Try to find which instance was installing and remove from set
                        // We can't easily know, so just clear all
                        self.installing.clear();
                    }
                }
                Task::none()
            }
        }
    }

    pub fn mark_installing(&mut self, id: &str) {
        self.installing.insert(id.to_string());
    }

    pub fn get_pending_loader(&self, id: &str) -> Option<&ModLoader> {
        self.pending_loader.get(id)
    }

    pub fn get_pending_loader_version(&self, id: &str) -> Option<&str> {
        self.pending_loader_version
            .get(id)
            .and_then(|v| v.as_deref())
    }

    pub fn view(&self) -> Element<'_, Message> {
        let title = text("Instances")
            .size(28)
            .style(|_| iced::widget::text::Style {
                color: Some(Color::WHITE),
            });

        // Create Section
        let create_input = text_input("Instance Name", &self.create_name)
            .on_input(Message::CreateNameChanged)
            .padding(10)
            .width(Length::Fixed(300.0));

        let version_list: Vec<String> = self
            .available_versions
            .iter()
            .filter(|v| self.show_snapshots || v.type_ == version_manager::VersionType::Release)
            .map(|v| v.id.clone())
            .collect();

        let version_picker = pick_list(
            std::borrow::Cow::Owned(version_list),
            self.selected_version.clone(),
            |v| Message::VersionSelected(Some(v)),
        )
        .placeholder("Select Version")
        .width(Length::Fixed(150.0));

        let create_btn = button(text("Create"))
            .on_press(Message::CreateInstance)
            .padding(10)
            .style(iced::widget::button::primary);

        let snapshot_toggle = row![
            checkbox(self.show_snapshots)
                .on_toggle(Message::ToggleSnapshots)
                .size(16),
            text("Show Snapshots").size(14).color(Color::WHITE)
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let create_row = row![create_input, version_picker, snapshot_toggle, create_btn]
            .spacing(10)
            .align_y(Alignment::Center);

        // Status
        let status = if let Some(msg) = &self.status_msg {
            text(msg).size(14).color(Color::from_rgb(0.8, 0.8, 0.8))
        } else {
            text("")
        };

        // List
        let list_content = if self.instances.is_empty() {
            column![
                text("No instances found.")
                    .size(16)
                    .color(Color::from_rgb(0.7, 0.7, 0.7))
            ]
            .width(Length::Fill)
            .align_x(Alignment::Center)
        } else {
            column(
                self.instances
                    .iter()
                    .map(|inst| self.instance_card(inst))
                    .collect::<Vec<_>>(),
            )
            .spacing(10)
        };

        let content = column![title, create_row, status, scrollable(list_content)]
            .spacing(20)
            .padding(20);

        content.into()
    }

    fn instance_card<'a>(&'a self, inst: &'a InstanceMetadata) -> Element<'a, Message> {
        let loader_label = if inst.loader_installed && inst.loader != ModLoader::Vanilla {
            format!("{} (installed)", inst.loader)
        } else {
            format!("{:?}", inst.loader)
        };

        let info = column![
            text(&inst.name).size(18).color(Color::WHITE),
            text(format!("{} • {}", inst.game_version, loader_label))
                .size(12)
                .color(Color::from_rgb(0.6, 0.6, 0.6))
        ];

        let is_installing = self.installing.contains(&inst.id);

        // Loader picker row
        let loader_options: Vec<ModLoader> = ALL_LOADERS.to_vec();
        let current_loader = self
            .pending_loader
            .get(&inst.id)
            .cloned()
            .unwrap_or(inst.loader.clone());

        let loader_picker = pick_list(
            std::borrow::Cow::Owned(loader_options),
            Some(current_loader.clone()),
            {
                let id = inst.id.clone();
                move |l| Message::LoaderSelected(id.clone(), l)
            },
        )
        .width(Length::Fixed(110.0));

        // Loader version picker
        let loader_version_picker: Element<'_, Message> =
            if let Some(versions) = self.available_loader_versions.get(&inst.id) {
                if versions.is_empty() {
                    text("No versions").size(12).color(Color::from_rgb(0.5, 0.5, 0.5)).into()
                } else {
                    let selected = self
                        .pending_loader_version
                        .get(&inst.id)
                        .and_then(|v| v.clone());
                    let id = inst.id.clone();
                    pick_list(
                        std::borrow::Cow::Owned(versions.clone()),
                        selected,
                        move |v| Message::LoaderVersionSelected(id.clone(), v),
                    )
                    .placeholder("Version")
                    .width(Length::Fixed(150.0))
                    .into()
                }
            } else if current_loader != ModLoader::Vanilla
                && !inst.loader_installed
            {
                text("Loading...").size(12).color(Color::from_rgb(0.5, 0.5, 0.5)).into()
            } else {
                text("").into()
            };

        // Install button
        let install_btn: Element<'_, Message> = if is_installing {
            text("Installing...")
                .size(12)
                .color(Color::from_rgb(0.9, 0.7, 0.2))
                .into()
        } else if inst.loader_installed && inst.loader != ModLoader::Vanilla {
            text("Installed")
                .size(12)
                .color(Color::from_rgb(0.2, 0.8, 0.4))
                .into()
        } else if self
            .pending_loader_version
            .get(&inst.id)
            .and_then(|v| v.as_ref())
            .is_some()
            && current_loader != ModLoader::Vanilla
        {
            button(text("Install").size(12))
                .on_press(Message::InstallLoader(inst.id.clone()))
                .padding([5, 10])
                .style(iced::widget::button::primary)
                .into()
        } else {
            text("").into()
        };

        let java_btn = button(text("Java").size(12))
            .on_press(Message::OpenJavaSettings(
                inst.id.clone(),
                inst.name.clone(),
            ))
            .padding([5, 10])
            .style(iced::widget::button::secondary);

        let delete_btn = button(text("Delete").size(12))
            .on_press(Message::DeleteInstance(inst.id.clone()))
            .padding([5, 10])
            .style(iced::widget::button::danger);

        let launch_btn = button(text("Launch").size(12))
            .on_press(Message::LaunchInstance(inst.id.clone()))
            .padding([5, 10])
            .style(iced::widget::button::success);

        let loader_row = row![loader_picker, loader_version_picker, install_btn]
            .spacing(6)
            .align_y(Alignment::Center);

        let left = column![info, loader_row].spacing(6);

        container(
            row![
                left,
                iced::widget::Space::new().width(Length::Fill),
                java_btn,
                launch_btn,
                delete_btn
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding(10)
        .style(|_| iced::widget::container::Style {
            background: Some(Color::from_rgb(0.18, 0.18, 0.20).into()),
            border: iced::Border {
                radius: 6.0.into(),
                ..iced::Border::default()
            },
            ..iced::widget::container::Style::default()
        })
        .into()
    }
}
