use iced::widget::{button, container, row, text};
use iced::{Alignment, Border, Color, Element, Length};

const BTN_SIDEBAR_BG: Color = Color::from_rgb(0.13, 0.77, 0.36);
const BTN_SIDEBAR_HOVER: Color = Color::from_rgb(0.11, 0.65, 0.31);

pub fn menu_button<'a, Message>(
    icon: Option<Element<'a, Message>>,
    label: &'a str,
) -> iced::widget::Button<'a, Message>
where
    Message: 'a,
{
    let content_row = match icon {
        Some(icon) => row![icon, text(label).size(16)]
            .spacing(8)
            .align_y(Alignment::Center),
        None => row![text(label).size(16)].align_y(Alignment::Center),
    };

    let content = container(content_row)
        .width(Length::Fill)
        .align_x(Alignment::Start);

    iced::widget::button(content)
        .padding([10, 20])
        .style(|_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => BTN_SIDEBAR_HOVER,
                _ => BTN_SIDEBAR_BG,
            };

            button::Style {
                background: Some(bg.into()),
                text_color: Color::WHITE,
                border: Border {
                    radius: 8.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        })
}

fn icon_from_handle<'a, Message>(handle: iced::widget::svg::Handle) -> Element<'a, Message>
where
    Message: 'a,
{
    iced::widget::svg(handle)
        .width(iced::Length::Fixed(24.0))
        .height(iced::Length::Fixed(24.0))
        .into()
}

pub fn icon_from_path<'a, Message>(path: &str) -> Element<'a, Message>
where
    Message: 'a,
{
    let full_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);

    // Use the manifest directory as the base so icons load correctly even if the
    // process is started from another working directory.
    icon_from_handle(iced::widget::svg::Handle::from_path(full_path))
}
