//! Skins, the reference's third rail slot: `pages/Skins.vue`.
//!
//! The page is a gallery of *sections* -- the bundles the reference ships, whose
//! names are its own keys -- over a preview of the skin in force. Both halves need
//! a service: the bundles come from Modrinth's skin store and the preview from a
//! rendered skin, so what this page draws today is the section list and the states
//! around it, with the sentence that says where the rest comes from rather than a
//! grid of empty boxes.
//!
//! What *is* real is the account's own half: the skins and capes Minecraft says it
//! owns, the skin in force cut into a front view (G104), the ability to put any of
//! them on (G106), and -- since G123 -- the ability to add one from a file: the
//! saved section's add cell opens the launcher's own file dialog, the chosen
//! texture is padded to the 64x64 the service takes and its arm style is read
//! from its own pixels, and the upload is one multipart write (`crate::pick`,
//! `crate::skin`, `Ask::AddSkin`).
//!
//! Three of those are a *write* to the reader's own Minecraft account -- putting a skin
//! on, putting a cape on, and adding the file -- so what a press will do is written
//! where the press is. Since G124 the two things this page had none of are here: the
//! *store* of the skins the reader has added ([`crate::saved_skins`], a folder under
//! this product's own directory, which the Saved-skins section is drawn from), and the
//! editor its rows open. That editor is the reference's `EditSkinModal.vue` -- the
//! arm-style choice, the cape choice, and the Ears notice with its link to project
//! `mfzaZK3Z` -- with two differences, each written where it is drawn: the reference's
//! own `unequip_skin` has no caller anywhere in the vendored frontend, so taking a skin
//! off is reached from this launcher's editor rather than from the reference's, and the
//! deletion the reference draws in a confirm dialog of its own lives in the editor too,
//! because this page has no preview panel to put it on ([`edit_view`]). Since G134 the
//! rows can be put in the reader's own order: the reference drags a saved skin anywhere
//! in its list (`VirtualSkinSectionList.vue`'s `reorder-saved-skins`, which `Skins.vue`
//! answers with `set_custom_skin_order`), and a row here carries a chevron up and a
//! chevron down instead, because this launcher has no drag widget -- the write
//! underneath is the same one ([`Step`], [`Reorder`]).

use iced::widget::{column, container, image, mouse_area, row, Space};
use iced::{Alignment, ContentFit, Element, Length, Padding, Theme};
use palantir_net::SkinChange;

use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP, ROW_GAP};
use crate::pages::{Ask, Open};
use crate::saved_skins;
use crate::skin::Appearance;
use crate::store::Store;
use crate::style::{INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Ink, Theme as Gen};
use crate::ui::{self, text};

/// The reference's sections, in the order its page lays them out.
///
/// `Skins.vue` builds the list as the reader's own saved skins first, then the
/// pack sections sorted by `getDefaultSkinSectionSortIndex`, whose
/// `DEFAULT_SKIN_SECTION_SORT_ORDER` is `['Default skins', 'Modrinth Pride']`
/// and whose ties keep the order the store listed them in. The names here are
/// the locale's, so the section headings are the reference's words rather than
/// a paraphrase of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// The skins a new account starts with.
    DefaultSkins,
    /// Modrinth's own.
    Modrinth,
    /// The pride bundle.
    ModrinthPride,
    /// The user's own saved skins.
    SavedSkins,
    /// Builders and Biomes.
    BuildersAndBiomes,
    /// Chaos Cubed.
    ChaosCubed,
    /// Chase the Skies.
    ChaseTheSkies,
    /// Minecon Earth 2017.
    MineconEarth2017,
    /// Mounts of Mayhem.
    MountsOfMayhem,
    /// Striding Hero.
    StridingHero,
    /// The Copper Age.
    TheCopperAge,
    /// The Garden Awakens.
    TheGardenAwakens,
    /// Tiny Takeover.
    TinyTakeover,
}

impl Section {
    /// Every section, in the reference's order.
    pub const ALL: [Section; 13] = [
        Section::SavedSkins,
        Section::DefaultSkins,
        Section::ModrinthPride,
        Section::Modrinth,
        Section::BuildersAndBiomes,
        Section::ChaosCubed,
        Section::ChaseTheSkies,
        Section::MineconEarth2017,
        Section::MountsOfMayhem,
        Section::StridingHero,
        Section::TheCopperAge,
        Section::TheGardenAwakens,
        Section::TinyTakeover,
    ];

    /// The reference's key for this section's heading.
    pub const fn key(self) -> Key {
        match self {
            Section::DefaultSkins => Key::AppSkinsSectionDefaultSkins,
            Section::Modrinth => Key::AppSkinsSectionModrinth,
            Section::ModrinthPride => Key::AppSkinsSectionModrinthPride,
            Section::SavedSkins => Key::AppSkinsSectionSavedSkins,
            Section::BuildersAndBiomes => Key::AppSkinsSectionBuildersAndBiomes,
            Section::ChaosCubed => Key::AppSkinsSectionChaosCubed,
            Section::ChaseTheSkies => Key::AppSkinsSectionChaseTheSkies,
            Section::MineconEarth2017 => Key::AppSkinsSectionMineconEarth2017,
            Section::MountsOfMayhem => Key::AppSkinsSectionMountsOfMayhem,
            Section::StridingHero => Key::AppSkinsSectionStridingHero,
            Section::TheCopperAge => Key::AppSkinsSectionTheCopperAge,
            Section::TheGardenAwakens => Key::AppSkinsSectionTheGardenAwakens,
            Section::TinyTakeover => Key::AppSkinsSectionTinyTakeover,
        }
    }

    /// The heading, as the reference writes it.
    pub fn label(self) -> &'static str {
        self.key().message()
    }
}

/// The request the page makes: read the account's own appearance.
///
/// Nothing but the round travels. *Which* account is the shell's to know -- it is
/// the one a launch would sign in as, and a page has never seen an account file or
/// a token -- so what a page asks is the question, and the shell is what can answer
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Which request this is, counting from one.
    pub round: u64,
}

/// A change to what the account wears, asked for by this page and made by the shell.
///
/// The same shape as [`Asked`], one step further out: the *change* is this page's to
/// describe -- the reader pressed a row, and the row knows which skin or cape it
/// is -- and the account, the token and the request are the shell's, for [`Asked`]'s
/// reason. The round travels so the answer can be matched to the change that is
/// actually waiting for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wear {
    /// Which request this is, counting from one, beside the reads' own counter.
    pub round: u64,
    /// What to change.
    pub change: SkinChange,
}

/// The request that opens the launcher's file dialog and uploads what it returns.
///
/// The third shape in this file, and the one where the page has the least: the path
/// comes from a dialog and the bytes from a file, so neither is a page's to describe --
/// what travels is the round and nothing else, and the change is built where the file
/// is read. The round is here for [`Wear`]'s reason: the answer has to be matchable to
/// the press that is waiting for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Add {
    /// Which request this is, counting from one, beside the reads' and writes' own
    /// counters.
    pub round: u64,
}

/// The order the reader asked the saved rows to be in.
///
/// The whole order rather than a move, because that is the write: the reference's
/// `set_custom_skin_order` takes the list of texture keys and
/// [`crate::saved_skins::reorder`] takes the same. The page computes it from the rows
/// it is drawing -- the only list it can see -- and the store ignores keys it does not
/// hold and keeps rows it was not told about, so an order that arrives short cannot
/// delete anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reorder {
    /// Which request this is, counting from one, beside the reads', the writes' and
    /// the editor's own counters.
    pub round: u64,
    /// Every saved row's key, in the order the reader put them in.
    pub keys: Vec<String>,
}

/// What came of asking the reader for a file.
///
/// Three answers rather than a `Result`, because a cancel is not a failure: a reader
/// who opened the dialog and changed their mind is a finished interaction, and a page
/// that apologised for it would be apologising for nothing. The third is not the
/// reader's doing either -- a build with no picker in it -- which is why it is its own
/// arm rather than a sentence in the second's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    /// The reader closed the dialog without choosing anything.
    Cancelled,
    /// A texture was chosen and the account wears it -- or would not, with the
    /// reason the service gave.
    Done(Result<(), String>),
    /// This machine has no dialog to open at all.
    NoPicker(String),
}

/// One skin the launcher has stored, as the page draws it.
///
/// The store's own row (`crate::saved_skins`) plus the one thing the page cannot
/// work out for itself: whether the texture asks for Ears features. That answer is
/// in the PNG, which a page never holds, so the shell reads it when it reads the
/// store and hands the row over with the flag already in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedRow {
    /// The row, as the store read it.
    pub entry: saved_skins::Entry,
    /// Whether the stored texture carries the Ears mod's marker.
    pub ears: bool,
}

/// Everything the page's own read answers with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    /// The account's own appearance, from Minecraft's document.
    pub appearance: Appearance,
    /// The skins the reader has added, in the reader's order.
    pub saved: Vec<SavedRow>,
}

/// Which of the edit modal's three actions a press asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Write the arm style and cape onto the row, and put the skin on.
    Save,
    /// Forget the row and its pixels -- the reference's `remove_custom_skin`.
    Forget,
    /// Take the account's skin off altogether.
    TakeOff,
}

/// An edit of one stored skin, as the modal is making it.
///
/// The state of the modal rather than a request: which row, and the two choices
/// the reader has made about it. It becomes an ask when one of the actions is
/// pressed, and the *round* it carries is the one the page will match the answer
/// against -- the modal opens inside a round rather than starting one, because
/// opening it asks nobody anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Which request this belongs to, so an answer is matched to the press waiting
    /// for it.
    pub round: u64,
    /// The stored row's key, which is what the shell reads its pixels by.
    pub key: String,
    /// What the row is called, for the modal's title.
    pub name: String,
    /// The arm style the reader has chosen, in the service's own words.
    pub variant: String,
    /// The cape the reader has chosen: a document id, or empty for none.
    pub cape: String,
    /// The row as the store holds it, which is what "an edit" is measured against.
    ///
    /// `EditSkinModal.vue` keeps `currentSkin` beside the modal's own `variant` and
    /// `selectedCape`, and its `hasEdits` is the difference between them. This
    /// carries the store's half so the editor can be drawn with Save already
    /// answering the same question, which is the whole of `hasEdits` once the
    /// texture section is not there (departure 15).
    pub stored: Stored,
    /// Whether the stored texture asks for Ears features.
    ///
    /// Carried because it is the row's own fact, read with the rest of it in one
    /// place. The reference draws its Ears notice in the preview pane's subtitle
    /// rather than in this dialog, and so does this page -- under the model, for
    /// the skin in force -- so nothing in the editor reads this yet.
    pub ears: bool,
    /// What the reader has asked for.
    pub act: Act,
    /// Whether the reader has been asked to confirm the deletion.
    ///
    /// The reference puts a `ConfirmModal` between the delete button and
    /// `remove_custom_skin`, with the question as its title and "This will
    /// permanently delete the selected skin. This action cannot be undone." as
    /// its description. A dialog inside a dialog is the shell's, so the question
    /// is drawn in this body instead and the flag is what says which of the two
    /// the body is.
    pub confirm: bool,
}

/// The row an editor opened on, as the store holds it.
///
/// The two fields `EditSkinModal.vue` compares against: `currentSkin.variant`
/// and `currentSkin.cape_id`, folded to an empty string when the row wears no
/// cape, which is what `(selectedCape?.id || null) !== (currentSkin.cape_id ||
/// null)` compares on its own side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stored {
    /// The arm style the row was stored with, in the service's own words.
    pub variant: String,
    /// The cape the row was stored with: a document id, or empty for none.
    pub cape: String,
}

/// Which way a saved row was asked to move.
///
/// The reference reorders its saved skins by dragging one anywhere in the list
/// (`VirtualSkinSectionList.vue`'s `reorder-saved-skins`, which `Skins.vue` answers
/// with `set_custom_skin_order`). This launcher has no drag widget -- [`crate::ui`] is
/// buttons, chips and fields -- so a row moves one place per press and a reader
/// reaches any order by repeating, which is this launcher's control rather than the
/// reference's. The write underneath is the same one either way: the whole order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Toward the front of the list.
    Up,
    /// Toward the back.
    Down,
}

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// A section was opened.
    Select(Section),
    /// The page was asked to add a skin from a file.
    AddSkin,
    /// What came of asking the reader for a file.
    Added {
        /// Which request this answers, so an answer to one the page has replaced is
        /// dropped rather than drawn.
        round: u64,
        /// The reader's answer, or the machine's.
        picked: Picked,
    },
    /// The reader asked to put something the account owns on -- or take it off.
    Wear(SkinChange),
    /// The shell's answer to a change: which change it answers, and what happened.
    ///
    /// A failure is a sentence in the same slot every other thing this page could
    /// not do goes; a success says nothing, because what it changed is drawn two
    /// rows down by the reload that follows it.
    Applied {
        /// Which change this answers.
        round: u64,
        /// Nothing on success, or the reason it could not be made.
        result: Result<(), String>,
    },
    /// The account's own appearance arrived.
    ///
    /// Boxed at the crossing, for the reason `project::Message::Found` is: this is
    /// the one variant that is a page's worth of data beside a dozen unit ones, and
    /// boxing it here keeps the page's own arms from boxing anything.
    Found {
        /// Which request this answers, so an answer to a question the page has
        /// replaced is dropped rather than drawn.
        round: u64,
        /// The account's appearance, or why it could not be read.
        result: Result<Box<Loaded>, String>,
    },
    /// A stored row was pressed: open the edit modal on it.
    Edit {
        /// The row's key.
        key: String,
    },
    /// A stored row was asked to move one place in the reader's own order.
    Move {
        /// The row's key.
        key: String,
        /// Which way -- one place, which is what the control offers.
        step: Step,
    },
    /// The arm style was chosen in the modal.
    ArmStyle {
        /// `CLASSIC` or `SLIM`, the two words the service uses.
        variant: &'static str,
    },
    /// A cape was chosen in the modal -- or none was.
    Cape {
        /// The cape's own id, or empty for none.
        id: String,
    },
    /// The editor's own delete button was pressed, which is a question and not a
    /// write.
    ///
    /// The reference's `deleteSkin` is reached through a `ConfirmModal` and not
    /// by pressing the trash once, so this press only asks; [`Act::Forget`] is
    /// what the write answers to.
    Forget,
    /// The reader changed their mind about deleting the skin.
    ///
    /// The reference's confirm is its own modal over the editor, so its Cancel
    /// leaves the editor exactly as it was; this one goes back to the editor
    /// rather than closing it, which is why it is not [`Message::CloseEdit`].
    CancelForget,
    /// The editor's own texture section was asked to replace the row's file.
    ///
    /// The reference's press opens a file browser over a hidden
    /// `<input type="file" accept="image/png">` and hands the chosen file to
    /// `onTextureFileInputChange`, which normalises it and writes it onto the skin
    /// it is editing. The write underneath is the shell's, and the ask this page
    /// can raise for a file is [`Ask::AddSkin`] -- which *adds* a row rather than
    /// replacing one, so a press here would leave the reader with two of the skin
    /// they were editing. The control is drawn anyway, with its own words, and
    /// says what it cannot do, which is the same answer the sign-in button gives.
    ReplaceTexture,
    /// The notice's own link was pressed: open the Ears mod's project page.
    OpenEars,
    /// The demo banner's button was pressed: sign in to Minecraft.
    ///
    /// The reference's `login()`, which opens its sign-in modal. That flow is a
    /// later stage here and it belongs to the shell -- the shell's own account
    /// card asks for it -- but a page cannot ask for it, so the press is the
    /// page's and what it says is the page's own sentence. Pressing it must not
    /// be silent: the reference's button opens a window, and a button that
    /// swallows its press is a button that looks like it is broken.
    SignIn,
    /// The modal was dismissed without doing anything.
    CloseEdit,
    /// The modal asked for one of its three actions.
    Act(Act),
    /// The shell's answer to what the modal asked for.
    Edited {
        /// Which request this answers.
        round: u64,
        /// Nothing on success, or the reason it could not be done.
        result: Result<(), String>,
    },
    /// The shell's answer to a move: which request it answers, and what happened.
    ///
    /// A success says nothing and reloads instead, for [`Message::Applied`]'s
    /// reason: the order the store now holds is what the page draws again. A failure
    /// is a sentence in the page's own slot, and the order still on screen is the
    /// one the store holds -- the write is one atomic file, so a refusal wrote
    /// nothing to re-read.
    Reordered {
        /// Which request this answers.
        round: u64,
        /// Nothing on success, or the reason the order could not be written.
        result: Result<(), String>,
    },
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

