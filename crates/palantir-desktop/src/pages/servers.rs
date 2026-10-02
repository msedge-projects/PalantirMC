//! Servers, the reference's fifth rail slot: the Modrinth Hosting listing at
//! `/hosting/manage/`.
//!
//! The reference's page is a listing of servers the user has bought from Modrinth
//! Hosting, and every fact on it comes from that service: the plans, their state,
//! the backups, the console. There is no local half to draw -- a server is not a
//! file on this machine -- and no account half either, because this launcher does
//! not hold a Modrinth credential (G118).
//!
//! Those two facts decide the page rather than describing it, because the
//! reference's `index.vue` reaches the same two conclusions and draws the same
//! thing for them. `serverList` is empty unless `loggedIn` is true
//! (`index.vue:526`), so with no account `showEmptyState` is true
//! (`index.vue:530`) and the listing branch is never reached: what the reference
//! draws at this route for a reader who is not signed in is `ServerListEmpty`
//! with `logged-in="false"` -- the marketing column, the decorative preview, and
//! the *Already have a server?* sign-in at the foot. That is this page, and it is
//! drawn from the reference's own keys rather than from a paraphrase of them.
//!
//! The rail slot's tooltip is `app.nav.modrinth-hosting` -- "Modrinth Hosting",
//! which is what the button means. A launcher that runs a server *of its own* --
//! a jar in an instance's folder, started like a game -- is a different feature
//! from this page and would not be this route.

use iced::widget::{column, container, row, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme};

use crate::icons_gen::Glyph;
use crate::icon;
use crate::page::{self, GAP};
use crate::store::{self, Store};
use crate::style::{medium, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Ink, Span, Theme as Gen};
use crate::ui::{self, text};

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// A new server was asked for.
    NewServer,
    /// The listing was asked for again.
    Refresh,
    /// Billing was asked for.
    ManageBilling,
    /// The pointer entered or left one of the page's controls, for the clock
    /// that carries a hover's 150 ms (see [`crate::ui`]).
    Hover {
        /// The control's stable name, one per control.
        key: &'static str,
        /// Whether the pointer arrived or left.
        over: bool,
        /// The hover end, where the control declares one of its own.
        hover: Option<f32>,
    },

    /// A wheel over this page's scroll region.
    ///
    /// Reported rather than applied: iced moves a scrollable with a `scroll_to`
    /// command, so which region glides, and how far, is the shell's -- see
    /// `crate::scroll`. This page's part is to hand the wheel on, and the name it
    /// carries is the region the widget was built with.
    Wheel(&'static str, crate::scroll::Wheel),
}

crate::hovered!(Message);

/// The listing's actions.
const MANAGE_BILLING_KEY: &str = "servers:manage-billing";
const NEW_SERVER_KEY: &str = "servers:new";
const REFRESH_KEY: &str = "servers:refresh";
const SIGN_IN_KEY: &str = "servers:sign-in";

/// The page's own state.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// The last thing the page could not do.
    pub notice: Option<String>,
}

impl State {
    /// Apply a message.
    ///
    /// Every one of the three answers with [`store::needs_account`] rather than
    /// with "not implemented yet": the listing, a new server and the billing page
    /// are all Modrinth Hosting, which is an account service, and the sentence a
    /// reader gets has to say which of the two kinds of gap this is. The *page*
    /// still draws `ServerListEmpty`, because that is what the reference draws for
    /// this same reader -- the notice is added above it, not swapped for it.
    pub fn update(&mut self, message: Message) {
        match message {
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

            Message::NewServer => self.notice = Some(store::needs_account("Creating a server")),
            Message::ManageBilling => self.notice = Some(store::needs_account("Billing")),
            Message::Refresh => self.notice = Some(store::needs_account("The server listing")),
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
        }
    }
}

// ---- ServerListEmpty.vue, quoted ------------------------------------------

