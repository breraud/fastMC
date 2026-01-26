use crate::instance_manager::{InstanceManager, InstanceMetadata};
use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Color, Element, Length, Task};

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    Loaded(Vec<InstanceMetadata>),
    CreateNameChanged(String),
    CreateInstance, // Uses the name in state and hardcoded version
    InstanceCreated(Result<InstanceMetadata, String>),
    DeleteInstance(String),
    InstanceDeleted(Result<String, String>), // Returns ID on success
}

pub struct InstancesScreen {
    instances: Vec<InstanceMetadata>,
    manager: InstanceManager,
    create_name: String,
    status_msg: Option<String>,
}

impl InstancesScreen {
    pub fn new() -> Self {
        let manager = InstanceManager::new();
        // Ensure directory exists
        let _ = manager.init();
        
        Self {
            instances: Vec::new(),
            manager,
            create_name: String::new(),
            status_msg: None,
        }
    }

    pub fn refresh(&self) -> Task<Message> {
        let manager = self.manager.clone();
        Task::perform(
            async move { manager.list_instances() },
            Message::Loaded,
        )
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
                let version = "1.21".to_string(); // Hardcoded for now
                let manager = self.manager.clone();
                
                self.status_msg = Some("Creating instance...".to_string());
                
                Task::perform(
                    async move {
                        manager.create_instance(name, version).map_err(|e| e.to_string())
                    },
                    Message::InstanceCreated,
                )
            }
            Message::InstanceCreated(result) => {
                match result {
                    Ok(_) => {
                        self.create_name.clear();
                        self.status_msg = Some("Instance created!".to_string());
                        self.refresh()
                    }
                    Err(e) => {
                        self.status_msg = Some(format!("Error: {}", e));
                        Task::none()
                    }
                }
            }
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
            Message::InstanceDeleted(result) => {
                match result {
                    Ok(_) => {
                        self.status_msg = Some("Instance deleted.".to_string());
                        self.refresh()
                    }
                    Err(e) => {
                        self.status_msg = Some(format!("Delete error: {}", e));
                        Task::none()
                    }
                }
            }
        }
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
            
        let create_btn = button(text("Create 1.21 Instance"))
            .on_press(Message::CreateInstance)
            .padding(10)
            .style(iced::widget::button::primary);

        let create_row = row![create_input, create_btn]
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
                text("No instances found.").size(16).color(Color::from_rgb(0.7, 0.7, 0.7))
             ]
             .width(Length::Fill)
             .align_x(Alignment::Center)
        } else {
            column(
                self.instances.iter().map(|inst| {
                    let info = column![
                        text(&inst.name).size(18).color(Color::WHITE),
                        text(format!("{} • {}", inst.game_version, format!("{:?}", inst.loader)))
                            .size(12)
                            .color(Color::from_rgb(0.6, 0.6, 0.6))
                    ];
                    
                    let delete_btn = button(text("Delete").size(12))
                        .on_press(Message::DeleteInstance(inst.id.clone()))
                        .padding([5, 10])
                        .style(iced::widget::button::danger);

                    container(
                        row![info, iced::widget::Space::new().width(Length::Fill), delete_btn]
                            .align_y(Alignment::Center)
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
                }).collect::<Vec<_>>()
            ).spacing(10)
        };

        let content = column![
            title,
            create_row,
            status,
            scrollable(list_content)
        ]
        .spacing(20)
        .padding(20);

        content.into()
    }
}