/// The one control in the page's header. The rows carry the other action, and their
/// hover names come from [`ui::scoped`] because they repeat.
const ADD_KEY: &str = "skins:add";

/// The Ears mod's project, as the reference's own notice links it.
///
/// `pages/Skins.vue` writes `to="/project/mfzaZK3Z"` where it draws the notice, and
/// this launcher's project page takes the same id -- the reference's link is its
/// router, and this one is the route the shell already has.
pub const EARS_PROJECT: &str = "mfzaZK3Z";

/// The name space the stored rows' editors take theirs from. One per row, keyed by
/// the texture's digest, because two rows can share a file name.
const EDIT_KEY: &str = "skins:edit";

/// The same for the two controls that move a stored row, one space each because a
/// row carries both and each is its own control to the hover clock.
const MOVE_UP_KEY: &str = "skins:move-up";
const MOVE_DOWN_KEY: &str = "skins:move-down";

/// The chevrons' own size: the reference's `!size-4` on the head's buttons, which
/// is the size it draws a small icon at (`HEAD_ICON`).
const MOVE_MARK: f32 = 16.0;

/// The name space the per-row Apply buttons take their hover names from.
const WEAR_KEY: &str = "skins:wear";

/// The front view's own size on the page: the texture's 16x32 at six times, which
/// is as large as the reference draws its model and the only kind of size that
/// does not blur a face that is eight pixels wide.
const DOLL_WIDTH: f32 = 96.0;
const DOLL_HEIGHT: f32 = 192.0;



/// The top or bottom edge of the add cell's dashed ring: [`DASHES_ACROSS`]
/// dashes and the gaps between them, the dash one share of the six-pixel
/// period and the gap two.
///
/// One pixel tall, which is the border's own width: a `border` on the cell
/// would have painted the same line in the same place.
fn dash_row<'a>(theme: Gen) -> Element<'a, Message> {
    let ink = theme_gen::ink(theme, Ink::Surface5);
    let mut strip = row![].width(Length::Fill).height(Length::Fixed(1.0));
    for _ in 0..DASHES_ACROSS {
        strip = strip
            .push(
                container(Space::with_height(Length::Fixed(1.0)))
                    .width(Length::FillPortion(1))
                    .style(move |_theme: &Theme| container::Appearance {
                        background: Some(iced::Background::Color(ink)),
                        ..container::Appearance::default()
                    }),
            )
            .push(Space::with_width(Length::FillPortion(2)));
    }
    strip.into()
}

/// The left or right edge of the same ring: [`DASHES_DOWN`] of them, a pixel
/// wide.
fn dash_column<'a>(theme: Gen) -> Element<'a, Message> {
    let ink = theme_gen::ink(theme, Ink::Surface5);
    let mut strip = column![].height(Length::Fill).width(Length::Fixed(1.0));
    for _ in 0..DASHES_DOWN {
        strip = strip
            .push(
                container(Space::with_width(Length::Fixed(1.0)))
                    .height(Length::FillPortion(1))
                    .style(move |_theme: &Theme| container::Appearance {
                        background: Some(iced::Background::Color(ink)),
                        ..container::Appearance::default()
                    }),
            )
            .push(Space::with_height(Length::FillPortion(2)));
    }
    strip.into()
}

/// The demo banner's own button. One control, so one name for the hover
/// clock -- the page's rows take theirs from [`ui::scoped`] because they repeat
/// and this one does not.
const SIGN_IN_KEY: &str = "skins:sign-in";

/// The mark beside the skin or cape that is in force.
const WORN_MARK: f32 = 14.0;

/// The page's own inset: `Skins.vue`'s root is `p-4` (16), where the library
/// pages are `p-6`.
const PAGE_INSET: f32 = 16.0;

/// The two columns' gap: `skin-layout`'s `gap` is `2.5rem` (40).
const COLUMN_GAP: f32 = 40.0;

/// The left column's own padding: `p-2` -- 8 on every side.
///
/// The template writes `p-2 pt-0`, and this is 8 above where that reads. A
/// capture settles it: the reference's title glyphs are at y 78..95 with the
/// page's own content top at 63, which is an eight-pixel offset from its
/// content box -- the same eight the section list's own `pt-2 pt-1` puts its
/// first header box at y75. With no top padding the title would sit at y71,
/// seven pixels above the reference's, and the preview box under it would land
/// at 111 rather than the 119 its `mt-4` gives the reference.
const COLUMN_PAD: f32 = 8.0;

/// The grid's two shares, from `grid-template-columns: minmax(0, 1fr)
/// minmax(0, 2.5fr)`: portions 2 and 5 are the same 1:2.5 at any width, which
/// is why they are not written as pixel widths.
const PREVIEW_PORTION: u16 = 2;
const LIST_PORTION: u16 = 5;

/// The title's size: `text-2xl` (24) over the reference's own `font-bold`, which
/// is why it is not drawn by [`page::title`] -- that one is `font-extrabold`.
const TITLE_SIZE: f32 = 24.0;

/// The preview box: `ml-5 mt-4 h-[calc(80vh-1rem)]` around a centred doll --
/// 20 from the left of the column, 16 below the title, and 576 - 16 = 560 tall
/// at this shell's 720-pixel window.
const PREVIEW_LEFT: f32 = 20.0;
const PREVIEW_TOP: f32 = 16.0;
const PREVIEW_HEIGHT: f32 = 560.0;

/// The section list: `pt-2` (8) above its first row, then `pt-1` (4) on the
/// first section and `pt-6` (24) on every other one.
const LIST_TOP: f32 = 8.0;
const SECTION_FIRST_TOP: f32 = 4.0;
const SECTION_TOP: f32 = 24.0;

/// A section's header row: `size-6` on the chevron (24), `gap-[6px]` after it,
/// and the title's `text-xl font-semibold leading-7` -- 20 over 28.
const SECTION_ICON: f32 = 24.0;
const SECTION_ICON_GAP: f32 = 6.0;
const SECTION_TITLE: f32 = 20.0;
const SECTION_HEIGHT: f32 = 28.0;

/// `content-class="pt-2"` (8) between a header and its cards.
const SECTION_CONTENT_TOP: f32 = 8.0;

/// The cards: `aspect-[31/40]`, `rounded-[20px]`, in a `grid-cols-3 gap-3` --
/// three columns and 12 between any two cells.
const CARD_COLUMNS: usize = 3;
const CARD_GAP: f32 = 12.0;
const CARD_RADIUS: f32 = 20.0;

/// The cells' height at this shell's own pane. The window is 1280, the rail 64
/// and the panel 300, so the page is 916; `p-4` leaves 884, the 40 between the
/// columns leaves 844, and the list's 2.5/3.5 share is 602.9. A cell is
/// (602.9 - 2 * 12) / 3 = 193.0 wide, and the reference's own `aspect-[31/40]`
/// makes it 193.0 * 40/31 = 249.0 tall. The width is a `FillPortion` so the
/// columns stay equal whatever the pane does; the height cannot be, because
/// iced has no aspect-ratio, which is the one number here a resize would
/// change.
const CARD_HEIGHT: f32 = 249.0;

/// A card's own padding around its picture and footer. The reference's
/// `SkinButton` carries none -- its picture is the whole card -- but its name
/// is a tooltip and this launcher has no tooltip widget, so the name is a line
/// under the picture and the card needs the room for it.
const CARD_INSET: f32 = 8.0;

/// The plus in the saved section's first cell: `size-8`.
const ADD_ICON: f32 = 32.0;

/// The add cell's own side padding, `px-3`, which the dashed ring does not
/// change: the words are three pixels in from the *cell*, and the ring is the
/// pixel the border was.
const ADD_CARD_PAD: f32 = 12.0;

/// The two lines' own heights: `leading-6` (24) over `leading-5` (20). Left to
/// itself, iced gives a 16-pixel label a 19.2-pixel line and a 14-pixel
/// subtitle a 16.8 one, which makes the group five pixels shorter than the
/// reference's 94 -- and a group centred in the cell moves its top down with
/// it: the capture reads the plus at y207 here against the reference's 203.
const ADD_LABEL_LINE: f32 = 24.0;
const ADD_SUBTITLE_LINE: f32 = 20.0;



/// The demo banner's own padding: `p-4 pt-0` on the block that holds it --
/// 16 on both sides and under it, none above, because the block sits against
/// the pane it is stuck to.
const BANNER_PAD: f32 = 16.0;

/// The banner's box: `rounded-[20px] border border-surface-5 bg-surface-3 p-4`.
const BANNER_RADIUS: f32 = 20.0;
const BANNER_INSET: f32 = 16.0;

/// `max-w-5xl` -- 1024. Wider than this shell's pane at any size it is
/// opened at, so the box is the pane's own width less its own padding, which
/// is what the reference's is at 1280 as well (916 - 32 = 884 of 1024).
const BANNER_MAX: f32 = 1024.0;

/// `gap-3` (12) between the icon and the words, and `gap-1` (4) between the two
/// lines of words (`flex-col gap-1`).
const BANNER_GAP: f32 = 12.0;
const BANNER_LINE_GAP: f32 = 4.0;

/// `size-6` on the `InfoIcon` that opens the banner, in `text-blue`.
const BANNER_ICON: f32 = 24.0;

/// The two lines of words: `text-lg font-semibold leading-6` over `text-base
/// leading-6` -- 18 and 16 on a 24-pixel line.
const BANNER_TITLE: f32 = 18.0;
const BANNER_LINE: f32 = 24.0;
const BANNER_DESCRIPTION: f32 = 16.0;

/// `gap-4` between that icon and the words under it, and `gap-0.5` (2)
/// between the two lines themselves.
const ADD_ICON_GAP: f32 = 16.0;
const ADD_LINE_GAP: f32 = 2.0;

/// The add cell's dashed edge, in the proportions the reference's own cell
/// draws it: reading down the reference's left border at x361 from y131 gives
/// `##....` over and over, two pixels of `--surface-5` and four of nothing --
/// Chromium's dash pattern for a one-pixel dashed edge. iced's `Border` has no
/// `style` to carry `border-dashed`, so the ring is drawn as what it is: four
/// strips of dashes over the cell's own background.
const DASH_ON: f32 = 2.0;
const DASH_OFF: f32 = 4.0;
/// The period those two make, six pixels, and the share of it a dash is:
/// `DASH_ON / (DASH_ON + DASH_OFF)`, which is the one-and-two the strips below
/// are built from.
const DASH_PERIOD: f32 = DASH_ON + DASH_OFF;
/// How many dashes across: 32 over the 190.3-pixel cell this shell's pane gives
/// (the same arithmetic as [`CARD_HEIGHT`]'s), which is a 5.95-pixel period
/// against the six a capture reads.
const DASHES_ACROSS: usize = 32;
/// And down: 41 over the 247 pixels inside the cell's own 249, a 6.02 period.
const DASHES_DOWN: usize = 41;



/// Which sections are open, in [`Section::ALL`]'s order.
///
/// A flag per section rather than the one-open accordion this page carried
/// before: the reference keeps a *set* of open keys and puts every section it
/// knows into it on the first pass (`VirtualSkinSectionList.vue`'s watch over
/// `sections` with `immediate: true`), so its page opens with every section
/// expanded and its headers toggle one section at a time.
///
/// Not called `Open`: `crate::pages::Open` is the navigation this page already
/// asks for (`Ask::Open`), and one name for both would be a shadow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded([bool; Section::ALL.len()]);

impl Default for Expanded {
    /// Every section open, which is the state the reference's own first pass
    /// leaves it in.
    fn default() -> Self {
        Expanded([true; Section::ALL.len()])
    }
}

impl Expanded {
    /// Whether `section` is open. A section the list does not carry is closed
    /// rather than a panic: the array is the list's own length, and a section
    /// added without one here would be a compile error at `Section::ALL`.
    pub fn is_open(&self, section: Section) -> bool {
        Section::ALL
            .iter()
            .position(|candidate| *candidate == section)
            .and_then(|index| self.0.get(index).copied())
            .unwrap_or(false)
    }

    /// Toggle one section, leaving every other one where it is.
    pub fn toggle(&mut self, section: Section) {
        if let Some(index) = Section::ALL.iter().position(|candidate| *candidate == section) {
            if let Some(flag) = self.0.get_mut(index) {
                *flag = !*flag;
            }
        }
    }
}

/// The page's own state.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Which sections are open.
    pub open: Expanded,
    /// The last thing the page could not do, shown rather than swallowed.
    pub notice: Option<String>,
    /// The account's own appearance: what Minecraft says it owns, and the skin in
    /// force cut into a front view. A `Load`, because it is a service's answer and
    /// the page is drawn before it arrives.
    pub appearance: Load<Appearance>,
    /// Which request the page is waiting for, so an answer to a question it has
    /// replaced is dropped rather than drawn.
    round: u64,
    /// Whether a change the reader asked for is still on its way.
    ///
    /// The rows draw their Apply buttons unusable while it is: a reader who pressed
    /// one and saw nothing happen would press it again, and the second press would
    /// be a second write to their own account.
    pub wearing: bool,
    /// The skins the reader has added, in the reader's order.
    ///
    /// Read from the launcher's own store rather than from a service: it arrives
    /// with [`State::appearance`] because the shell reads both in one turn, and a
    /// page that asked twice would draw its rows and its doll a frame apart.
    pub saved: Vec<SavedRow>,
    /// The edit the modal is making, when it is open. `None` is a closed modal,
    /// which is what the shell draws from: the editor belongs to the page that
    /// owns the row, and the shell is what puts it over the window.
    pub edit: Option<Edit>,
}

