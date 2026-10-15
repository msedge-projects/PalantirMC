use iced::widget::{Text, button, column, container, row, svg, text};
use iced::{Background, Border, Color, Element, Length, Theme};

pub const BG: Color = rgb(0x10151C);
pub const SURFACE: Color = rgb(0x171D25);
pub const INSET: Color = rgb(0x202934);
pub const LINE: Color = rgb(0x303D4B);
pub const INK: Color = rgb(0xE8EDF2);
pub const MUTED: Color = rgb(0x8F9EAC);
pub const COPPER: Color = rgb(0xE9AD7C);
pub const DANGER: Color = rgb(0xEF9393);
/// The accent laid over the inset well — a selected chip's or tab's plate.
pub const PLATE: Color = rgb(0x4B4643);

const fn rgb(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 255) as f32 / 255.0,
        ((hex >> 8) & 255) as f32 / 255.0,
        (hex & 255) as f32 / 255.0,
    )
}

pub fn accent(value: crate::model::Accent) -> Color {
    match value {
        crate::model::Accent::Copper => COPPER,
        crate::model::Accent::Glacier => rgb(0x88B8C6),
        crate::model::Accent::Lavender => rgb(0xBBA5DE),
    }
}
pub fn theme(value: crate::model::Accent) -> Theme {
    Theme::custom(
        "Graphite / copper".into(),
        iced::theme::Palette {
            background: BG,
            text: INK,
            primary: accent(value),
            success: rgb(0x8BC4A3),
            danger: DANGER,
        },
    )
}

pub fn label(value: impl ToString) -> Text<'static> {
    text(value.to_string())
        .size(14)
        .shaping(iced::widget::text::Shaping::Advanced)
}
pub fn muted(value: impl ToString) -> Text<'static> {
    label(value).style(MUTED)
}
pub fn title(value: impl ToString) -> Text<'static> {
    label(value).size(30)
}
pub fn subtitle(value: impl ToString) -> Text<'static> {
    label(value).size(19)
}

pub fn panel(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(SURFACE.into()),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    }
}
pub fn chrome(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(SURFACE.into()),
        ..Default::default()
    }
}
pub fn well(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(INSET.into()),
        border: Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
pub enum Button {
    Primary,
    Quiet,
    Selected,
    Danger,
}
impl button::StyleSheet for Button {
    type Style = Theme;
    fn active(&self, theme: &Theme) -> button::Appearance {
        let (fill, ink) = match self {
            Self::Primary => (theme.palette().primary, BG),
            Self::Quiet => (INSET, INK),
            Self::Selected => (PLATE, theme.palette().primary),
            Self::Danger => (rgb(0x3E292D), DANGER),
        };
        button::Appearance {
            background: Some(Background::Color(fill)),
            text_color: ink,
            border: Border {
                radius: 999.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
    fn hovered(&self, theme: &Theme) -> button::Appearance {
        let mut appearance = self.active(theme);
        appearance.border = Border {
            width: 1.0,
            color: theme.palette().primary,
            radius: 999.0.into(),
        };
        appearance
    }
    fn pressed(&self, theme: &Theme) -> button::Appearance {
        let mut appearance = self.hovered(theme);
        if let Some(Background::Color(fill)) = appearance.background {
            appearance.background = Some(
                Color {
                    r: fill.r * 0.9,
                    g: fill.g * 0.9,
                    b: fill.b * 0.9,
                    ..fill
                }
                .into(),
            );
        }
        appearance
    }
    fn disabled(&self, theme: &Theme) -> button::Appearance {
        let mut appearance = self.active(theme);
        appearance.text_color = MUTED;
        appearance.background = Some(INSET.into());
        appearance
    }
}
impl From<Button> for iced::theme::Button {
    fn from(value: Button) -> Self {
        Self::Custom(Box::new(value))
    }
}

pub fn input<'a, M: Clone + 'a>(placeholder: &str, value: &str) -> iced::widget::TextInput<'a, M> {
    iced::widget::text_input(placeholder, value)
        .style(iced::theme::TextInput::Custom(Box::new(Field)))
}
impl iced::widget::text_input::StyleSheet for Field {
    type Style = Theme;
    fn active(&self, _: &Theme) -> iced::widget::text_input::Appearance {
        iced::widget::text_input::Appearance {
            background: INSET.into(),
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 9.0.into(),
            },
            icon_color: MUTED,
        }
    }
    fn focused(&self, theme: &Theme) -> iced::widget::text_input::Appearance {
        let mut style = self.active(theme);
        style.border.color = theme.palette().primary;
        style
    }
    fn placeholder_color(&self, _: &Theme) -> Color {
        MUTED
    }
    fn value_color(&self, _: &Theme) -> Color {
        INK
    }
    fn disabled_color(&self, _: &Theme) -> Color {
        MUTED
    }
    fn selection_color(&self, theme: &Theme) -> Color {
        Color {
            a: 0.3,
            ..theme.palette().primary
        }
    }
    fn disabled(&self, theme: &Theme) -> iced::widget::text_input::Appearance {
        self.active(theme)
    }
}
pub fn selector() -> iced::theme::PickList {
    let style = std::rc::Rc::new(Field);
    iced::theme::PickList::Custom(style.clone(), style)
}
struct Field;
impl iced::widget::pick_list::StyleSheet for Field {
    type Style = Theme;
    fn active(&self, _: &Theme) -> iced::widget::pick_list::Appearance {
        iced::widget::pick_list::Appearance {
            text_color: INK,
            placeholder_color: MUTED,
            handle_color: MUTED,
            background: INSET.into(),
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 9.0.into(),
            },
        }
    }
    fn hovered(&self, theme: &Theme) -> iced::widget::pick_list::Appearance {
        let mut style = self.active(theme);
        style.border.color = theme.palette().primary;
        style
    }
}
impl iced::overlay::menu::StyleSheet for Field {
    type Style = Theme;
    fn appearance(&self, theme: &Theme) -> iced::overlay::menu::Appearance {
        iced::overlay::menu::Appearance {
            text_color: INK,
            background: SURFACE.into(),
            border: Border {
                color: LINE,
                width: 1.0,
                radius: 9.0.into(),
            },
            selected_text_color: BG,
            selected_background: theme.palette().primary.into(),
        }
    }
}
/// A rounded well that frames an icon — the tile at the head of a
/// catalogue card.
pub fn icon_tile<'a, M: 'a>(inner: Element<'a, M>) -> Element<'a, M> {
    container(inner)
        .width(52)
        .height(52)
        .center_x()
        .center_y()
        .style(well)
        .into()
}

/// A small pill of muted text: a status or a statistic on a card.
pub fn badge<'a, M: 'a>(value: impl ToString) -> Element<'a, M> {
    container(muted(value).size(11))
        .padding([4, 10])
        .style(pill)
        .into()
}
fn pill(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(INSET.into()),
        border: Border {
            radius: 999.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// The browsing-list card: icon tile, title, one meta line, a row of
/// badges, and the action cluster at the right. The composition is ours;
/// only the plain pattern "icon, name, meta, actions" is shared with
/// catalogue UIs at large.
pub fn project_card<'a, M: Clone + 'a>(
    icon: Element<'a, M>,
    title: impl ToString,
    meta: impl ToString,
    badges: Vec<String>,
    actions: Element<'a, M>,
) -> Element<'a, M> {
    let mut middle = column![label(title).size(17), muted(meta).size(12)]
        .spacing(5)
        .width(Length::Fill);
    if !badges.is_empty() {
        let mut row = row![].spacing(6);
        for value in badges {
            row = row.push(badge(value));
        }
        middle = middle.push(row);
    }
    container(
        row![icon, middle, actions]
            .spacing(14)
            .align_items(iced::Alignment::Center),
    )
    .padding(16)
    .style(panel)
    .into()
}

pub fn emblem<'a, M: 'a>(kind: &str) -> Element<'a, M> {
    let bytes: &'static [u8] = match kind {
        "worlds" => include_bytes!("../assets/worlds.svg"),
        "help" => include_bytes!("../assets/help.svg"),
        "account" => include_bytes!("../assets/identity.svg"),
        "download" => include_bytes!("../assets/downloads.svg"),
        _ => include_bytes!("../assets/additions.svg"),
    };
    svg(svg::Handle::from_memory(bytes))
        .width(140)
        .height(80)
        .into()
}

