#[derive(Default)]
pub struct PlayScreen {
    counter: i32,
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Increment,
    Decrement,
}

impl PlayScreen {
    pub fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::container(iced::widget::column![
            iced::widget::text("Play Screen"),
            iced::widget::text(format!("Counter: {}", self.counter)),
            iced::widget::container(iced::widget::row![
                iced::widget::button("-").on_press(Message::Decrement),
                iced::widget::button("+").on_press(Message::Increment),
            ])
            .padding(10)
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
        ])
        .padding(20)
        .into()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Increment => self.counter += 1,
            Message::Decrement => self.counter -= 1,
        }
    }
}