impl State {
    /// Apply a message, and answer with what the shell has to do about it.
    ///
    /// An [`Ask`] rather than this page's own [`Asked`], for the project page's
    /// reason: this page asks for two kinds of thing now -- read me the account's
    /// own appearance, and change what it wears -- and only one of them is a
    /// question.
    pub fn update(&mut self, message: Message) -> Option<Ask> {
        match message {
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

            Message::Select(section) => self.open.toggle(section),
            Message::AddSkin => {
                // One at a time, like a row's Apply and for a stronger reason: the
                // dialog this opens is modal, so a second press would be a second
                // dialog behind the first.
                if !self.wearing {
                    self.wearing = true;
                    self.notice = None;
                    let round = self.round + 1;
                    self.round = round;
                    return Some(Ask::AddSkin(Add { round }));
                }
            }
            Message::Added { round, picked } => {
                if round == self.round {
                    self.wearing = false;
                    match picked {
                        // The reader changed their mind: the press is finished and
                        // there is nothing to say about it.
                        Picked::Cancelled => {}
                        // A texture that went up reloads the lists, for the reason a
                        // change that worked does: what changed is the document, and
                        // the new row is a better confirmation than a sentence about
                        // it would be.
                        Picked::Done(Ok(())) => return Some(Ask::Skins(self.ask())),
                        Picked::Done(Err(reason)) | Picked::NoPicker(reason) => {
                            self.notice = Some(reason)
                        }
                    }
                }
            }
            Message::Wear(change) => {
                // The sentence about the last thing this page could not do is about
                // a press this one replaces, and the change is on its way with the
                // round that will tell its answer from a stale one.
                if !self.wearing {
                    self.wearing = true;
                    self.notice = None;
                    let round = self.round + 1;
                    self.round = round;
                    return Some(Ask::Wear(Wear { round, change }));
                }
            }
            Message::Applied { round, result } => {
                if round == self.round {
                    self.wearing = false;
                    match result {
                        // A success says nothing and reloads instead: what changed
                        // is the document's answer, so the answer is what is drawn
                        // again -- the check moves to the row that is now in force,
                        // which is a better confirmation than a sentence about it.
                        Ok(()) => return Some(Ask::Skins(self.ask())),
                        Err(reason) => self.notice = Some(reason),
                    }
                }
            }
            Message::Found { round, result } => {
                if round == self.round {
                    self.appearance = match result {
                        Ok(loaded) => {
                            // The rows and the doll arrive together and are drawn
                            // together; the store's half is moved out of the box
                            // before the appearance is.
                            let loaded = *loaded;
                            self.saved = loaded.saved;
                            Load::Ready(loaded.appearance)
                        }
                        Err(reason) => Load::Failed(reason),
                    };
                }
            }
            Message::Edit { key } => {
                // Opening the editor asks nobody anything -- the row's own facts are
                // in the store this page is already drawing -- so it raises no ask
                // and starts no round: the round it carries is the one the write it
                // leads to will be answered by.
                if let Some(row) = self.saved.iter().find(|row| row.entry.key == key) {
                    self.notice = None;
                    self.edit = Some(Edit {
                        round: self.round,
                        key: row.entry.key.clone(),
                        name: row.entry.name.clone(),
                        variant: row.entry.variant.clone(),
                        cape: row.entry.cape.clone(),
                        stored: Stored {
                            variant: row.entry.variant.clone(),
                            cape: row.entry.cape.clone(),
                        },
                        ears: row.ears,
                        act: Act::Save,
                        confirm: false,
                    });
                }
            }
            Message::Move { key, step } => {
                // One write at a time, for the reason the editor's own actions are:
                // two reorders would be two read-modify-writes of the same index.
                if !self.wearing {
                    if let Some(keys) = moved(&self.saved, &key, step) {
                        self.wearing = true;
                        self.notice = None;
                        let round = self.round + 1;
                        self.round = round;
                        return Some(Ask::Reorder(Reorder { round, keys }));
                    }
                }
            }
            Message::Reordered { round, result } => {
                if round == self.round {
                    self.wearing = false;
                    match result {
                        // What changed is the store's order, so the answer is the
                        // read of it again, the way a change that worked reloads the
                        // document rather than thanking the reader.
                        Ok(()) => return Some(Ask::Skins(self.ask())),
                        Err(reason) => self.notice = Some(reorder_line(&reason)),
                    }
                }
            }
            Message::ArmStyle { variant } => {
                if let Some(edit) = self.edit.as_mut() {
                    edit.variant = variant.to_string();
                }
            }
            Message::Cape { id } => {
                if let Some(edit) = self.edit.as_mut() {
                    edit.cape = id;
                }
            }
            Message::OpenEars => {
                // The notice's link, which is the reference's own `to` attribute: a
                // navigation rather than a request, so it travels as `Open` and the
                // shell turns it into an address.
                return Some(Ask::Open(Open::Project(EARS_PROJECT.to_string())));
            }
            Message::CloseEdit => self.edit = None,
            Message::Forget => {
                // The question, which asks nobody: the write is still one press
                // away and one press back from here.
                if let Some(edit) = self.edit.as_mut() {
                    self.notice = None;
                    edit.confirm = true;
                }
            }
            Message::CancelForget => {
                if let Some(edit) = self.edit.as_mut() {
                    edit.confirm = false;
                }
            }
            Message::ReplaceTexture => {
                self.notice = Some(crate::store::not_implemented("Replacing a skin's texture"));
            }
            Message::SignIn => {
                // The sign-in the banner's button asks for. The sentence is the
                // shell's own for this flow (`Shell`'s `Message::SignIn`), said here
                // because this page has no way to raise it.
                self.notice = Some(crate::store::not_implemented("Signing in to Minecraft"));
            }
            Message::Act(act) => {
                // One at a time, like every other write this page can ask for: a
                // second press while the first is out would be a second request
                // against the same account.
                if !self.wearing {
                    if let Some(edit) = self.edit.as_mut() {
                        edit.act = act;
                        let edit = edit.clone();
                        self.wearing = true;
                        self.notice = None;
                        return Some(Ask::EditSkin(edit));
                    }
                }
            }
            Message::Edited { round, result } => {
                if round == self.round {
                    self.wearing = false;
                    // The modal closes either way: on success what it changed is
                    // drawn by the reload, and on failure the sentence belongs in
                    // the page's own slot rather than under a modal that would be
                    // covering the row it is about.
                    self.edit = None;
                    match result {
                        Ok(()) => return Some(Ask::Skins(self.ask())),
                        Err(reason) => self.notice = Some(reason),
                    }
                }
            }
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
        }
        None
    }

    /// The request the page owes because nothing has been asked for yet.
    ///
    /// The reference reads the account's skins as the page mounts; this is the
    /// same rule stated where the shell can see it, so a window opened straight on
    /// `/skins` draws the account's own skins rather than an empty gallery.
    pub fn opening(&mut self) -> Option<Asked> {
        if self.appearance == Load::Idle {
            Some(self.ask())
        } else {
            None
        }
    }

    /// Bump the round, mark the page as waiting, and describe the request.
    fn ask(&mut self) -> Asked {
        self.round += 1;
        self.appearance = Load::Loading;
        Asked { round: self.round }
    }
}

/// Draw the page.
///
/// The reference's frame is one grid and one band: `skin-layout` is
/// `minmax(0, 1fr) minmax(0, 2.5fr)` with `gap` (2.5rem), holding the title and
/// the model preview on the left and the sections on the right, and the demo
/// banner sits below that grid -- a sibling of it, stuck to the bottom of the
/// window -- for a reader nobody is signed in as. So the page here is a column
/// of the scrolling layout and, in that state, the banner under it.
///
/// The reference's `sticky top-6` on the left column is a departure this file
/// carries rather than a number it picks: iced has no sticky, so the whole page
/// scrolls and the title leaves the window where the reference's would stay.
pub fn view<'a>(theme: Gen, state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = &state.notice {
        blocks.push(ui::admonition(
            theme,
            ui::Severity::Info,
            Key::AppSkinsTitle.message(),
            notice,
        ));
    }
    blocks.push(
        row![
            preview_column(theme, state),
            Space::with_width(COLUMN_GAP),
            section_list(theme, state, store),
        ]
        .align_items(Alignment::Start)
        .into(),
    );
    let layout = page::body_padded(blocks, GAP, PAGE_INSET, Message::Wheel);
    // The reference's demo banner is a *sibling* of the layout root, not a
    // third child of its grid: `sticky bottom-0 w-full` inside a
    // `grid-template-columns` root would be one column wide. Here that is the
    // shape too -- the layout scrolls above it and the banner is always at the
    // bottom of the pane -- so this is one column rather than the one region a
    // page used to return.
    match demo_banner(theme, state) {
        Some(banner) => column![layout, banner].into(),
        None => layout,
    }
}

/// The banner under the page when nobody is signed in, if that is the state.
///
/// The reference's own `v-if="!currentUser"`, and this launcher cannot read an
/// account of its own -- a page has never seen one -- so what it goes by is the
/// answer the shell gives a read with no Microsoft session, which is a sentence
/// rather than a request (`Shell::skins`). That is the same state: the shell
/// answers exactly the way the reference's own gate does, where a reader who is
/// not signed into Minecraft is told to sign in rather than shown an empty
/// gallery.
///
/// Idle and Loading draw nothing, which is the one difference: the reference
/// knows there is no user before it has asked, and this page only finds out
/// when the read answers.
fn demo_banner<'a>(theme: Gen, state: &'a State) -> Option<Element<'a, Message>> {
    if state.appearance.failure().is_none() {
        return None;
    }
    // The words: `text-contrast` on the title and `text-primary` on the
    // description, which is what `--color-text-default` resolves to -- the same
    // pair the reference's headings use and the same one [`INK_CONTRAST`] and
    // `INK_DEFAULT` are.
    let words: Element<'a, Message> = column![
        text(Key::AppSkinsDemoTitle.message())
            .size(BANNER_TITLE)
            .line_height(iced::Pixels(BANNER_LINE))
            .font(crate::style::semibold())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        text(Key::AppSkinsDemoDescription.message())
            .size(BANNER_DESCRIPTION)
            .line_height(iced::Pixels(BANNER_LINE))
            .font(crate::style::regular())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
    ]
    .spacing(BANNER_LINE_GAP)
    .into();
    let words = row![
        icon::icon(Glyph::Info, BANNER_ICON, theme_gen::ink(theme, Ink::Blue)),
        words,
    ]
    .spacing(BANNER_GAP)
    // `items-start`: the icon sits on the title's line rather than centred on
    // the pair of them, which at 24 against 52 is a nine-pixel difference at the
    // top of the box.
    .align_items(Alignment::Start)
    .width(Length::Fill);
    // The button is the reference's own `<Button type="colored" color="brand">`
    // with `messages.signInButton` in it, at no `size` attribute -- so
    // `Button.vue`'s own default, `md`: `h-9`, `rounded-xl`, `px-2.5`, `gap-1.5`,
    // a 20-pixel slot icon and a 16-pixel label. A capture of the reference
    // measures that frame at 36 tall and 202 wide, y 643..679 of a 720-pixel
    // window, its fill the brand green (27,217,106).
    let button: Element<'a, Message> = ui::button_with_icon_sized(
        theme,
        SIGN_IN_KEY,
        Glyph::LogIn,
        Key::AppSkinsSignInButton,
        ui::Kind::Colored,
        ui::Size::Md,
        Length::Shrink,
        Some(Message::SignIn),
    );
    // `max-w-5xl` is the one number this cannot honour: a page is never told how
    // wide its pane is (`crate::scroll::Geometry` carries an offset and a height
    // and nothing else), so the box is the pane's own width less its padding --
    // 905 - 32 = 873 here, against the reference's 884 -- and a window wide
    // enough to reach the cap is the one size where this would be wrong.
    let inner: Element<'a, Message> = container(
        row![words, button]
            .align_items(Alignment::Center)
            // `justify-between` with `gap-3`: the words take the room that is
            // left, and the button sits at the far end of the box.
            .spacing(BANNER_GAP)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding(BANNER_INSET)
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(iced::Background::Color(theme_gen::ink(theme, Ink::Surface3))),
        border: iced::Border {
            color: theme_gen::ink(theme, Ink::Surface5),
            width: 1.0,
            radius: BANNER_RADIUS.into(),
        },
        ..container::Appearance::default()
    })
    .into();
    Some(
        container(inner)
            .padding(Padding {
                top: 0.0,
                right: BANNER_PAD,
                bottom: BANNER_PAD,
                left: BANNER_PAD,
            })
            .into(),
    )
}

/// The gap between the model and the block the renderer draws under it.
///
/// `gap-6` on `.skin-preview-subtitle`, the block's own spacing. The distance
/// between the *model* and that block is the reference renderer's own -- 79
/// between the model's ink at y485 and the nametag at y564 on a capture -- and
/// it is the one number here that is the renderer's rather than a class, since
/// the model is a WebGL canvas and the block is DOM over it.
const PREVIEW_SUBTITLE_GAP: f32 = 24.0;

/// The left column: the page's own title over the model preview.
///
/// `sticky top-6 self-start p-2` and the preview's `ml-5 mt-4
/// h-[calc(80vh-1rem)]`: the title at the column's own edge and the box 16
/// below it, 560 tall, with the model and the block under it centred together
/// -- which is [`preview_box`]'s shape in every state, not only the one with a
/// model in it.
///
/// [`page::draw`] would do the four states, but it hands the ready arm a
/// closure and nothing else, and the other three have to go *in the box* rather
/// than under the title: a sentence on its own line at y147 is not the shape
/// the reference's box is at all. So the match is here, and it calls the same
/// three blocks [`page::draw`] calls.
fn preview_column<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    // The three states with no account to draw are the box and one sentence in
    // it, which is `preview_box`'s own shape with no block under the model.
    // The three states with no account to draw are the box and one sentence in
    // it, which is `preview_box`'s own shape with no block under the model.
    let title: Element<'a, Message> = text(Key::AppSkinsTitle.message())
        .size(TITLE_SIZE)
        // `font-bold` (700) rather than [`page::title`]'s `font-extrabold`
        // (800): both headings are the reference's, and the reference's own
        // classes differ by exactly this.
        .font(crate::style::inter(iced::font::Weight::Bold))
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)))
        .into();
    let body: Element<'a, Message> = match &state.appearance {
        Load::Ready(appearance) => preview_body(theme, appearance, state.wearing),
        Load::Empty => preview_box(page::empty(theme, Key::BrowseNoResults), None),
        Load::Failed(reason) => preview_box(page::failed(theme, reason), None),
        Load::Idle | Load::Loading => preview_box(page::waiting(theme, "your skins"), None),
    };
    container(column![title, Space::with_height(PREVIEW_TOP), body].width(Length::Fill))
        .width(Length::FillPortion(PREVIEW_PORTION))
        .padding(COLUMN_PAD)
        .into()
}