/// `max-w-[20rem]` on the column that holds the heading, the features and the
/// buttons -- the reference's own measure for it, and the reason its description
/// wraps where it does.
const COLUMN: f32 = 320.0;
/// `max-w-[25rem]` on `ServerListEmptyPreview`.
const PREVIEW: f32 = 400.0;
/// `gap-2` on the row that holds the column and the preview.
const ROW_GAP: f32 = 8.0;
/// `gap-8`, between the heading, the features and the buttons.
const COLUMN_GAP: f32 = 32.0;
/// `gap-8`, between the preview row and the sign-in at the foot.
const FOOT_GAP: f32 = 32.0;
/// `gap-6`, between the three features.
const FEATURE_GAP: f32 = 24.0;
/// `gap-4`, across a feature: its plate and its two lines.
const FEATURE_GAP_X: f32 = 16.0;
/// `gap-0.5`, between a feature's title and its description.
const FEATURE_TEXT_GAP: f32 = 2.0;
/// `gap-4`, between the *New server* button and *Learn more*, and between the two
/// parts of the sign-in at the foot.
const ACTION_GAP: f32 = 16.0;
/// `gap-2`, between the plate's glyph and its text.
const PLATE_GAP: f32 = 8.0;

/// `size-10` on a feature's plate, and `rounded-[0.875rem]` on it.
const FEATURE_PLATE: f32 = 40.0;
const FEATURE_PLATE_RADIUS: f32 = 14.0;
/// `size-5`, the glyph inside the plate and the arrow beside *Learn more*.
const GLYPH: f32 = 20.0;

/// `h-[38rem]` on the preview: the reference draws it taller than the viewport and
/// lets the page clip it, which is why its foot is under a fade.
const PREVIEW_HEIGHT: f32 = 608.0;

/// The free space the four `mx-auto` margins share, and the one gap between them.
///
/// `ServerListEmpty`'s row is `flex-wrap items-center justify-center gap-2` and
/// *both* of its children carry `mx-auto`, so the row's free space is split evenly
/// between four auto margins and `justify-content` never runs. Measured at the
/// reference's own 1280x720 (`/tmp/ref/hosting-clean3.png`): a content column
/// 868 wide, `868 - 320 - 400 - 8 = 140` free, so 35 a margin -- the column's
/// text starts at x=123 (`88 + 35`) and the preview at x=521
/// (`123 + 320 + 35 + 35 + 8`).
///
/// The row below is therefore drawn with *no* spacing of its own and three
/// spacers instead, because iced adds a row's spacing between every pair of
/// children and this row has one gap among five. The middle spacer carries two
/// margins and that gap: `35 + 8 + 35 = 78`, and `35 + 78 + 35 = 148` is the whole
/// of the free space, so the picture lands on the reference's own pixels at the
/// reference's own window and keeps its shape at another.
const MARGIN_SHARE: [u16; 3] = [35, 78, 35];

/// The three features `ServerListEmpty.vue` lists, in its own order.
const FEATURES: [(Glyph, Key, Key); 3] = [
    (
        Glyph::PackageOpen,
        Key::ServersListEmptyOneClickModInstallsTitle,
        Key::ServersListEmptyOneClickModInstallsDescription,
    ),
    (
        Glyph::Globe,
        Key::ServersListEmptySimpleSetupTitle,
        Key::ServersListEmptySimpleSetupDescription,
    ),
    (
        Glyph::UserPlus,
        Key::ServersListEmptyPlayWithFriendsTitle,
        Key::ServersListEmptyPlayWithFriendsDescription,
    ),
];

/// One row of the preview's friend list: a name, and the button beside it.
///
/// The names and the statuses are the reference's own fixture -- `friends` in
/// `ServerListEmptyPreview.vue` -- not this launcher's, because the preview is
/// the reference's picture of the dialog and a row that said something else would
/// be a different picture.
const FRIENDS: [(&str, FriendStatus); 8] = [
    ("Josh", FriendStatus::Added),
    ("Prospector", FriendStatus::Invite),
    ("Fetch", FriendStatus::Cancel),
    ("IMB11", FriendStatus::Invite),
    ("Truman", FriendStatus::Invite),
    ("Boris", FriendStatus::Invite),
    ("Saya", FriendStatus::Invite),
    ("Michael", FriendStatus::Invite),
];

/// `totalFriendCount` in the preview: the heading counts a list it does not draw.
const FRIEND_COUNT: &str = "11";

/// The three states a friend row's button is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FriendStatus {
    /// `added`: disabled, with a check in front of the label.
    Added,
    /// `cancel`: `outlined`, so it reads as taking the invite back.
    Cancel,
    /// `invite`: the frame's own default.
    Invite,
}

/// Draw the page.
pub fn view<'a>(theme: Gen, state: &'a State, _store: &'a Store) -> Element<'a, Message> {
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = &state.notice {
        blocks.push(ui::admonition(
            theme,
            ui::Severity::Info,
            Key::AppNavModrinthHosting.message(),
            notice,
        ));
    }
    blocks.push(empty_state(theme));
    page::body(blocks, GAP, Message::Wheel)
}

