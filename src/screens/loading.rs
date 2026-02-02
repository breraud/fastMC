use iced::widget::{column, container, text, Space};
use iced::{Color, Element, Length};

#[derive(Debug, Clone)]
pub enum Message {
    // No interaction needed for loading screen
}

pub struct LoadingScreen;

impl LoadingScreen {
    pub fn view(&self) -> Element<'_, Message> {
        let title = text("FastMC")
            .size(40)
            .style(|_| iced::widget::text::Style {
                color: Some(Color::WHITE),
            });

        let subtitle = text("Preparing your adventure...")
            .size(16)
            .style(|_| iced::widget::text::Style {
                color: Some(Color::from_rgb(0.7, 0.7, 0.7)),
            });

        // Simple spinner simulation using text for now, could be a real widget later
        let spinner = text("...")
            .size(24)
            .style(|_| iced::widget::text::Style {
                color: Some(Color::from_rgb(0.13, 0.77, 0.36)), // Accent green
            });

        let content = column![
            title,
            Space::new().height(10),
            subtitle,
            Space::new().height(30),
            spinner
        ]
        .align_x(iced::Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_| container::Style {
                background: Some(Color::from_rgb(0.05, 0.05, 0.06).into()), // Dark bg
                ..Default::default()
            })
            .into()
    }
}