/// The reference's preview box: `ml-5 mt-4 flex h-[calc(80vh-1rem)]
/// items-center justify-center` -- 20 from the column's left edge (the 16 below
/// the title is [`preview_column`]'s), [`PREVIEW_HEIGHT`] tall at this window,
/// and whatever is inside it centred on both axes.
///
/// One box for every state is the point: the reference's renderer always draws
/// a model, so it never has a state to place here, and a state that drew
/// nothing but a line of text would be a different shape from the one the
/// reference's own column has.
/// The reference's render box: the model, and under it the block the renderer
/// draws beneath itself.
///
/// `SkinPreviewRenderer` fills the box and lays its `#subtitle` slot and its
/// nametag *inside* it -- on a signed-out capture of the reference the box is
/// y119..679, the model's ink y255..485, the nametag y564..580 and the Edit
/// button y608..644, all inside those 560 pixels. This drew the model in the box
/// and everything else under it, which put the account's own rows below the
/// bottom of the pane: the box alone is 560 tall and the pane is 671, so nothing
/// after it was ever on screen.
///
/// The gap between the two is this launcher's own. The reference's is the space
/// its WebGL model leaves, measured at 79 between the model's ink and the
/// nametag; the block's own spacing is `gap-6` on `.skin-preview-subtitle`, and
/// 24 is what the two are held apart by here.
fn preview_box<'a>(
    model: Element<'a, Message>,
    subtitle: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mut inner = column![model].spacing(PREVIEW_SUBTITLE_GAP).width(Length::Fill);
    if let Some(subtitle) = subtitle {
        inner = inner.push(subtitle);
    }
    container(
        row![
            Space::with_width(PREVIEW_LEFT),
            container(inner)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y(),
        ]
        .width(Length::Fill),
    )
    .height(Length::Fixed(PREVIEW_HEIGHT))
    .into()
}

/// The preview box and the account's own half under it.
///
/// The reference's box holds its `SkinPreviewRenderer` and nothing else *besides*
/// what that renderer draws inside itself; this launcher's own two lists are
/// drawn in the same block, under the model, because the reference has no
/// page-level list for them at all -- its sections are store skins, and the cape
/// choice lives in a modal that opens from a selected skin. Keeping them here is
/// what keeps "wear one of the account's capes" reachable with nothing stored;
/// the words are the reference's own section labels.
fn preview_body<'a>(theme: Gen, appearance: &'a Appearance, wearing: bool) -> Element<'a, Message> {
    let model: Element<'a, Message> = match &appearance.front {
        Some(front) => image(front.handle())
            .width(Length::Fixed(DOLL_WIDTH))
            .height(Length::Fixed(DOLL_HEIGHT))
            .into(),
        None => text(
            appearance
                .note
                .clone()
                .unwrap_or_else(|| crate::store::not_implemented("This skin")),
        )
        .size(13.0)
        .font(crate::style::medium())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY)))
        .into(),
    };
    // The block under the model: the nametag, the sentence about what Minecraft
    // says is in force, the notice, and the account's own rows -- the reference's
    // nametag and subtitle, which its renderer draws inside the box.
    let mut body = column![
        text(appearance.username.clone())
            .size(16.0)
            .font(crate::style::semibold())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        caption(theme, &wearing_line(appearance)),
    ]
    .spacing(ROW_GAP)
    .width(Length::Fill);
    // The Ears notice, which the reference draws in the preview's own `#subtitle`
    // slot -- under the model, not in the editor -- and only for a skin that asks
    // for it. The one this can see is the skin in force, because that is the one
    // the pane above is drawing (departure 18).
    if appearance.ears.is_some() {
        body = body.push(ears_notice(theme));
    }
    // The account's own skins: drawn only where one is *not* already on, which
    // is the rule [`wear_skin`] is; an account normally owns exactly the one it
    // wears (`MinecraftSkins::equipped`), and a row with no action would be a
    // line saying what the sentence above already says.
    for (index, skin) in appearance.skins.iter().enumerate() {
        let Some(action) = wear_skin(skin, wearing) else {
            continue;
        };
        let identity = if skin.id.is_empty() { index.to_string() } else { skin.id.clone() };
        body = body.push(owned_row(
            theme,
            &variant_label(&skin.variant),
            false,
            Key::AppSkinsApplyButton,
            ui::scoped(WEAR_KEY, &identity),
            Some(action),
        ));
    }
    body = body.push(heading(theme, Key::AppSkinsModalCapeSection.message()));
    if appearance.capes.is_empty() {
        body = body.push(caption(theme, Key::AppSkinsModalNoneCapeOption.message()));
    }
    for cape in &appearance.capes {
        let name = if cape.alias.is_empty() { cape.id.as_str() } else { cape.alias.as_str() };
        // One answer for which cape is in force rather than each row's own state:
        // asked once, the rows and the doll above cannot disagree about it.
        let worn = appearance.equipped_cape().is_some_and(|worn| worn.id == cape.id);
        let identity = if cape.id.is_empty() { name.to_string() } else { cape.id.clone() };
        body = body.push(owned_row(
            theme,
            name,
            worn,
            Key::AppSkinsApplyButton,
            ui::scoped(WEAR_KEY, &identity),
            wear_cape(cape, worn, wearing),
        ));
    }
    // The reference's own "no cape" choice, drawn only when something has to be
    // taken off: a row that hides a cape the account is not wearing would be a
    // button whose only effect is a request to change nothing.
    if let Some(cape) = appearance.equipped_cape() {
        body = body.push(owned_row(
            theme,
            Key::AppSkinsModalNoneCapeOption.message(),
            false,
            Key::AppSkinsApplyButton,
            ui::scoped(WEAR_KEY, &format!("none:{}", cape.id)),
            (!wearing).then_some(Message::Wear(SkinChange::NoCape)),
        ));
    }
    // The model and the block are centred together, which is what the reference's
    // renderer does with the two: on a signed-out capture its box is y119..679,
    // the model y255..485 and the nametag y564..580, so the group sits above the
    // middle rather than the model alone doing so.
    preview_box(model, Some(body.into()))
}

/// The right column: the reference's sections, in its own order and at its own
/// spacing.
///
/// `pt-2` above the first row, the open section's cards under its header, and
/// 24 between one section and the next. Which sections are open is
/// [`Expanded`]'s: every one of them until a header is pressed, which is the state
/// the reference's own first pass leaves it in.
fn section_list<'a>(theme: Gen, state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let mut list = column![].width(Length::FillPortion(LIST_PORTION));
    for (index, section) in Section::ALL.iter().enumerate() {
        let first = index == 0;
        list = list.push(Space::with_height(if first {
            LIST_TOP + SECTION_FIRST_TOP
        } else {
            SECTION_TOP
        }));
        let open = state.open.is_open(*section);
        // The header is the reference's `Accordion` button: a `size-6` chevron
        // turned when the section is open, then its `text-xl font-semibold`
        // title in the default ink -- `text-primary`, which is what the
        // reference's Tailwind calls `--color-text-default`.
        let head = row![
            icon::icon(
                if open { Glyph::ChevronUp } else { Glyph::ChevronDown },
                SECTION_ICON,
                theme_gen::ink(theme, INK_DEFAULT),
            ),
            Space::with_width(SECTION_ICON_GAP),
            text(section.label())
                .size(SECTION_TITLE)
                .font(crate::style::semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        ]
        .height(Length::Fixed(SECTION_HEIGHT))
        .align_items(Alignment::Center)
        .width(Length::Fill);
        list = list.push(
            mouse_area(head)
                .interaction(iced::mouse::Interaction::Pointer)
                .on_press(Message::Select(*section)),
        );
        if open {
            list = list.push(Space::with_height(SECTION_CONTENT_TOP));
            list = list.push(section_content(theme, state, store, *section));
        }
    }
    list.into()
}

/// One open section's content.
///
/// The saved section is the grid; every other one is the sentence that says
/// where its skins come from. The reference draws those from Modrinth's own
/// skin store, an account service this launcher does not hold (G118), and the
/// sentence is inside the section that would draw them rather than over the
/// whole page.
fn section_content<'a>(
    theme: Gen,
    state: &'a State,
    store: &'a Store,
    section: Section,
) -> Element<'a, Message> {
    match section {
        Section::SavedSkins => saved_grid(theme, state, store),
        _ => caption(theme, &crate::store::needs_account("The skin store")),
    }
}

/// The saved skins' grid: `grid-cols-3 gap-3` with the add cell first.
///
/// The reference's own column count is 4, 5 or 6 on windows 1300, 1750 and
/// 2050 wide (`VirtualSkinSectionList.vue`); this shell's pane is 916 and iced
/// has no window width to branch on, so the three-column layout a 980-pixel
/// viewport gets is the one this draws at every size.
fn saved_grid<'a>(theme: Gen, state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let live = !state.wearing;
    let last = state.saved.len().saturating_sub(1);
    let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(state.saved.len() + 1);
    cells.push(add_card(theme, live));
    for (index, row) in state.saved.iter().enumerate() {
        // The two controls this launcher has instead of the reference's drag:
        // drawn only where a move would do something -- the first row has no up,
        // the last no down -- and only while no write is out, because the order
        // they would ask the store for is the order it is about to be read back
        // in anyway.
        let up = (index > 0 && live).then_some(Message::Move {
            key: row.entry.key.clone(),
            step: Step::Up,
        });
        let down = (index < last && live).then_some(Message::Move {
            key: row.entry.key.clone(),
            step: Step::Down,
        });
        cells.push(saved_card(theme, row, live, up, down, store));
    }
    let mut grid = column![].spacing(CARD_GAP).width(Length::Fill);
    let mut line = row![].spacing(CARD_GAP).width(Length::Fill);
    let mut filled = 0;
    for cell in cells {
        line = line.push(cell);
        filled += 1;
        if filled == CARD_COLUMNS {
            grid = grid.push(line);
            line = row![].spacing(CARD_GAP).width(Length::Fill);
            filled = 0;
        }
    }
    if filled > 0 {
        // The cells that are not there keep their share: a row of three columns
        // with two skins in it draws two cards of the same width, not stretched
        // ones, which is what the reference's own grid does.
        for _ in filled..CARD_COLUMNS {
            line = line.push(Space::with_width(Length::FillPortion(1)));
        }
        grid = grid.push(line);
    }
    grid.into()
}

/// The saved section's first cell: the reference's `SkinLikeTextButton` as a
/// dropzone.
///
/// `aspect-[31/40]`, `rounded-[20px]`, a dashed `border-surface-5` over
/// `bg-surface-2` (hovering raises it to `bg-surface-3`), a `size-8` plus, and
/// `gap-4` down to the two lines: `text-base font-semibold leading-6` over
/// `text-sm font-medium leading-5 text-primary`. The drag-and-drop it also
/// offers is not drawn -- this launcher's own file dialog is the only way a
/// file arrives (G123) -- and the dash is drawn as strips rather than as a
/// border style, for [`DASH_ON`]'s reason.
fn add_card<'a>(theme: Gen, live: bool) -> Element<'a, Message> {
    let (_, fraction) = ui::interaction(ADD_KEY);
    let background = crate::theme::mix(
        theme_gen::ink(theme, Ink::Surface2),
        theme_gen::ink(theme, Ink::Surface3),
        fraction,
    );
    let words = column![
        icon::icon(Glyph::Plus, ADD_ICON, theme_gen::ink(theme, INK_CONTRAST)),
        column![
            text(Key::AppSkinsAddButton.message())
                .size(16.0)
                .line_height(iced::Pixels(ADD_LABEL_LINE))
                .font(crate::style::semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            text(Key::AppSkinsAddButtonDragAndDrop.message())
                .size(14.0)
                .line_height(iced::Pixels(ADD_SUBTITLE_LINE))
                .font(crate::style::medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        ]
        .spacing(ADD_LINE_GAP)
        .align_items(Alignment::Center),
    ]
    .spacing(ADD_ICON_GAP)
    .align_items(Alignment::Center);
    // The words sit inside the dashed ring rather than in the cell: the ring is
    // a pixel of the cell's own height on each side, so the cell is still
    // `CARD_HEIGHT` tall and the words are still `px-3` from its edge.
    let body = container(words)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x()
        .center_y()
        .padding(Padding {
            top: 0.0,
            bottom: 0.0,
            left: ADD_CARD_PAD,
            right: ADD_CARD_PAD,
        });
    let sides = row![dash_column(theme), body, dash_column(theme)]
        .height(Length::Fixed(CARD_HEIGHT - 2.0));
    let cell = container(
        column![dash_row(theme), sides, dash_row(theme)]
            .width(Length::Fill)
            .height(Length::Fill),
    )
        .width(Length::FillPortion(1))
        .height(Length::Fixed(CARD_HEIGHT))
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(iced::Background::Color(background)),
            // No border of its own: the ring above is the border, and it is the
            // dashed one.
            border: iced::Border {
                color: iced::Color::TRANSPARENT,
                width: 0.0,
                radius: CARD_RADIUS.into(),
            },
            ..container::Appearance::default()
        });
    let area = mouse_area(cell)
        .interaction(iced::mouse::Interaction::Pointer)
        .on_enter(Message::Hover { key: ADD_KEY, over: true, hover: None })
        .on_exit(Message::Hover { key: ADD_KEY, over: false, hover: None });
    // Unusable while a change is in flight, like every other write on this
    // page: the dialog is modal, so a second press would be a second dialog.
    if live {
        area.on_press(Message::AddSkin).into()
    } else {
        area.into()
    }
}