/// `ServerListEmpty` with `logged-in="false"`: the column, the preview, and the
/// sign-in at the foot.
///
/// Three blocks and one gap, in the reference's order: the row that grows
/// (`grow`, so its two children are centred in what is left), then the foot,
/// with `justify-between` between them and `gap-8` for the space.
fn empty_state<'a>(theme: Gen) -> Element<'a, Message> {
    column![]
        .width(Length::Fill)
        .spacing(FOOT_GAP)
        .push(
            row![]
                .width(Length::Fill)
                .spacing(0.0)
                .align_items(Alignment::Center)
                .push(Space::with_width(Length::FillPortion(MARGIN_SHARE[0])))
                .push(column_text(theme))
                .push(Space::with_width(Length::FillPortion(MARGIN_SHARE[1])))
                .push(preview(theme))
                .push(Space::with_width(Length::FillPortion(MARGIN_SHARE[2]))),
        )
        .push(
            column![]
                .width(Length::Fill)
                .spacing(ACTION_GAP)
                .align_items(Alignment::Center)
                .push(
                    text(Key::ServersListEmptyAlreadyHaveServerLabel.message())
                        .size(14.0)
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_SECONDARY,
                        ))),
                )
                .push(ui::button_with_icon_sized(
                    theme,
                    SIGN_IN_KEY,
                    Glyph::LogIn,
                    Key::ServersListEmptySignInButton,
                    ui::Kind::Standard,
                    ui::Size::Md,
                    Length::Shrink,
                    Some(Message::Refresh),
                )),
        )
        .into()
}

/// `mx-auto w-full max-w-[20rem] flex flex-col items-start gap-8`: the heading,
/// the three features, and the buttons.
fn column_text<'a>(theme: Gen) -> Element<'a, Message> {
    let mut column = column![]
        .width(Length::Fixed(COLUMN))
        .spacing(COLUMN_GAP)
        .align_items(Alignment::Start);
    column = column.push(
        column![]
            .spacing(PLATE_GAP)
            .push(
                text(Key::ServersListEmptyModrinthHostingLabel.message())
                    .size(30.0)
                    .line_height(iced::Pixels(36.0))
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(
                        theme,
                        INK_CONTRAST,
                    ))),
            )
            .push(
                text(Key::ServersListEmptyNoServersDescription.message())
                    .size(16.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(
                        theme,
                        INK_DEFAULT,
                    ))),
            ),
    );
    let mut features = column![].spacing(FEATURE_GAP);
    for (glyph, title, description) in FEATURES {
        features = features.push(feature(theme, glyph, title, description));
    }
    column = column.push(features);
    column.push(
        row![]
            .spacing(ACTION_GAP)
            .align_items(Alignment::Center)
            // `size="lg"`: 40 pixels, which is the row `ButtonFrame.vue` calls
            // `lg` and the height measured at y=537..576.
            .push(ui::button_with_icon_sized(
                theme,
                NEW_SERVER_KEY,
                Glyph::Plus,
                Key::ServersListEmptyNewServerButton,
                ui::Kind::Colored,
                ui::Size::Lg,
                Length::Shrink,
                Some(Message::NewServer),
            ))
            // `AutoLink` with a `size-5` arrow and `gap-1`. A link this launcher
            // cannot follow is drawn as the quiet button it behaves like, and the
            // press says which service it would have asked.
            .push(ui::button_with_icon_sized(
                theme,
                MANAGE_BILLING_KEY,
                Glyph::RightArrow,
                Key::ServersListEmptyLearnMoreLink,
                ui::Kind::Quiet,
                ui::Size::Md,
                Length::Shrink,
                Some(Message::ManageBilling),
            )),
    )
    .into()
}

/// One feature: its plate and its two lines, `items-start gap-4`.
fn feature<'a>(theme: Gen, glyph: Glyph, title: Key, description: Key) -> Element<'a, Message> {
    row![]
        .spacing(FEATURE_GAP_X)
        .align_items(Alignment::Start)
        .push(plate(theme, glyph))
        .push(
            column![]
                .spacing(FEATURE_TEXT_GAP)
                .push(
                    text(title.message())
                        .size(18.0)
                        .line_height(iced::Pixels(24.0))
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_CONTRAST,
                        ))),
                )
                .push(
                    text(description.message())
                        .size(14.0)
                        .line_height(iced::Pixels(20.0))
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_DEFAULT,
                        ))),
                ),
        )
        .into()
}

