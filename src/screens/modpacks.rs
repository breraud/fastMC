#[derive(Default)]
pub struct ModpacksScreen;

#[derive(Debug, Clone, Copy)]
pub enum Message {}

impl ModpacksScreen {
    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::container(
            iced::widget::column![iced::widget::text("Modpacks Screen")]
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