/// One stored skin, as the reference's `SkinButton`.
///
/// `aspect-[31/40]`, `rounded-[20px]`, `border-surface-4` over `bg-surface-3`,
/// with the hover swapping them for `surface-5` and `surface-4` -- drawn as a
/// mix by the hover's own fraction, so the card arrives at exactly the two
/// colours the reference names. What the reference puts in the cell is the
/// forward render its preview service makes; this launcher's own store holds
/// the texture and nothing renders it, so the picture is the stored PNG drawn
/// to fit. The name is the reference's tooltip, which iced has no widget for,
/// so it is a line under the picture, and the two chevrons are the reorder this
/// launcher draws instead of the reference's drag.
fn saved_card<'a>(
    theme: Gen,
    row: &'a SavedRow,
    live: bool,
    up: Option<Message>,
    down: Option<Message>,
    store: &'a Store,
) -> Element<'a, Message> {
    let key = ui::scoped(EDIT_KEY, &row.entry.key);
    let (_, fraction) = ui::interaction(key);
    let background = crate::theme::mix(
        theme_gen::ink(theme, Ink::Surface3),
        theme_gen::ink(theme, Ink::Surface4),
        fraction,
    );
    let border = crate::theme::mix(
        theme_gen::ink(theme, Ink::Surface4),
        theme_gen::ink(theme, Ink::Surface5),
        fraction,
    );
    // The picture is the store's own file, handed to iced as a path so the
    // renderer loads it once and keeps it; a store with no home (a test) draws
    // no picture rather than a broken one.
    let picture: Element<'a, Message> = match store.paths() {
        Some(home) => image(image::Handle::from_path(saved_skins::texture_path(home, &row.entry)))
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Contain)
            .into(),
        None => Space::with_height(Length::Fill).into(),
    };
    // A press on the picture opens the editor, the way the reference's hover
    // buttons do; a press on the footer does not, or a chevron beside it would
    // open the editor as well as move the row.
    let picture: Element<'a, Message> = if live {
        mouse_area(picture)
            .interaction(iced::mouse::Interaction::Pointer)
            .on_press(Message::Edit { key: row.entry.key.clone() })
            .into()
    } else {
        picture
    };
    let mut footer = row![
        text(row.entry.name.clone())
            .size(13.0)
            .font(crate::style::medium())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        Space::with_width(Length::Fill),
    ]
    .spacing(4.0)
    .align_items(Alignment::Center)
    .width(Length::Fill);
    // The reference draws these two controls only under the pointer and only on
    // a skin that is not in force; this launcher draws them whenever they would
    // move something, which is where they can be found rather than guessed at.
    if let Some(message) = up {
        footer = footer.push(ui::icon_button(
            theme,
            ui::scoped(MOVE_UP_KEY, &row.entry.key),
            Glyph::ChevronUp,
            MOVE_MARK,
            message,
        ));
    }
    if let Some(message) = down {
        footer = footer.push(ui::icon_button(
            theme,
            ui::scoped(MOVE_DOWN_KEY, &row.entry.key),
            Glyph::ChevronDown,
            MOVE_MARK,
            message,
        ));
    }
    let cell = container(column![picture, footer].spacing(6.0).width(Length::Fill))
        .width(Length::FillPortion(1))
        .height(Length::Fixed(CARD_HEIGHT))
        .padding(Padding {
            top: CARD_INSET,
            bottom: CARD_INSET,
            left: CARD_INSET,
            right: CARD_INSET,
        })
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(iced::Background::Color(background)),
            border: iced::Border { color: border, width: 1.0, radius: CARD_RADIUS.into() },
            ..container::Appearance::default()
        });
    mouse_area(cell)
        .on_enter(Message::Hover { key, over: true, hover: None })
        .on_exit(Message::Hover { key, over: false, hover: None })
        .into()
}

/// The question `remove_custom_skin` is only asked after.
///
/// `Skins.vue`'s `ConfirmModal`: `app.skins.delete-modal.title` as the modal's
/// own title, `app.skins.delete-modal.description` under it, and `Delete` beside
/// Cancel in the actions row. A dialog inside a dialog is the shell's to draw
/// (`Shell::modal_layer` answers one modal at a time, and the editor is the one
/// it is answering), so the question is this body's heading instead of the
/// frame's title, with the reference's two sentences and the reference's two
/// buttons in the reference's own order: proceed last, at the right-hand end
/// where `justify-end` puts it.
fn forget_confirm<'a>(
    theme: Gen,
    edit: &Edit,
    wearing: bool,
) -> Element<'a, Message> {
    column![
        heading(theme, Key::AppSkinsDeleteModalTitle.message()),
        caption(theme, Key::AppSkinsDeleteModalDescription.message()),
    ]
    .spacing(EDITOR_HEADING_GAP)
    .push(
        row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Center)
            .push(Space::with_width(Length::Fill))
            .push(ui::button_text(
                theme,
                ui::scoped(EDIT_KEY, "forget-cancel"),
                CANCEL_LABEL,
                ui::Kind::Outlined,
                Message::CancelForget,
            ))
            .push(ui::button_or(
                theme,
                ui::scoped(EDIT_KEY, "forget"),
                Key::AppSkinsDeleteButton,
                ui::Kind::Danger,
                (!wearing).then_some(Message::Act(Act::Forget)),
            )),
    )
    .width(Length::Fill)
    .into()
}

/// The question's own "Cancel".
///
/// `commonMessages.cancelButton` in the reference, which the vendored message
/// table does not carry -- the generator took the app's own ids and this is the
/// shared package's -- so the word is written here, as
/// [`TAKE_OFF_LABEL`] is for the one control the reference has no words for at
/// all.
const CANCEL_LABEL: &str = "Cancel";

/// Whether the editor holds an edit worth saving.
///
/// `hasEdits` in `EditSkinModal.vue`, with the one condition it cannot have
/// here left out: in `edit` mode it is true when a texture has been uploaded,
/// when `variant` differs from `currentSkin.variant`, or when the chosen cape's
/// id differs from the row's. The upload belongs to a texture section this
/// launcher cannot draw (departure 15), so what is left is the two the modal's
/// own controls can change -- and a reader who has touched neither is not asked
/// to press Save.
fn has_edits(edit: &Edit) -> bool {
    edit.variant != edit.stored.variant || edit.cape != edit.stored.cape
}

/// The editor's body: the arm style, the cape, the Ears notice, and three actions.
///
/// This is the reference's `EditSkinModal.vue`, drawn as the *body* of this launcher's
/// modal rather than as a modal of its own: the shell puts the dialog's frame, its title
/// and its close button around what this returns, because an editor belongs to the row
/// it edits and the modal layer is the shell's.
///
/// The reference's modal has Save and Cancel. Deleting a saved skin is a button in its
/// preview panel leading to a confirm dialog of its own (`Skins.vue`'s `deleteSkin`),
/// and taking the account's skin off is `helpers/skins.ts`'s `unequip_skin`, which
/// nothing in the vendored frontend calls at all. This page has no preview panel -- the
/// doll above draws what Minecraft says is in force, not a candidate -- so the editor
/// carries the deletion, and it carries the take-off for the same reason: it is the one
/// place a reader would look for it.
///
/// `wearing` is handed in rather than read here because it belongs to the page: while a
/// write is out, the two actions that *are* writes are drawn unusable, which is the rule
/// every other row on this page follows.
pub fn edit_view<'a>(
    theme: Gen,
    edit: &'a Edit,
    capes: &'a [palantir_net::MinecraftCape],
    wearing: bool,
) -> Element<'a, Message> {
    let slim = edit.variant.eq_ignore_ascii_case("SLIM");
    let mut body = column![].spacing(EDITOR_SECTION_GAP).width(Length::Fill);
    if edit.confirm {
        return forget_confirm(theme, edit, wearing);
    }
    // The name the row is stored under. The reference's own title is the sentence
    // "Editing skin"; *which* skin is the row's, and a reader who opened the wrong one
    // needs to see that before they press Save.
    body = body.push(caption(theme, &edit.name));
    // The texture section, which the reference draws first and only in `edit` mode
    // for a skin whose `source` is not `default`. A stored row's source is
    // `custom` or `custom_external` -- the store holds nothing else -- so the
    // section is here for every row this editor opens.
    body = body.push(section(
        theme,
        Key::AppSkinsModalTextureSection.message(),
        ui::button_with_icon(
            theme,
            ui::scoped(EDIT_KEY, "replace-texture"),
            Glyph::Upload,
            Key::AppSkinsModalReplaceTextureButton,
            ui::Kind::Standard,
            Length::Shrink,
            Some(Message::ReplaceTexture),
        ),
    ));
    // Arm style: the reference's own `RadioButtons` over the service's two words.
    body = body.push(section(
        theme,
        Key::AppSkinsModalArmStyleSection.message(),
        ui::chips(
            theme,
            &[ui::scoped(EDIT_KEY, "arm:wide"), ui::scoped(EDIT_KEY, "arm:slim")],
            &[(variant_label("CLASSIC"), !slim), (variant_label("SLIM"), slim)],
            move |index| {
                Some(Message::ArmStyle {
                    variant: if index == 0 { "CLASSIC" } else { "SLIM" },
                })
            },
        ),
    ));
    // The cape: the reference's own "None" cell *first*, then one cell per cape the
    // account owns, four to a row. A choice's mark is the row's *stored* cape id, so a
    // chip draws the check the other choices do only when it is the one this row asks
    // for -- the none cell included, which is how a row with no cape says so.
    let mut cape_list = column![].spacing(CAPE_GAP);
    for row in cape_rows(capes) {
        let mut keys: Vec<&'static str> = Vec::with_capacity(row.len());
        let mut labels: Vec<(String, bool)> = Vec::with_capacity(row.len());
        // The ids travel with the row so a press names a cape rather than a place in
        // the list: the fourth cell of the second row is not index 4.
        let mut ids: Vec<String> = Vec::with_capacity(row.len());
        for choice in row {
            match choice {
                None => {
                    keys.push(ui::scoped(EDIT_KEY, "cape:none"));
                    labels.push((
                        Key::AppSkinsModalNoneCapeOption.message().to_string(),
                        edit.cape.is_empty(),
                    ));
                    ids.push(String::new());
                }
                Some(cape) => {
                    keys.push(ui::scoped(EDIT_KEY, &format!("cape:{}", cape.id)));
                    labels.push((cape_name(cape), edit.cape == cape.id));
                    ids.push(cape.id.clone());
                }
            }
        }
        cape_list = cape_list.push(ui::chips(theme, &keys, &labels, move |index| {
            Some(Message::Cape { id: ids.get(index).cloned().unwrap_or_default() })
        }));
    }
    body = body.push(section(theme, Key::AppSkinsModalCapeSection.message(), cape_list.into()));
    // The actions. The reference's own row is `flex gap-2 justify-end` -- Cancel
    // then Save, both at the right-hand end, 8 apart -- so the fill goes first and
    // Save, which is the last control in the reference's row too, is the last
    // control here. Cancel is the dialog's own close button, which the shell
    // draws in the head above this body. Save is the reference's own button with
    // its `SaveIcon` in front of the label, and Forget is its danger preset -- the
    // one colour the reference gives the single control that takes something away.
    // Take-off has no key to be drawn unusable with, because the reference has no
    // word for it at all; the page refuses a second press in `update`, which is the
    // rule where it cannot be sidestepped.
    body = body.push(
        row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Center)
            .push(Space::with_width(Length::Fill))
            .push(ui::button_text(
                theme,
                ui::scoped(EDIT_KEY, "takeoff"),
                TAKE_OFF_LABEL,
                ui::Kind::Outlined,
                Message::Act(Act::TakeOff),
            ))
            .push(ui::button_or(
                theme,
                ui::scoped(EDIT_KEY, "forget"),
                Key::AppSkinsDeleteButton,
                ui::Kind::Danger,
                // A question, not the write: the reference's delete goes through its
                // `ConfirmModal`, and this one asks in the same body.
                (!wearing).then_some(Message::Forget),
            ))
            .push(ui::button_with_icon(
                theme,
                ui::scoped(EDIT_KEY, "save"),
                Glyph::Save,
                Key::AppSkinsModalSaveSkinButton,
                ui::Kind::Colored,
                Length::Shrink,
                // Unusable while nothing has been changed, which is the
                // reference's `disableSave`: a save that writes the row back
                // exactly as it was is a write a reader did not ask for.
                (!wearing && has_edits(edit)).then_some(Message::Act(Act::Save)),
            )),
    );
    body.into()
}

/// The gap between two of the reference's own sections in the editor.
///
/// `EditSkinModal.vue`'s right column is `flex flex-col gap-4`, so one section
/// and the next are 16 apart -- which is what this drew as [`crate::page::GAP`]
/// before, and 4 short of it.
const EDITOR_SECTION_GAP: f32 = 16.0;

/// The gap between a section's own heading and the control under it.
///
/// Every `<h2 class="text-base font-semibold mb-2">` in that column, so the
/// heading sits 8 above its own control and 8 closer to it than to the section
/// above -- the reference's two numbers, and the reason the blocks read as
/// pairs rather than as a list.
const EDITOR_HEADING_GAP: f32 = 8.0;

/// One of the reference's own sections: its heading and the control beneath it.
///
/// The `<section>` element itself, which is a `flex-col` child of the column
/// with nothing between it and its own contents except the heading's `mb-2`.
fn section<'a>(
    theme: Gen,
    label: &str,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    column![heading(theme, label), control].spacing(EDITOR_HEADING_GAP).into()
}

/// How many capes the reference's cape list draws to a row.
///
/// `EditSkinModal.vue`'s list is
/// `grid grid-cols-[repeat(4,max-content)] auto-rows-max gap-2 overflow-y-auto pr-1`:
/// four columns, each as wide as its widest cell, with `gap-2` -- 8px -- between
/// cells and 8px between rows.
const CAPE_COLUMNS: usize = 4;

/// The gap between two cells of that grid, in both directions.
///
/// The same `gap-2` as [`CAPE_COLUMNS`]'s own line, and it is what
/// [`crate::ui::chips`] already puts between the chips in one of its rows, so the
/// rows this page draws need no gap of their own between cells -- only between
/// the rows.
const CAPE_GAP: f32 = 8.0;

/// The cape list's own cells, in the reference's order and its own rows.
///
/// [`EditSkinModal.vue`] draws the no-cape cell ahead of its `v-for`, and the
/// `v-for` runs over `sortedCapes`, which is
/// `[...(props.capes || [])].sort((a, b) => (a.name || '').toLowerCase()
/// .localeCompare((b.name || '').toLowerCase()))` -- the account's own capes, by
/// their names, case-folded, with an unnamed cape first among them because its
/// name is the empty string. `None` is the no-cape cell; it is only ever first,
/// and a row is never padded out with copies of it.
fn cape_rows(
    capes: &[palantir_net::MinecraftCape],
) -> Vec<Vec<Option<&palantir_net::MinecraftCape>>> {
    let mut sorted: Vec<&palantir_net::MinecraftCape> = capes.iter().collect();
    // `sort_by` is stable, so two capes the service gave the same name keep the
    // order it listed them in -- which is what `Array.prototype.sort` does too.
    sorted.sort_by(|left, right| cape_sort_name(left).cmp(&cape_sort_name(right)));
    let mut cells: Vec<Option<&palantir_net::MinecraftCape>> = Vec::with_capacity(sorted.len() + 1);
    cells.push(None);
    cells.extend(sorted.into_iter().map(Some));
    cells.chunks(CAPE_COLUMNS).map(|row| row.to_vec()).collect()
}