/// A feature's plate: `size-10 rounded-[0.875rem] border bg-surface-1` with the
/// feature's glyph at `size-5 text-brand`.
///
/// The reference lays two more layers inside the plate -- a `green-800` to
/// `green-950` gradient at half opacity and a shade over it -- and a texture
/// image at 40% through `mix-blend-luminosity`. This has no gradient fill and no
/// texture, so the plate carries the middle of that gradient mixed into the
/// surface instead: measured at the reference's own window the plate reads
/// `(22, 28, 30)` where the page is `(22, 24, 28)`, which is what
/// [`plate_fill`] is that colour.
fn plate<'a, Message: 'a>(theme: Gen, glyph: Glyph) -> Element<'a, Message> {
    container(icon::icon(glyph, GLYPH, theme_gen::ink(theme, Ink::Brand)))
        .width(Length::Fixed(FEATURE_PLATE))
        .height(Length::Fixed(FEATURE_PLATE))
        .center_x()
        .center_y()
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(plate_fill(theme))),
            border: Border {
                color: crate::theme::mix(
                    theme_gen::ink(theme, Ink::TextPrimary),
                    Color::TRANSPARENT,
                    0.10,
                ),
                width: 1.0,
                radius: FEATURE_PLATE_RADIUS.into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

/// The plate's fill: the middle of the reference's gradient, over `--surface-1`.
fn plate_fill(theme: Gen) -> iced::Color {
    crate::theme::mix(
        theme_gen::ink(theme, Ink::Green800),
        theme_gen::ink(theme, Ink::Surface1),
        0.5,
    )
}

/// `ServerListEmptyPreview`: the reference's own picture of the invite dialog,
/// drawn at its own measurements and with its own strings.
///
/// It is `inert aria-hidden` in the reference -- a picture of a dialog, not a
/// dialog -- so nothing here takes a press, and every control below is drawn the
/// way the picture draws it rather than the way it behaves.
fn preview<'a>(theme: Gen) -> Element<'a, Message> {
    let mut list = column![].width(Length::Fill).push(
        container(
            text(text_gen::sharing_invite_players_modal_friends_heading(FRIEND_COUNT))
                .size(14.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        )
        .width(Length::Fill)
        .padding(Padding { top: 0.0, right: 0.0, bottom: 8.0, left: 16.0 }),
    );
    for (name, status) in FRIENDS {
        list = list.push(friend_row(theme, name, status));
    }
    let friends = container(
        list.padding(Padding { top: 12.0, right: 0.0, bottom: 12.0, left: 0.0 }),
    )
    .width(Length::Fill)
    .style(move |_theme: &Theme| container::Appearance {
        // `bg-surface-1`: the picture's list sits on the sunken surface, a step
        // darker than the panel it is in.
        background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface1))),
        ..container::Appearance::default()
    });
    container(
        column![]
            .width(Length::Fill)
            .spacing(0.0)
            .push(preview_head(theme))
            .push(preview_search(theme))
            .push(friends)
            .push(Space::with_height(Length::Fill))
            .push(preview_invite_link(theme)),
    )
    .width(Length::Fixed(PREVIEW))
    .height(Length::Fixed(PREVIEW_HEIGHT))
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface2))),
        border: Border {
            color: theme_gen::ink(theme, Ink::Surface3),
            width: 1.0,
            radius: theme_gen::span(Span::RadiusLg).into(),
        },
        ..container::Appearance::default()
    })
    .into()
}

/// The picture's title row: `p-4` over a `border-surface-3` rule.
fn preview_head<'a>(theme: Gen) -> Element<'a, Message> {
    row![]
        .width(Length::Fill)
        .spacing(PLATE_GAP)
        .align_items(Alignment::Center)
        .padding(16.0)
        .push(icon::icon(
            Glyph::UserPlus,
            GLYPH,
            theme_gen::ink(theme, INK_DEFAULT),
        ))
        .push(
            text(Key::SharingInvitePlayersModalHeading.message())
                .size(18.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(
                    theme,
                    INK_CONTRAST,
                ))),
        )
        .push(Space::with_width(Length::Fill))
        .push(icon::icon(
            Glyph::X,
            GLYPH,
            theme_gen::ink(theme, INK_SECONDARY),
        ))
        .into()
}