pub fn mark<'a, M: 'a>(size: f32) -> Element<'a, M> {
    svg(svg::Handle::from_memory(
        include_bytes!("../assets/portal.svg").as_slice(),
    ))
    .width(size)
    .height(size)
    .into()
}
pub fn landscape<'a, M: 'a>() -> Element<'a, M> {
    svg(svg::Handle::from_memory(
        include_bytes!("../assets/landscape.svg").as_slice(),
    ))
    .width(iced::Length::Fill)
    .height(190)
    .into()
}

pub fn collection<'a, M: 'a>() -> Element<'a, M> {
    svg(svg::Handle::from_memory(
        include_bytes!("../assets/collection.svg").as_slice(),
    ))
    .width(180)
    .height(100)
    .into()
}
// Original asymmetric navigation geometry, drawn for this product; no font-glyph dependency.
pub fn icon<'a, M: 'a>(name: &str, selected: bool) -> Element<'a, M> {
    let path = match name {
        "home" => "<path d='m3 10 9-7 9 7v11h-6v-7H9v7H3Z'/>",
        "library" => "<path d='M3 5h5v15H3Zm8-2h5v17h-5Zm8 5 3 11'/>",
        "discover" => "<path d='m4 19 6-15h5L9 15h11l-3 5H4'/><circle cx='19' cy='5' r='2'/>",
        "play" => "<path d='m8 4 12 8-12 8Z'/>",
        "stop" => "<rect x='5' y='5' width='14' height='14' rx='2'/>",
        "folder" => "<path d='M3 6h7l2 3h9v11H3Z'/>",
        "account" => "<circle cx='12' cy='7' r='4'/><path d='M4 21v-3c0-6 16-6 16 0v3'/>",
        "download" => "<path d='M12 3v12m-5-5 5 5 5-5M4 17v4h16v-4'/>",
        "search" => "<circle cx='10' cy='10' r='6'/><path d='m15 15 6 6'/>",
        "logs" => "<path d='M5 3h14v18H5ZM8 7h8m-8 4h8m-8 4h5'/>",
        "check" => "<path d='m4 12 5 5L20 6'/>",
        "settings" => "<path d='M4 6h16M4 12h16M4 18h16'/><path d='M8 3v6m8 0v6m-6 0v6'/>",
        "worlds" => "<path d='m3 8 9-5 9 5v9l-9 5-9-5Zm9-5v19M3 8l9 5 9-5'/>",
        "help" => {
            "<path d='M8 8c0-6 10-6 10 0 0 3-6 3-6 6m0 4v1'/><circle cx='12' cy='12' r='10'/>"
        }
        _ => "<circle cx='12' cy='12' r='9'/><path d='M12 10v7m0-11v1'/>",
    };
    let ink = if selected { "#E9AD7C" } else { "#8F9EAC" };
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='{ink}' stroke-width='1.6' stroke-linejoin='round' stroke-linecap='round'>{path}</svg>"
    );
    svg(svg::Handle::from_memory(source.into_bytes()))
        .width(20)
        .height(20)
        .into()
}