/// What `sortedCapes` compares: the cape's own name, folded.
fn cape_sort_name(cape: &palantir_net::MinecraftCape) -> String {
    cape.alias.to_lowercase()
}

/// What a cape cell is labelled with.
///
/// `CapeButton`'s `:name="cape.name || formatMessage(messages.capeFallbackName)"`
/// -- the name the service gave it, and the reference's own word "Cape" when it
/// gave none. This launcher's field for that name is the alias.
fn cape_name(cape: &palantir_net::MinecraftCape) -> String {
    if cape.alias.is_empty() {
        Key::AppSkinsModalCapeFallbackName.message().to_string()
    } else {
        cape.alias.clone()
    }
}

/// The take-off action's own words.
///
/// Hand-written rather than taken from the generated table, because that table is the
/// reference's and the reference has no string for this: `unequip_skin` is in its client
/// and nothing in its frontend calls it. The page's other hand-written sentences -- the
/// Saved-skins empty state, the note under the doll -- are the same kind of thing.
const TAKE_OFF_LABEL: &str = "Take it off";

/// The reference's Ears notice, with its own link where its placeholder is.
///
/// `app.skins.ears-feature-notice` is `"This skin uses features from the {ears} mod"`,
/// and the reference fills the placeholder with a sentinel, splits on it and draws what
/// is between the halves as a `router-link` to `/project/mfzaZK3Z` labelled "Ears"
/// (`Skins.vue`, its `earsFeatureNoticeParts`). This is that split
/// ([`crate::text::placeholder`]) and that link. A message that arrived without the
/// placeholder is drawn whole rather than dropped: the reader still needs to know why
/// their skin looks different.
///
/// The reference wraps that copy in a 40-pixel icon, a `max-w-[340px]` column and an
/// outlined "Turn Ears features off" button beside a `Toggle` -- the runtime switch
/// for the mod, which this launcher has no way to send. The sentence and its link are
/// the part that carries the information, and they are drawn under the model.
fn ears_notice<'a>(theme: Gen) -> Element<'a, Message> {
    let message = Key::AppSkinsEarsFeatureNotice.message();
    let Some((before, after)) = crate::text::placeholder_parts(message, "ears") else {
        return caption(theme, message);
    };
    row![]
        .spacing(6.0)
        .align_items(Alignment::Center)
        .push(caption(theme, before))
        .push(
            iced::widget::mouse_area(
                text("Ears")
                    .size(13.0)
                    .font(crate::style::medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::AccentContrast))),
            )
            .interaction(iced::mouse::Interaction::Pointer)
            .on_press(Message::OpenEars),
        )
        .push(caption(theme, after))
        .into()
}

/// The change that puts one of the account's own skins on, or nothing when there
/// is nothing to do.
///
/// Its own function so the rule -- not the skin already in force, and not while a
/// change is in flight -- is a thing a test can read rather than something to be
/// inferred from a drawn row.
fn wear_skin(skin: &palantir_net::MinecraftSkin, wearing: bool) -> Option<Message> {
    if skin.equipped() || wearing {
        return None;
    }
    Some(Message::Wear(SkinChange::Skin {
        variant: skin.variant.clone(),
        url: skin.url.clone(),
    }))
}

/// The same rule for one of the account's capes.
fn wear_cape(cape: &palantir_net::MinecraftCape, worn: bool, wearing: bool) -> Option<Message> {
    if worn || wearing {
        return None;
    }
    Some(Message::Wear(SkinChange::Cape { id: cape.id.clone() }))
}

/// The order a move would write, or `None` when the press would change nothing.
///
/// [`wear_skin`]'s arrangement and its reason: the rule -- the row is found by its
/// key, and it has a neighbour to trade places with -- is a thing a test can read
/// rather than something to be inferred from a drawn list. The two ends answer
/// `None` rather than an order equal to the one on screen, because a press there
/// would be a write that changes nothing, and the row is drawn without the control
/// that would send it (see [`saved_grid`]).
fn moved(rows: &[SavedRow], key: &str, step: Step) -> Option<Vec<String>> {
    let index = rows.iter().position(|row| row.entry.key == key)?;
    let to = match step {
        Step::Up if index > 0 => index - 1,
        Step::Down if index + 1 < rows.len() => index + 1,
        _ => return None,
    };
    let mut keys: Vec<String> = rows.iter().map(|row| row.entry.key.clone()).collect();
    keys.swap(index, to);
    Some(keys)
}

/// The sentence under an order that could not be written.
///
/// The reference's answer to the same failure is a notification with a title
/// (`app.skins.reorder-error.title`, "Failed to reorder skins") over one of two
/// sentences -- "Your skin order could not be saved.", or the thrown error's own
/// message. This page's notice slot is one line, so the reference's title comes
/// first and the store's refusal follows it: the title is what happened and the
/// refusal is which file this launcher could not write, which is the half a reader
/// can act on and the half the reference keeps only when its error carries one.
fn reorder_line(reason: &str) -> String {
    format!("{}: {reason}", Key::AppSkinsReorderErrorTitle.message())
}

/// One thing the account owns: what it is called, and the mark when it is worn.
///
/// The mark is the reference's own `CheckIcon`, in the accent ink rather than a word
/// this launcher would have had to invent: the document says `ACTIVE`, and a reader
/// who sees the check on the skin the page is drawing above knows which is which.
fn owned_row<'a>(
    theme: Gen,
    name: &str,
    worn: bool,
    label: Key,
    key: &'static str,
    action: Option<Message>,
) -> Element<'a, Message> {
    let ink = if worn { INK_CONTRAST } else { INK_SECONDARY };
    let mut line = row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .push(
            text(name.to_string())
                .size(14.0)
                .font(crate::style::medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, ink))),
        )
        .push(Space::with_width(Length::Fill));
    if worn {
        line = line.push(icon::icon(
            Glyph::Check,
            WORN_MARK,
            theme_gen::ink(theme, Ink::AccentContrast),
        ));
    }
    // The action is the reference's own Apply, on the row it applies to. It is
    // drawn unusable rather than dropped while a change is in flight (`None`,
    // which is what `button_or` takes for that): a second press would be a second
    // write to the reader's own account.
    let line = line.push(ui::button_or(
        theme,
        key,
        label,
        ui::Kind::Standard,
        action,
    ));
    // Named rather than inferred: `ui::card` takes `impl Into<Element>`, and an
    // `into()` inside that is a conversion with two possible targets.
    let row: Element<'a, Message> = line.into();
    ui::card(theme, row)
}

/// A heading inside the account's block: the reference's section titles.
fn heading<'a>(theme: Gen, label: &str) -> Element<'a, Message> {
    text(label.to_string())
        .size(16.0)
        .font(crate::style::semibold())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)))
        .into()
}

/// A line of prose under something, in the panel's own secondary ink.
fn caption<'a>(theme: Gen, line: &str) -> Element<'a, Message> {
    text(line.to_string())
        .size(13.0)
        .font(crate::style::medium())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY)))
        .into()
}

/// The one sentence about the skin in force, from the document's own vocabulary.
fn wearing_line(appearance: &Appearance) -> String {
    match appearance.equipped() {
        Some(skin) => format!("Wearing the {} skin", variant_label(&skin.variant).to_lowercase()),
        None => "Minecraft does not say which skin is in force.".to_string(),
    }
}