/// The picture's search row: the field, then *Add*.
fn preview_search<'a>(theme: Gen) -> Element<'a, Message> {
    row![]
        .width(Length::Fill)
        .spacing(PLATE_GAP)
        .align_items(Alignment::Center)
        .padding(Padding { top: 16.0, right: 16.0, bottom: 16.0, left: 16.0 })
        .push(
            container(
                row![]
                    .spacing(PLATE_GAP)
                    .align_items(Alignment::Center)
                    .push(icon::icon(
                        Glyph::Search,
                        16.0,
                        theme_gen::ink(theme, INK_SECONDARY),
                    ))
                    .push(
                        text(Key::SharingInvitePlayersModalSearchPlaceholder.message())
                            .size(14.0)
                            .font(medium())
                            .style(iced::theme::Text::Color(theme_gen::ink(
                                theme,
                                INK_SECONDARY,
                            ))),
                    ),
            )
            .width(Length::Fill)
            .height(Length::Fixed(36.0))
            .padding(Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 12.0 })
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface3))),
                border: Border { radius: 12.0.into(), ..Border::default() },
                ..container::Appearance::default()
            }),
        )
        .push(ui::button_with_icon_sized(
            theme,
            REFRESH_KEY,
            Glyph::Plus,
            Key::SharingInvitePlayersModalAdd,
            ui::Kind::Colored,
            ui::Size::Md,
            Length::Shrink,
            None,
        ))
        .into()
}

