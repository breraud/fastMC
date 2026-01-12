#[derive(Default)]
pub struct ServerScreen;

#[derive(Debug, Clone, Copy)]
pub enum Message {}

impl ServerScreen {
    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::container(
            iced::widget::column![iced::widget::text("Server Screen")]
                .align_x(iced::Alignment::Center)
                .spacing(8),
        )
        .center(iced::Length::Fill)
        .padding(20)
        .into()
    }

    pub fn update(&mut self, message: Message) {
        match message {}
    }
}