/// `CLASSIC` and `SLIM` as a reader reads them.
///
/// The document's words are the format's; the reference's are the modal's own two
/// arm-style labels, and they are the same two things.
fn variant_label(variant: &str) -> String {
    if variant.eq_ignore_ascii_case("SLIM") {
        Key::AppSkinsModalArmStyleSlim.message().to_string()
    } else {
        Key::AppSkinsModalArmStyleWide.message().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sections_are_the_reference_s_own_and_carry_its_names() {
        assert_eq!(Section::ALL.len(), 13);
        // No two sections show the same heading: a list built from one key twice
        // would look deliberate and be wrong.
        let mut labels: Vec<&str> = Section::ALL.iter().map(|section| section.label()).collect();
        let count = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(count, labels.len());
        assert_eq!(Section::DefaultSkins.label(), "Default skins");
        assert_eq!(Section::ModrinthPride.label(), "Modrinth Pride");
        for section in Section::ALL {
            assert_eq!(section.label(), section.key().message());
            assert!(section.key().name().starts_with("app.skins.section."));
        }
    }

    #[test]
    fn a_section_header_toggles_its_own_section_and_leaves_the_others() {
        // The reference opens every section it knows on its first pass and each
        // header toggles only itself; the one-open accordion this page carried
        // before is gone with this state.
        let mut state = State::default();
        assert!(state.open.is_open(Section::SavedSkins));
        assert!(state.open.is_open(Section::TheCopperAge));
        state.update(Message::Select(Section::Modrinth));
        assert!(!state.open.is_open(Section::Modrinth), "its own header closes it");
        assert!(state.open.is_open(Section::SavedSkins), "and leaves the others");
        state.update(Message::Select(Section::Modrinth));
        assert!(state.open.is_open(Section::Modrinth), "pressing again opens it");
    }

    #[test]
    fn the_demo_banner_is_the_reference_s_own_box() {
        // `p-4 pt-0` on the block, `rounded-[20px] border border-surface-5
        // bg-surface-3 p-4` on the box inside it, `gap-3` between the icon and
        // the words, `gap-1` between the two lines, `size-6` on the icon, and
        // `text-lg` over `text-base` on a `leading-6` line.
        assert_eq!(BANNER_PAD, 16.0);
        assert_eq!(BANNER_RADIUS, 20.0);
        assert_eq!(BANNER_INSET, 16.0);
        assert_eq!(BANNER_GAP, 12.0);
        assert_eq!(BANNER_LINE_GAP, 4.0);
        assert_eq!(BANNER_ICON, 24.0);
        assert_eq!(BANNER_TITLE, 18.0);
        assert_eq!(BANNER_DESCRIPTION, 16.0);
        assert_eq!(BANNER_LINE, 24.0);
        // `max-w-5xl`, the one number the box cannot honour -- and the reason
        // that is a number rather than a guess: this shell's pane is 905 at a
        // 1280-pixel window (1280 less the 64 rail and the 300 panel), and 873
        // after the banner's own padding, which is under the cap by 151.
        assert_eq!(BANNER_MAX, 1024.0);
        assert!(BANNER_MAX > 905.0 - 2.0 * BANNER_PAD);
    }

    #[test]
    fn the_demo_banner_is_drawn_only_where_the_reference_draws_it() {
        // The reference's own `v-if="!currentUser"`, and this page's reading of
        // the same state: the answer a read with no Microsoft session gets,
        // which is a sentence rather than a request.
        let store = Store::default();
        assert!(demo_banner(Gen::ALL[0], &State::default()).is_none(), "not asked for yet");
        let waiting = State { appearance: Load::Loading, ..State::default() };
        assert!(demo_banner(Gen::ALL[0], &waiting).is_none(), "still waiting on the read");
        let signed_out = State {
            appearance: Load::Failed("Sign in to a Microsoft account to see the skins it owns.".into()),
            ..State::default()
        };
        for theme in Gen::ALL {
            assert!(demo_banner(*theme, &signed_out).is_some(), "nobody is signed in");
            drop(view(*theme, &signed_out, &store));
        }
        // An account's own appearance in hand is the other state, and the
        // banner is not in it.
        let signed_in = State {
            appearance: Load::Ready(crate::skin::Appearance::of(
                "Steve",
                palantir_net::MinecraftSkins { skins: vec![], capes: vec![] },
                Err("the network is down".into()),
            )),
            ..State::default()
        };
        assert!(demo_banner(Gen::ALL[0], &signed_in).is_none());
    }

    

    #[test]
    fn the_add_cell_s_dashed_ring_is_the_reference_s_own_pattern() {
        // Reading down the reference's own left border at x 361 from y 131
        // gives `##....` over and over: two pixels of `--surface-5` and four of
        // nothing.
        assert_eq!(DASH_ON, 2.0);
        assert_eq!(DASH_OFF, 4.0);
        assert_eq!(DASH_PERIOD, 6.0);
        // The ring is two strips of the card's own height around the words, so
        // the cell is still the 249 its own aspect gives at this pane.
        assert_eq!(2.0 + (CARD_HEIGHT - 2.0), CARD_HEIGHT);
        assert_eq!(DASHES_ACROSS, 32);
        assert_eq!(DASHES_DOWN, 41);
        // And the periods those counts give over this cell's own inside --
        // 190.3 wide, 247 tall: 5.95 and 6.02, against the six a capture reads.
        let across = 190.3 / DASHES_ACROSS as f32;
        assert!((across - DASH_PERIOD).abs() < 0.1, "{across} is not six");
        let down = (CARD_HEIGHT - 2.0) / DASHES_DOWN as f32;
        assert!((down - DASH_PERIOD).abs() < 0.1, "{down} is not six");
        // And the share a dash takes of its own period: one of the three, which
        // is the one-and-two the strips are built from.
        assert_eq!(DASH_ON / DASH_PERIOD, 1.0 / 3.0);
    }

    #[test]
    fn the_add_cell_s_two_lines_take_the_reference_s_own_line_heights() {
        // `leading-6` over `leading-5`, and the group's own 94: a 32-pixel plus,
        // `gap-4`, a 24-pixel line, `gap-0.5` and a 20-pixel line.
        assert_eq!(ADD_ICON + ADD_ICON_GAP + ADD_LABEL_LINE + ADD_LINE_GAP + ADD_SUBTITLE_LINE, 94.0);
        // And which is what puts the plus where the reference puts it. The group is
        // centred in the cell, so it starts 77.5 below the cell's top, and the
        // plus's ink -- four rows of it in the reference's capture, y203..206 --
        // sits in the middle of its own 32-pixel box, 14 further down:
        // 112 + 77.5 + 14 = 203.5, which is the reference's 203. Left to
        // iced's own lines the group is five shorter, its top is 2.5 lower and
        // the capture reads y207.
        let ink = 4.0;
        let group_top = 112.0 + (CARD_HEIGHT - 94.0) / 2.0;
        let ink_top = group_top + (ADD_ICON - ink) / 2.0;
        assert!((ink_top - 203.0).abs() < 1.0, "{ink_top} is not the reference's 203");
    }

    #[test]
    fn the_banner_s_sign_in_says_what_it_cannot_do() {
        // The reference's button opens a sign-in window; this launcher's flow is
        // not built, so the press says so in the page's own notice rather than
        // being swallowed.
        let mut state = State::default();
        assert_eq!(state.update(Message::SignIn), None, "and it asks nobody for it");
        let notice = state.notice.clone().expect("the press says something");
        assert!(notice.contains("Signing in to Minecraft"), "got {notice}");
        assert!(notice.ends_with("is not implemented yet."), "got {notice}");
    }

    #[test]
    fn the_page_asks_once_and_keeps_the_answer_that_answers_it() {
        let mut state = State::default();
        assert_eq!(state.appearance, Load::Idle, "nothing asked for yet");
        let asked = state.opening().expect("a first request");
        assert_eq!(asked.round, 1);
        assert_eq!(state.appearance, Load::Loading, "and it is waiting on it");
        assert!(state.opening().is_none(), "asked once, not once per frame");
        // An answer to a question the page has replaced is dropped rather than
        // drawn: the round travels with it, which is what makes that decidable.
        assert!(state.update(Message::Found { round: 0, result: Err("old".into()) }).is_none());
        assert_eq!(state.appearance, Load::Loading, "so the stale one is not drawn");
        // The answer that does answer it becomes the page's own state, and a
        // failure stays a sentence rather than becoming an empty gallery.
        let appearance = crate::skin::Appearance::of(
            "Steve",
            palantir_net::MinecraftSkins::default(),
            Err("the network is down".into()),
        );
        // What arrives is the two halves the shell read in one turn: the account's
        // appearance, and the launcher's own stored skins. Nothing is stored in this
        // test, so the second half is empty.
        let loaded = Loaded { appearance, saved: Vec::new() };
        state.update(Message::Found { round: asked.round, result: Ok(Box::new(loaded)) });
        assert!(matches!(state.appearance, Load::Ready(_)));
        state.update(Message::Found { round: asked.round, result: Err("signed out".into()) });
        assert_eq!(state.appearance, Load::Failed("signed out".into()));
    }

    #[test]
    fn adding_a_skin_asks_the_shell_for_a_file_and_takes_one_at_a_time() {
        // The page's half of an upload: one request, carrying the round, and no
        // second one while the first is out. The dialog is the shell's (`Add`), and
        // what the reader chose never passes through here -- only the round does.
        let mut state = State::default();
        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("the press asks the shell for a file");
        };
        assert_eq!(add.round, 1);
        assert!(state.wearing, "and the page waits on it");
        assert_eq!(state.notice, None);
        assert_eq!(state.update(Message::AddSkin), None, "and a second press is dropped");
        assert!(state.wearing);
    }

    #[test]
    fn the_three_answers_to_that_request_draw_three_different_pages() {
        // Cancelled: the press is finished and nothing is said. Failed: a sentence in
        // the slot every other failure goes. No picker: also a sentence, and a
        // different one -- the reader did nothing wrong.
        let mut state = State::default();
        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("the press asks the shell for a file");
        };
        assert_eq!(
            state.update(Message::Added { round: add.round, picked: Picked::Cancelled }),
            None,
            "a cancel asks for nothing"
        );
        assert!(!state.wearing);
        assert_eq!(state.notice, None, "and says nothing");

        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("asked again");
        };
        assert_eq!(
            state.update(Message::Added {
                round: add.round,
                picked: Picked::Done(Err("Minecraft refused the change: not a skin".to_string())),
            }),
            None
        );
        assert!(!state.wearing);
        assert!(state.notice.as_deref().unwrap_or_default().contains("not a skin"));

        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("asked a third time");
        };
        state.update(Message::Added {
            round: add.round,
            picked: Picked::NoPicker("This build has no file picker".to_string()),
        });
        assert!(!state.wearing);
        assert!(state.notice.as_deref().unwrap_or_default().contains("no file picker"));
    }

    #[test]
    fn an_upload_that_worked_reloads_the_lists_and_a_stale_answer_is_dropped() {
        // The same rule a successful wear follows, from the same place: what changed is
        // Minecraft's document, so the document is what is read again.
        let mut state = State::default();
        let Some(Ask::AddSkin(add)) = state.update(Message::AddSkin) else {
            panic!("the press asks the shell for a file");
        };
        // An answer to a request the page has replaced is dropped: the round is what
        // makes that decidable.
        assert_eq!(
            state.update(Message::Added { round: 0, picked: Picked::Cancelled }),
            None
        );
        assert!(state.wearing, "so the page is still waiting");
        let Some(Ask::Skins(asked)) = state
            .update(Message::Added { round: add.round, picked: Picked::Done(Ok(())) })
        else {
            panic!("an upload that worked reloads");
        };
        assert_eq!(asked.round, add.round + 1);
        assert_eq!(state.appearance, Load::Loading);
        assert!(!state.wearing, "the request is finished even while the read is out");
        assert_eq!(state.notice, None, "a success is not a sentence");
    }

    #[test]
    fn wearing_something_is_one_request_and_the_second_press_is_not_a_second_write() {
        let mut state = State::default();
        let change = SkinChange::Cape { id: "cape-1".to_string() };
        let Some(Ask::Wear(worn)) = state.update(Message::Wear(change.clone())) else {
            panic!("the press asks the shell");
        };
        assert_eq!(worn.round, 1);
        assert_eq!(worn.change, change);
        assert!(state.wearing, "and the page is waiting on it");
        // A second press while the first is in flight is dropped, not sent: the
        // rows draw their buttons unusable, and this is the same rule where it
        // cannot be sidestepped.
        assert_eq!(state.update(Message::Wear(change.clone())), None);
        assert!(state.wearing);
        // A failure is a sentence, and the page is ready to press again.
        let answered = state.update(Message::Applied {
            round: 1,
            result: Err("Minecraft refused the change: nope".to_string()),
        });
        assert_eq!(answered, None);
        assert!(!state.wearing);
        assert!(state.notice.as_deref().unwrap_or_default().contains("nope"));
        // And an answer to a change the page has replaced is dropped rather than
        // reported: the round is what makes that decidable.
        assert_eq!(
            state.update(Message::Applied { round: 0, result: Err("old".to_string()) }),
            None
        );
        assert!(state.notice.as_deref().unwrap_or_default().contains("nope"));
    }

    #[test]
    fn a_change_that_worked_reloads_the_lists_instead_of_saying_so() {
        let mut state = State::default();
        let Some(Ask::Wear(worn)) = state.update(Message::Wear(SkinChange::NoCape)) else {
            panic!("the press asks the shell");
        };
        // The answer to the write is not the answer to the read: the page asks
        // again, and the reload is what shows the check on the row that is now in
        // force.
        let Some(Ask::Skins(asked)) = state.update(Message::Applied { round: worn.round, result: Ok(()) }) else {
            panic!("a change that worked reloads");
        };
        assert_eq!(asked.round, worn.round + 1);
        assert_eq!(state.appearance, Load::Loading);
        assert!(!state.wearing, "the press is finished even while the read is out");
        assert_eq!(state.notice, None, "a success is not a sentence");
    }

    #[test]
    fn the_rows_offer_to_wear_everything_that_is_not_already_on() {
        // The rule the drawn rows follow, read where it can be: a skin already in
        // force is not offered, a cape already on is not offered, and neither is
        // anything at all while a change is in flight.
        let worn_skin = palantir_net::MinecraftSkin {
            id: "skin-1".to_string(),
            state: "ACTIVE".to_string(),
            url: "http://textures.minecraft.net/texture/aaa".to_string(),
            variant: "CLASSIC".to_string(),
        };
        let cold_skin = palantir_net::MinecraftSkin {
            state: "INACTIVE".to_string(),
            ..worn_skin.clone()
        };
        assert!(wear_skin(&worn_skin, false).is_none());
        let Some(Message::Wear(SkinChange::Skin { variant, url })) = wear_skin(&cold_skin, false)
        else {
            panic!("a skin that is not on is offered");
        };
        assert_eq!(variant, "CLASSIC");
        assert!(url.ends_with("aaa"));
        assert!(wear_skin(&cold_skin, true).is_none(), "not while one is in flight");
        let cape = palantir_net::MinecraftCape {
            id: "cape-1".to_string(),
            state: "INACTIVE".to_string(),
            url: "http://textures.minecraft.net/texture/ccc".to_string(),
            alias: "Migrator".to_string(),
        };
        assert!(matches!(
            wear_cape(&cape, false, false),
            Some(Message::Wear(SkinChange::Cape { id })) if id == "cape-1"
        ));
        assert!(wear_cape(&cape, true, false).is_none(), "it is already on");
        assert!(wear_cape(&cape, false, true).is_none(), "something is in flight");
    }

    #[test]
    fn the_page_draws_in_every_theme_and_in_both_of_its_shapes() {
        let store = Store::default();
        for theme in Gen::ALL {
            // Every section open, which is the reference's own first state, and
            // then a page with two of them closed.
            drop(view(*theme, &State::default(), &store));
            let mut closed = State::default();
            closed.open.toggle(Section::SavedSkins);
            closed.open.toggle(Section::ModrinthPride);
            drop(view(*theme, &closed, &store));
            let state = State { notice: Some("x".into()), ..State::default() };
            drop(view(*theme, &state, &store));
            // And with an account's appearance in hand, which is the shape the page
            // is drawn in after the answer arrives -- including the one where the
            // picture could not be made.
            let appearance = crate::skin::Appearance::of(
                "Steve",
                palantir_net::MinecraftSkins {
                    skins: vec![palantir_net::MinecraftSkin {
                        id: "skin-1".into(),
                        state: "ACTIVE".into(),
                        url: "http://textures.minecraft.net/texture/aaa".into(),
                        variant: "SLIM".into(),
                    }],
                    capes: vec![palantir_net::MinecraftCape {
                        id: "cape-1".into(),
                        state: "ACTIVE".into(),
                        url: "http://textures.minecraft.net/texture/ccc".into(),
                        alias: "Migrator".into(),
                    }],
                },
                Err("the network is down".into()),
            );
            let state = State {
                appearance: Load::Ready(appearance),
                ..State::default()
            };
            drop(view(*theme, &state, &store));
        }
    }

    #[test]
    fn opening_the_editor_reads_the_row_it_was_pressed_on() {
        let row = SavedRow {
            entry: saved_skins::Entry {
                key: "abc".to_string(),
                name: "my skin".to_string(),
                variant: "SLIM".to_string(),
                cape: "cape-1".to_string(),
                source: saved_skins::Source::Custom,
                file: "abc.png".to_string(),
            },
            ears: true,
        };
        let mut state = State { saved: vec![row], ..State::default() };
        // A press on a row the page does not hold asks for nothing and opens nothing.
        assert_eq!(state.update(Message::Edit { key: "nope".to_string() }), None);
        assert!(state.edit.is_none());
        state.update(Message::Edit { key: "abc".to_string() });
        let edit = state.edit.clone().expect("the editor opened on the row");
        assert_eq!(edit.key, "abc");
        assert_eq!(edit.name, "my skin");
        assert_eq!(edit.variant, "SLIM");
        assert_eq!(edit.cape, "cape-1");
        assert!(edit.ears, "and the marker travels with it");
        // The two choices are the modal's own state, and neither asks anybody anything.
        assert_eq!(state.update(Message::ArmStyle { variant: "CLASSIC" }), None);
        assert_eq!(state.update(Message::Cape { id: "cape-2".to_string() }), None);
        let edit = state.edit.as_ref().expect("still open");
        assert_eq!(edit.variant, "CLASSIC");
        assert_eq!(edit.cape, "cape-2");
        assert_eq!(state.update(Message::CloseEdit), None);
        assert!(state.edit.is_none(), "and closing it is not a request either");
    }

    #[test]
    fn each_of_the_editors_three_actions_travels_as_one_ask() {
        for act in [Act::Save, Act::Forget, Act::TakeOff] {
            let opened = Edit {
                round: 7,
                key: "abc".to_string(),
                name: "my skin".to_string(),
                variant: "CLASSIC".to_string(),
                cape: String::new(),
                stored: Stored::default(),
                ears: false,
                act: Act::Save,
                confirm: false,
            };
            let mut state = State { edit: Some(opened), ..State::default() };
            let Some(Ask::EditSkin(edit)) = state.update(Message::Act(act)) else {
                panic!("the press asks the shell: {act:?}");
            };
            assert_eq!(edit.act, act);
            assert_eq!(edit.round, 7, "the round the answer is matched by");
            assert!(state.wearing, "and the page waits on it");
            // A second press while the first is out is dropped, not sent.
            assert_eq!(state.update(Message::Act(Act::Forget)), None);
            assert!(state.wearing);
        }
        // With nothing open there is nothing to ask for.
        let mut state = State::default();
        assert_eq!(state.update(Message::Act(Act::Save)), None);
    }

    #[test]
    fn the_editors_answer_closes_it_and_a_success_reloads() {
        // The modal opens inside a round rather than starting one, so the answer that
        // comes back carries the round the page is already on.
        let opened = Edit {
            round: 3,
            key: "abc".to_string(),
            name: "my skin".to_string(),
            variant: "CLASSIC".to_string(),
            cape: String::new(),
            stored: Stored::default(),
            ears: false,
            act: Act::Save,
            confirm: false,
        };
        let mut state = State { round: 3, edit: Some(opened), wearing: true, ..State::default() };
        let Some(Ask::Skins(asked)) = state.update(Message::Edited { round: 3, result: Ok(()) })
        else {
            panic!("a write that worked reloads");
        };
        assert_eq!(asked.round, 4);
        assert!(state.edit.is_none(), "the modal closes either way");
        assert!(!state.wearing);
        assert_eq!(state.notice, None, "a success is not a sentence");

        // A failure is a sentence in the page's own slot, and the modal still closes.
        state.round = 4;
        state.edit = Some(Edit {
            round: 4,
            key: "abc".to_string(),
            name: "my skin".to_string(),
            variant: "CLASSIC".to_string(),
            cape: String::new(),
            stored: Stored::default(),
            ears: false,
            act: Act::Forget,
            confirm: false,
        });
        state.wearing = true;
        assert_eq!(
            state.update(Message::Edited { round: 4, result: Err("gone".to_string()) }),
            None
        );
        assert!(state.edit.is_none());
        assert!(!state.wearing);
        assert!(state.notice.as_deref().unwrap_or_default().contains("gone"));
        // And an answer to an edit the page has replaced is dropped rather than drawn.
        assert_eq!(
            state.update(Message::Edited { round: 99, result: Err("old".to_string()) }),
            None
        );
        assert!(state.notice.as_deref().unwrap_or_default().contains("gone"));
    }

    /// A 64x64 skin texture carrying the Ears marker, as the store would hold it.
    ///
    /// `skin::ears_of` reads the pixel at `(0, 32)` and ignores the alpha, so one
    /// byte triple is the whole of a skin that asks for the mod.
    fn ears_texture() -> Vec<u8> {
        let mut texture = ::image::RgbaImage::from_pixel(64, 64, ::image::Rgba([0, 0, 0, 0]));
        texture.put_pixel(0, 32, ::image::Rgba([0x3F, 0x23, 0xD8, 0xFF]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        ::image::DynamicImage::ImageRgba8(texture)
            .write_to(&mut bytes, ::image::ImageFormat::Png)
            .expect("a texture this page can decode");
        bytes.into_inner()
    }

    #[test]
    fn the_notice_is_drawn_under_the_model_and_not_in_the_editor() {
        // The reference puts the Ears notice in the preview's `#subtitle` slot and
        // nowhere else, and the pane can only see the skin in force -- so that is
        // where it is drawn, for a texture that carries the marker.
        let appearance = crate::skin::Appearance::of(
            "Steve",
            palantir_net::MinecraftSkins { skins: vec![], capes: vec![] },
            Ok(ears_texture()),
        );
        assert!(appearance.ears.is_some(), "the marker is read");
        let ready = Load::Ready(appearance.clone());
        let mut state = State { appearance: ready, ..State::default() };
        state.saved = vec![stored("abc")];
        drop(view(Gen::ALL[0], &state, &Store::default()));
        // And an editor on that row draws the editor, with no notice in it: the
        // dialog's own body is Texture, Arm style, Cape, the ears line and the
        // actions, and the notice is not one of them in the reference either.
        let edit = Edit {
            round: 1,
            key: "abc".to_string(),
            name: "abc".to_string(),
            variant: "CLASSIC".to_string(),
            cape: String::new(),
            stored: Stored::default(),
            ears: true,
            act: Act::Save,
            confirm: false,
        };
        drop(edit_view(Gen::ALL[0], &edit, &[], false));
        // The marker is what the notice is for, so a texture without it is a page
        // without one.
        let plain = crate::skin::Appearance::of(
            "Steve",
            palantir_net::MinecraftSkins { skins: vec![], capes: vec![] },
            Err("no texture".into()),
        );
        assert!(plain.ears.is_none());
    }

    #[test]
    fn the_ears_notices_link_opens_the_mods_own_project() {
        let mut state = State::default();
        let Some(Ask::Open(Open::Project(id))) = state.update(Message::OpenEars) else {
            panic!("the link is a navigation");
        };
        assert_eq!(id, EARS_PROJECT);
        assert_eq!(id, "mfzaZK3Z", "the project the reference's own link names");
    }

    #[test]
    fn the_editor_draws_in_every_theme_and_around_both_answers_to_ears() {
        let capes = vec![palantir_net::MinecraftCape {
            id: "cape-1".to_string(),
            state: "ACTIVE".to_string(),
            url: String::new(),
            alias: "Migrator".to_string(),
        }];
        for theme in Gen::ALL {
            for ears in [false, true] {
                let edit = Edit {
                    round: 1,
                    key: "abc".to_string(),
                    name: "my skin".to_string(),
                    variant: "SLIM".to_string(),
                    cape: String::new(),
                    stored: Stored { variant: "CLASSIC".to_string(), cape: String::new() },
                    ears,
                    act: Act::Save,
                    confirm: false,
                };
                // With capes to choose and with none: an account that owns no cape still
                // gets the modal's own "None".
                drop(edit_view(*theme, &edit, &capes, false));
                drop(edit_view(*theme, &edit, &[], true));
            }
        }
    }

    /// One cape, as the service names it.
    fn cape(id: &str, alias: &str) -> palantir_net::MinecraftCape {
        palantir_net::MinecraftCape {
            id: id.to_string(),
            state: "AVAILABLE".to_string(),
            url: String::new(),
            alias: alias.to_string(),
        }
    }

    #[test]
    fn the_editors_actions_are_three_controls_with_three_names() {
        // The hover clock is keyed per control, so two buttons that shared a name
        // would light together on one hover. The reference's own Save and Delete
        // and this launcher's take-off each take a name of their own, and all
        // three live under the editor's own namespace. The texture section's
        // button is a fourth, and takes a fourth.
        let names = ["save", "takeoff", "forget", "replace-texture"];
        let scoped: Vec<&str> = names.iter().map(|name| ui::scoped(EDIT_KEY, name)).collect();
        let mut unique = scoped.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), names.len(), "got {scoped:?}");
    }

    #[test]
    fn forgetting_a_skin_asks_first_and_the_answer_goes_back_to_the_editor() {
        let mut state = State { saved: vec![stored("abc")], ..State::default() };
        state.update(Message::Edit { key: "abc".to_string() });
        // The delete button asks, and asking is not a write: nothing leaves and
        // the editor stays open.
        assert_eq!(state.update(Message::Forget), None, "a question asks nobody");
        let edit = state.edit.as_ref().expect("the editor is still open");
        assert!(edit.confirm, "and it is asking");
        // Changing your mind goes back to the editor rather than closing it, which
        // is what the reference's own Cancel does over its editor.
        assert_eq!(state.update(Message::CancelForget), None);
        let edit = state.edit.as_ref().expect("still open");
        assert!(!edit.confirm, "and it stops asking");
        // And the second time round, the write is one press from the ask.
        state.update(Message::Forget);
        let Some(Ask::EditSkin(edit)) = state.update(Message::Act(Act::Forget)) else {
            panic!("proceeding is the write");
        };
        assert_eq!(edit.act, Act::Forget);
        assert!(state.wearing, "and the page waits on it");
        // A press with nothing open is nothing, in either direction.
        let mut empty = State::default();
        assert_eq!(empty.update(Message::Forget), None);
        assert_eq!(empty.update(Message::CancelForget), None);
    }

    #[test]
    fn the_texture_sections_press_says_what_it_cannot_do() {
        // The control is the reference's own, and it cannot be wired: the only ask
        // this page can raise for a file adds a row, and a replace would leave the
        // reader with two of the skin they were editing. So it says so, the way the
        // sign-in button does.
        let mut state = State::default();
        assert_eq!(state.update(Message::ReplaceTexture), None, "and it asks nobody");
        let notice = state.notice.clone().expect("the press says something");
        assert!(notice.contains("Replacing a skin's texture"), "got {notice}");
        assert!(notice.ends_with("is not implemented yet."), "got {notice}");
    }

    #[test]
    fn save_is_only_offered_once_the_editor_holds_an_edit() {
        // The modal opens on the row as the store holds it, so nothing has been
        // changed and `hasEdits` is false: Save is the reference's own disabled
        // button, which it draws rather than hides.
        let mut state = State { saved: vec![stored("abc")], ..State::default() };
        state.update(Message::Edit { key: "abc".to_string() });
        let edit = state.edit.clone().expect("the editor opened on the row");
        assert_eq!(edit.variant, edit.stored.variant, "it opened on the row's own arm style");
        assert_eq!(edit.cape, edit.stored.cape, "and on the row's own cape");
        assert!(!has_edits(&edit), "so there is nothing to save yet");
        // The two choices the modal owns are the two that can make an edit.
        state.update(Message::ArmStyle { variant: "SLIM" });
        let edit = state.edit.as_ref().expect("still open");
        assert!(has_edits(edit), "a different arm style is one");
        state.update(Message::ArmStyle { variant: "CLASSIC" });
        state.update(Message::Cape { id: "cape-9".to_string() });
        let edit = state.edit.as_ref().expect("still open");
        assert!(has_edits(edit), "and a different cape is the other");
        // Putting both back is not an edit, and neither is choosing the row's own
        // cape id where it already was.
        state.update(Message::Cape { id: String::new() });
        assert!(!has_edits(state.edit.as_ref().expect("still open")));
    }

    #[test]
    fn the_editors_blocks_are_the_reference_s_own_two_gaps_apart() {
        // `flex flex-col gap-4` between the sections, and `mb-2` under each of
        // their own headings -- so a heading is 8 from what it labels and 16 from
        // the section above it, which is the pairing the reference's column reads as.
        assert_eq!(EDITOR_SECTION_GAP, 16.0, "`gap-4`");
        assert_eq!(EDITOR_HEADING_GAP, 8.0, "`mb-2`");
        assert_ne!(EDITOR_SECTION_GAP, GAP, "which is the page-wide 12 it was before");
        // And the two numbers are what the editor is built from: two sections and
        // the actions, each a block of its own.
        let edit = Edit {
            round: 1,
            key: "abc".to_string(),
            name: "my skin".to_string(),
            variant: "CLASSIC".to_string(),
            cape: String::new(),
            stored: Stored::default(),
            ears: true,
            act: Act::Save,
            confirm: false,
        };
        drop(edit_view(Gen::ALL[0], &edit, &[], false));
    }

    #[test]
    fn the_cape_list_is_ours_first_and_then_the_account_s_capes_by_name() {
        // `sortedCapes` compares the names folded, and an unnamed cape has nothing to
        // compare, so it sorts ahead of every named one.
        let capes = vec![cape("z", "Zebra"), cape("m", "migrator"), cape("u", "")];
        let rows = cape_rows(&capes);
        let names: Vec<Option<String>> =
            rows[0].iter().map(|cell| cell.map(cape_sort_name)).collect();
        assert_eq!(
            names,
            vec![
                None,
                Some(String::new()),
                Some("migrator".to_string()),
                Some("zebra".to_string()),
            ],
            "none first, then the names folded, and an unnamed cape has nothing to compare"
        );
        // An account that owns no cape at all still gets the reference's own cell,
        // and nothing beside it.
        let rows = cape_rows(&[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 1);
        assert!(rows[0][0].is_none(), "which is the no-cape cell");
        // And a cell is labelled with the service's name, or the reference's own word.
        assert_eq!(cape_name(&cape("m", "Migrator")), "Migrator");
        assert_eq!(cape_name(&cape("u", "")), "Cape");
    }

    #[test]
    fn the_cape_list_is_four_to_a_row_and_never_padded_with_a_second_none() {
        let capes: Vec<palantir_net::MinecraftCape> =
            (0..9).map(|n| cape(&format!("c{n}"), &format!("cape {n}"))).collect();
        // Nine capes and the no-cape cell are ten cells, which is two rows of four
        // and one of two.
        let rows = cape_rows(&capes);
        let lengths: Vec<usize> = rows.iter().map(Vec::len).collect();
        assert_eq!(lengths, vec![4, 4, 2], "and the last row is short rather than padded");
        let cells: usize = rows.iter().map(Vec::len).sum();
        assert_eq!(cells, capes.len() + 1, "one cell per cape, and ours");
        let nones: usize = rows.iter().flatten().filter(|cell| cell.is_none()).count();
        assert_eq!(nones, 1, "the no-cape cell is drawn once, not once per row");
    }

    #[test]
    fn the_saved_section_draws_a_row_per_stored_skin_and_an_empty_state() {
        let store = Store::default();
        // The Saved-skins section is the first the list draws -- `Skins.vue` puts
        // it before every pack section -- and with nothing stored it draws its add
        // cell, which is the reference's own empty state.
        assert_eq!(Section::ALL[0], Section::SavedSkins);
        drop(view(Gen::ALL[0], &State::default(), &store));
        let state = State { saved: vec![stored("abc")], ..State::default() };
        drop(view(Gen::ALL[0], &state, &store));
    }

    /// One stored row, as the page draws it.
    fn stored(key: &str) -> SavedRow {
        SavedRow {
            entry: saved_skins::Entry {
                key: key.to_string(),
                name: key.to_string(),
                variant: "CLASSIC".to_string(),
                cape: String::new(),
                source: saved_skins::Source::Custom,
                file: format!("{key}.png"),
            },
            ears: false,
        }
    }

    #[test]
    fn a_move_swaps_a_row_with_its_neighbour_and_asks_the_shell_once() {
        // The reference reorders this list by dragging a row anywhere; this launcher's
        // control is a chevron per row, so the order asked for is always the list on
        // screen with two neighbours swapped -- and it travels as the whole order,
        // which is the write the reference's `set_custom_skin_order` makes too.
        let mut state = State {
            round: 4,
            saved: vec![stored("a"), stored("b"), stored("c")],
            ..State::default()
        };
        let Some(Ask::Reorder(order)) = state.update(Message::Move {
            key: "b".to_string(),
            step: Step::Up,
        }) else {
            panic!("the press asks the shell");
        };
        assert_eq!(order.round, 5, "the answer is matched to this press");
        assert_eq!(order.keys, ["b", "a", "c"], "one place toward the front");
        assert!(state.wearing, "and the page waits on the write");
        assert_eq!(state.notice, None);

        // A second press while the first is out is dropped rather than queued: two
        // reorders would be two read-modify-writes of the same index.
        assert_eq!(
            state.update(Message::Move { key: "c".to_string(), step: Step::Up }),
            None
        );
        assert!(state.wearing);
    }

    #[test]
    fn the_ends_of_the_list_do_not_move_and_a_key_the_page_does_not_hold_is_nothing() {
        // The rule `moved` states, read where a test can: the first row has no place
        // to move up into and the last none to move down into, so such a press is not
        // an ask -- it is not even a write that changes nothing -- and the row is
        // drawn without the control that would send it.
        let rows = vec![stored("a"), stored("b")];
        assert_eq!(moved(&rows, "a", Step::Up), None);
        assert_eq!(moved(&rows, "b", Step::Down), None);
        assert_eq!(moved(&rows, "missing", Step::Up), None);
        assert_eq!(moved(&rows, "a", Step::Down), Some(vec!["b".into(), "a".into()]));

        let mut state = State { saved: rows, ..State::default() };
        assert_eq!(
            state.update(Message::Move { key: "a".to_string(), step: Step::Up }),
            None
        );
        assert!(!state.wearing, "and nothing is waiting");
    }

    #[test]
    fn a_reorder_that_worked_reloads_the_list_and_one_that_failed_says_so() {
        let mut state = State { round: 9, wearing: true, ..State::default() };
        let Some(Ask::Skins(asked)) = state.update(Message::Reordered { round: 9, result: Ok(()) })
        else {
            panic!("an order that was written reloads the store");
        };
        assert_eq!(asked.round, 10);
        assert!(!state.wearing);
        assert_eq!(state.notice, None, "a success is not a sentence");

        state.round = 10;
        state.wearing = true;
        assert_eq!(
            state.update(Message::Reordered {
                round: 10,
                result: Err("cannot write 'skins/index.json': disk full".to_string()),
            }),
            None
        );
        assert!(!state.wearing);
        let notice = state.notice.clone().expect("a failure is a sentence");
        assert!(
            notice.contains("Failed to reorder skins"),
            "the reference's own title comes first: {notice}"
        );
        assert!(
            notice.contains("disk full"),
            "and the store's refusal is the half that names what failed: {notice}"
        );
        // An answer to a move the page has replaced is dropped rather than drawn.
        state.notice = None;
        assert_eq!(
            state.update(Message::Reordered { round: 99, result: Err("old".to_string()) }),
            None
        );
        assert_eq!(state.notice, None, "the stale answer changed nothing");
    }
}