/// One friend: the account's plate, the name, and the button that acts on it.
fn friend_row<'a>(theme: Gen, name: &str, status: FriendStatus) -> Element<'a, Message> {
    let (label, kind) = match status {
        FriendStatus::Added => (Key::SharingInvitePlayersModalAdded, ui::Kind::Standard),
        FriendStatus::Cancel => (Key::SharingInvitePlayersModalCancel, ui::Kind::Outlined),
        FriendStatus::Invite => (Key::SharingInvitePlayersModalInvite, ui::Kind::Standard),
    };
    let button = if status == FriendStatus::Added {
        ui::button_with_icon_sized(
            theme,
            REFRESH_KEY,
            Glyph::Check,
            label,
            kind,
            ui::Size::Md,
            Length::Shrink,
            None,
        )
    } else {
        ui::button_or_sized(theme, REFRESH_KEY, label, kind, ui::Size::Md, None)
    };
    row![]
        .width(Length::Fill)
        .spacing(12.0)
        .align_items(Alignment::Center)
        .height(Length::Fixed(44.0))
        .padding(Padding { top: 0.0, right: 16.0, bottom: 0.0, left: 16.0 })
        .push(
            // The picture's avatar, which is a photograph this tree does not carry;
            // the account glyph stands in at the reference's own 24 pixels.
            container(icon::icon(
                Glyph::CircleUser,
                24.0,
                theme_gen::ink(theme, INK_SECONDARY),
            ))
            .width(Length::Fixed(24.0))
            .height(Length::Fixed(24.0))
            .center_x()
            .center_y(),
        )
        .push(
            text(name.to_string())
                .size(16.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        )
        // `justify-between`: the name against the left and the button against the
        // right, with whatever is between them left empty.
        .push(Space::with_width(Length::Fill))
        .push(button)
        .into()
}

/// The picture's foot: the invite link, under a rule, over `--surface-2`.
fn preview_invite_link<'a>(theme: Gen) -> Element<'a, Message> {
    column![]
        .width(Length::Fill)
        .spacing(PLATE_GAP)
        .padding(16.0)
        .push(
            text(Key::SharingInvitePlayersModalInviteLinkHeading.message())
                .size(16.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(
                    theme,
                    INK_CONTRAST,
                ))),
        )
        .push(
            row![]
                .width(Length::Fill)
                .spacing(PLATE_GAP)
                .align_items(Alignment::Center)
                .height(Length::Fixed(32.0))
                .padding(Padding { top: 0.0, right: 10.0, bottom: 0.0, left: 10.0 })
                .push(
                    text("https://modrinth.com/server/abc123")
                        .size(14.0)
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_DEFAULT,
                        ))),
                )
                .push(Space::with_width(Length::Fill))
                .push(icon::icon(
                    Glyph::ClipboardCopy,
                    16.0,
                    theme_gen::ink(theme, INK_SECONDARY),
                )),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_says_the_service_it_needs_rather_than_that_it_is_unbuilt() {
        // The distinction this page exists to keep now: a Modrinth Hosting action
        // is not waiting for a slice, it is waiting for an account this launcher
        // does not hold. A notice that read "is not implemented yet" would be a
        // promise nothing is keeping.
        let mut state = State::default();
        for message in [Message::NewServer, Message::ManageBilling, Message::Refresh] {
            state.update(message);
            let notice = state.notice.clone().expect("a notice");
            assert!(notice.contains("Modrinth account"), "{notice}");
            assert!(!notice.contains("is not implemented yet"), "{notice}");
        }
    }

    #[test]
    fn the_page_draws_the_empty_state_in_every_theme_with_and_without_a_notice() {
        let store = Store::default();
        for theme in Gen::ALL {
            for notice in [None, Some("x".to_string())] {
                let state = State { notice };
                drop(view(*theme, &state, &store));
            }
        }
    }

    #[test]
    fn the_columns_are_the_reference_s_own_measures() {
        // `max-w-[20rem]` and `max-w-[25rem]`, and the free space their row's four
        // `mx-auto` margins split at the reference's own 868-pixel content column:
        // 868 - 320 - 400 - 8 = 140, so 35 a margin, which is `MARGIN_SHARE`'s
        // first and last thirds and its 43 for the middle margin plus the gap.
        assert_eq!(COLUMN, 20.0 * 16.0);
        assert_eq!(PREVIEW, 25.0 * 16.0);
        assert_eq!(PREVIEW_HEIGHT, 38.0 * 16.0);
        assert_eq!(ROW_GAP, 2.0 * 4.0);
        assert_eq!(COLUMN_GAP, 8.0 * 4.0);
        assert_eq!(FEATURE_GAP, 6.0 * 4.0);
        assert_eq!(FEATURE_GAP_X, 4.0 * 4.0);
        assert_eq!(FEATURE_TEXT_GAP, 0.5 * 4.0);
        assert_eq!(ACTION_GAP, 4.0 * 4.0);
        assert_eq!(FEATURE_PLATE, 10.0 * 4.0);
        let free = 868.0 - COLUMN - PREVIEW - ROW_GAP;
        assert_eq!(free, 140.0);
        assert_eq!(free / 4.0, 35.0);
        assert_eq!(MARGIN_SHARE[0], 35);
        assert_eq!(MARGIN_SHARE[2], 35);
        assert_eq!(MARGIN_SHARE[1] as f32, 35.0 + ROW_GAP + 35.0);
        // And the three of them are exactly the free space, which is what puts the
        // column's first ink at 123 and the preview's at 521 rather than near them.
        let spacers: f32 = MARGIN_SHARE.iter().map(|share| *share as f32).sum();
        assert_eq!(spacers, 148.0);
        assert_eq!(spacers, free + ROW_GAP);
    }

    #[test]
    fn the_features_are_the_reference_s_three_in_its_own_order() {
        assert_eq!(FEATURES.len(), 3);
        assert_eq!(FEATURES[0].1, Key::ServersListEmptyOneClickModInstallsTitle);
        assert_eq!(FEATURES[1].1, Key::ServersListEmptySimpleSetupTitle);
        assert_eq!(FEATURES[2].1, Key::ServersListEmptyPlayWithFriendsTitle);
        // The descriptions are the reference's own sentences, so the columns wrap
        // where the reference's wrap. Each is one key and never a literal.
        for (_, _, description) in FEATURES {
            assert!(description.message().ends_with('.'), "{:?}", description);
        }
    }

    #[test]
    fn the_preview_lists_the_reference_s_own_friends_and_statuses() {
        // The picture is the reference's: its names, its three button states, and
        // the count its heading quotes -- which is 11 while eight rows are drawn,
        // exactly as the reference draws it.
        assert_eq!(FRIENDS.len(), 8);
        assert_eq!(FRIEND_COUNT, "11");
        assert_eq!(FRIENDS[0], ("Josh", FriendStatus::Added));
        assert_eq!(FRIENDS[2], ("Fetch", FriendStatus::Cancel));
        assert_eq!(FRIENDS.iter().filter(|(_, status)| *status == FriendStatus::Invite).count(), 6);
        assert_eq!(FRIENDS.iter().filter(|(_, status)| *status == FriendStatus::Added).count(), 1);
        assert_eq!(FRIENDS.iter().filter(|(_, status)| *status == FriendStatus::Cancel).count(), 1);
    }
}
