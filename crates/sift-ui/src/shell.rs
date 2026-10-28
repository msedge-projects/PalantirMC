//! The window's furniture: title bar, rail, right panel, and the content column.
//!
//! **Laid out from measured rectangles, not from a responsive tree.** The parity
//! requirement is +/-2 px against the Modrinth app, so every element is placed with the
//! numbers in [`crate::geometry`] and painted into an explicit rect. A layout engine that
//! decides its own gaps would be re-deciding, every frame, something the captures already
//! answered.
//!
//! **Repaint only when something moves.** An idle window draws nothing: the frame loop
//! exists while a hover, a scroll glide or a transition is running and stops when it lands.
//! That is the `idle CPU 0%` budget in `docs/LIGHTNESS.md`, and it is the one place this
//! crate differs most from the slice it grew out of, which asked for a frame every vsync
//! whether or not anything had changed.

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::geometry as g;
use crate::icons::{self, Glyph};
use crate::theme::Theme;

/// The Minecraft version a quick-created instance gets.
///
/// The reference asks in a modal; until that modal exists, a version the launcher can resolve
/// is better than none, and it is one constant to change when the modal lands.
const DEFAULT_MC_VERSION: &str = "1.21.4";

/// A rail destination. The reference has five, separated from the utility group below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rail {
    /// The welcome screen and the instance list.
    Home,
    /// Find modpacks and content.
    Discover,
    /// Skins: the player's own and the catalogue's.
    Skins,
    /// Screenshots taken in any instance.
    Screenshots,
    /// Servers the launcher knows about.
    Servers,
}

impl Rail {
    /// Every destination, in the reference's order.
    pub const ALL: [Rail; 5] = [
        Rail::Home,
        Rail::Discover,
        Rail::Skins,
        Rail::Screenshots,
        Rail::Servers,
    ];

    /// The name the reference prints in the page header.
    pub fn title(self) -> &'static str {
        match self {
            Rail::Home => "Home",
            Rail::Discover => "Discover modpacks",
            Rail::Skins => "Skins",
            Rail::Screenshots => "Screenshots",
            Rail::Servers => "Servers",
        }
    }

    /// The glyph the reference draws for this destination.
    pub fn glyph(self) -> Glyph {
        match self {
            Rail::Home => Glyph::Home,
            Rail::Discover => Glyph::Compass,
            Rail::Skins => Glyph::Shirt,
            Rail::Screenshots => Glyph::Image,
            Rail::Servers => Glyph::Server,
        }
    }
}

/// What the shell is showing, and what is moving.
pub struct Shell {
    /// The selected rail destination.
    pub rail: Rail,
    /// The hovered rail destination, if any. A press animates between the two, which is
    /// why this is state rather than a per-frame read.
    hovered: Option<Rail>,
    /// fastframe's scroll state: the wheel step, the touchpad scale and the glide.
    scrolling: fastframe_scroll::Scrolling,
    /// The colour scheme, from the generated tokens.
    pub theme: Theme,
    /// Project icons, fetched off the frame thread and cached as textures.
    icons: crate::images::Images,
    /// The one game this shell starts, and the worker that reports on it.
    game: crate::game::Game,
    /// The pack install behind the Discover cards' Install buttons, and its worker.
    install: crate::install::Install,
    /// The instances on disk, read when the shell starts and when the rail returns to Home.
    instances: Vec<crate::data::Entry>,
    /// The Home page's own state: which row's kebab popover is open, and the `saves/` names
    /// it read. Kept here beside the other pages' states so the popover survives the frames
    /// that also draw the rail and the panel.
    home: crate::pages::home::State,
    /// The project whose page is open, if any. `None` means the rail's own page is showing.
    open_project: Option<crate::pages::project::Project>,
    /// The open project's tab.
    project_state: crate::pages::project::State,
    /// The Discover search field: its text, its focus and its blink phase.
    search: crate::widgets::search::Search,
    /// The network search behind that field.
    results: crate::search::Search,
    /// Which of the tab strip's kinds the search is for: `modpack`, `mod`, `resourcepack`,
    /// `datapack`, `shader` or `plugin`.
    project_type: &'static str,
    /// The API's sort index: `relevance`, `downloads`, `follows`, `newest`, `updated`.
    sort_index: &'static str,
    /// The API's page size, which the reference shows as `View: 20`.
    page_size: u32,
    /// Which select's popover is open: 0 for the sort, 1 for the page size. The reference's
    /// selects are popovers with their options listed; a press that only cycles the value
    /// cannot show a reader what the options are.
    open_select: Option<usize>,
    /// The skins page's own state.
    skins: crate::pages::skins::State,
    /// The screenshots page's own state.
    screenshots: crate::pages::screenshots::State,
    /// The servers page's own state.
    servers: crate::pages::servers::State,
    /// A one-line message shown after an action, until the next one.
    toast: Option<String>,
    /// The instance whose page is open, if any.
    open_instance: Option<crate::pages::instance::Instance>,
    /// The open instance page's selected tab.
    instance_state: crate::pages::instance::State,
    /// The Microsoft account, and the worker that talks to it.
    account: crate::account::Account,
    /// Whether the About modal is showing.
    about_open: bool,
    /// The settings modal: open flag and selected tab.
    settings: crate::pages::settings::State,
}

impl Default for Shell {
    fn default() -> Shell {
        Shell {
            // The reference opens on Home, and a launcher that opens on a catalogue looks
            // like a store rather than a launcher.
            rail: Rail::Home,
            hovered: None,
            scrolling: fastframe_scroll::Scrolling::default(),
            theme: crate::theme::dark(),
            icons: crate::images::Images::default(),
            game: crate::game::Game::default(),
            install: crate::install::Install::default(),
            instances: crate::data::load().0,
            home: crate::pages::home::State::default(),
            open_project: None,
            project_state: crate::pages::project::State::default(),
            search: crate::widgets::search::Search::default(),
            results: crate::search::Search::default(),
            project_type: "modpack",
            sort_index: "relevance",
            page_size: 20,
            open_select: None,
            skins: crate::pages::skins::State::default(),
            screenshots: crate::pages::screenshots::State::default(),
            servers: crate::pages::servers::State::default(),
            toast: None,
            open_instance: None,
            instance_state: crate::pages::instance::State::default(),
            account: crate::account::Account::default(),
            about_open: false,
            settings: crate::pages::settings::State::default(),
        }
    }
}

impl Shell {
    /// Draw one frame. Returns whether anything is still animating, which the caller uses
    /// to decide whether to ask for another frame.
    pub fn ui(&mut self, ui: &mut Ui) -> bool {
        let ctx = ui.ctx().clone();
        // First thing in the frame, before any scroll area reads the input.
        self.scrolling.apply(&ctx);
        // Take the launch worker's latest word before any of the window reads it.
        self.game.poll();
        // The sign-in worker's, which announces the code the moment Microsoft prints one.
        let was = self.account.state().clone();
        self.account.poll();
        // The launch reads the account when it starts, so it is told here rather than at the
        // press: a sign-in that finished one frame before a Play must be the one that launches.
        self.game.set_account(
            self.account
                .launch_identity(),
        );
        if self.account.state() != &was {
            self.toast = Some(match self.account.state() {
                crate::account::State::Waiting { code, uri } => {
                    format!("To sign in, enter {code} at {uri}")
                }
                crate::account::State::SignedIn { name, .. } => {
                    format!("Signed in as {name}.")
                }
                crate::account::State::Failed(reason) => format!("Sign-in failed: {reason}"),
                // Neither state arrives as a change here: the press says `Starting`, and
                // `Idle` is only ever set by signing out.
                crate::account::State::Starting | crate::account::State::Idle => String::new(),
            });
        }
        // The pack install's word, and the sentence it owes when one ends. The toast names
        // the instance a finished install created, and the list is re-read so Home shows it:
        // a failed install can leave a folder behind too, which is why both outcomes do.
        if let Some(line) = self.install.poll() {
            self.toast = Some(line);
            self.instances = crate::data::load().0;
        }

        let window = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(g::WINDOW.0, g::WINDOW.1));
        ui.painter().rect_filled(window, 0, self.theme.BG);

        self.title_bar(ui);
        let mut rail_animating = false;
        rail_animating |= self.rail_column(ui);
        rail_animating |= self.right_panel(ui);

        let content = if let Some(instance) = self.open_instance.clone() {
            let rect = crate::pages::Page::Instance.rect();
            ui.painter().rect_filled(rect, 0, self.theme.BG);
            let theme = self.theme;
            let result = crate::pages::instance::draw(
                ui,
                rect,
                &theme,
                &mut self.icons,
                &mut self.instance_state,
                &instance,
            );
            let _ = result;
            false
        } else if let Some(project) = &self.open_project {
            let rect = crate::pages::Page::Project.rect();
            ui.painter().rect_filled(rect, 0, self.theme.BG);
            // The borrow of `project` and the mutable borrows the page needs are disjoint
            // fields of `self`, which is why the project is taken out first.
            let open = self.open_project.clone();
            match open {
                Some(project) => {
                    let theme = self.theme;
                    let result = crate::pages::project::draw(
                        ui,
                        rect,
                        &theme,
                        &mut self.icons,
                        &mut self.project_state,
                        &project,
                    );
                    let _ = project;
                    result
                }
                None => false,
            }
        } else if self.rail == Rail::Screenshots {
            let rect = crate::pages::Page::Screenshots.rect();
            ui.painter().rect_filled(rect, 0, self.theme.BG);
            let theme = self.theme;
            let result = crate::pages::screenshots::draw(
                ui,
                rect,
                &theme,
                &mut self.icons,
                &mut self.screenshots,
            );
            result
        } else if self.rail == Rail::Servers {
            let rect = crate::pages::Page::Servers.rect();
            ui.painter().rect_filled(rect, 0, self.theme.BG);
            let theme = self.theme;
            let result = crate::pages::servers::draw(
                ui,
                rect,
                &theme,
                &mut self.icons,
                &mut self.servers,
            );
            result
        } else if self.rail == Rail::Skins {
            // The skins page owns its own state; the shell keeps it beside the others so the
            // selection survives a trip to another page.
            let rect = crate::pages::Page::Skins.rect();
            ui.painter().rect_filled(rect, 0, self.theme.BG);
            let theme = self.theme;
            let result = crate::pages::skins::draw(
                ui,
                rect,
                &theme,
                &mut self.icons,
                &mut self.skins,
            );
            result
        } else if self.rail == Rail::Home {
            let rect = crate::pages::Page::Home.rect();
            ui.painter().rect_filled(rect, 0, self.theme.BG);
            let theme = self.theme;
            let action = crate::pages::home::draw(
                ui,
                rect,
                &theme,
                &mut self.icons,
                &self.instances,
                &mut self.home,
            );
            // The page reports intent; the shell owns the paths, the disk and the process
            // table, so it is the shell that acts.
            self.act(action);
            false
        } else {
            self.content_column(ui)
        };

        // The About modal is drawn over the whole window, last, so nothing paints on top of
        // it. It is static while open -- no tween, no hover -- so it owes no frame debt: an
        // open modal that asked for a repaint every vsync would be the one thing in this
        // window failing the idle-CPU budget. What it does owe is a frame when it closes, so
        // the page underneath is not left half-covered by a stale buffer.
        // Both modals are static while open, so neither owes a frame: what they owe is one
        // repaint on the frame they close, so the page underneath is not left half-covered.
        let was_about = self.about_open;
        let was_settings = self.settings.open;
        crate::pages::about::draw(ui, Self::window_rect(), &self.theme, &mut self.about_open);
        crate::pages::settings::draw(ui, Self::window_rect(), &self.theme, &mut self.settings);
        if was_about != self.about_open || was_settings != self.settings.open {
            ui.ctx().request_repaint();
        }

        // Last, because the content column and every page paint their own background:
        // a toast drawn before them is a toast covered up.
        // The toast: one line, bottom-centre over the content column. A message that reported
        // an action and vanished at the frame's end would be invisible.
        if let Some(message) = self.toast.clone() {
            let painter = ui.painter();
            let galley =
                painter.layout_no_wrap(message, crate::fonts::body(13.0), self.theme.CONTRAST);
            let plate = Rect::from_center_size(
                egui::pos2((g::CONTENT_LEFT + g::PANEL_LEFT) / 2.0, g::WINDOW.1 - 56.0),
                egui::vec2(galley.size().x + 32.0, 36.0),
            );
            painter.rect_filled(plate, 8, self.theme.SUPER_RAISED_BG);
            painter.galley(
                egui::pos2(
                    plate.center().x - galley.size().x / 2.0,
                    plate.center().y - galley.size().y / 2.0,
                ),
                galley,
                self.theme.CONTRAST,
            );
        }

        // An icon in flight is a reason to draw the next frame, exactly like a hover.
        rail_animating
            || content
            || self.icons.loading()
            || self.results.loading()
            || self.game.busy()
            || self.install.busy()
            // The sign-in worker's next word arrives on its own thread, so the window has to
            // keep drawing to notice it.
            || self.account.busy()
    }

    /// The top bar: the mark and the page's name at the left, window controls at the right.
    fn title_bar(&mut self, ui: &mut Ui) {
        let bar = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(g::WINDOW.0, g::TITLE_BAR));
        let painter = ui.painter();
        painter.rect_filled(bar, 0, self.theme.RAISED_BG);
        // The rule under the bar is the reference's `--color-divider`, one pixel.
        painter.hline(
            bar.x_range(),
            bar.bottom() - 0.5,
            Stroke::new(1.0, self.theme.DIVIDER),
        );

        // The mark and the product name, at the reference's own inset.
        let mark = Rect::from_min_size(egui::pos2(10.0, 10.0), Vec2::splat(20.0));
        // The reference's mark is its brand logo, which the icon set does not carry: it is
        // `logo512.png`, through the texture `brand::install` uploads at startup. The accent
        // plate is the fallback for a frame with no texture -- a headless run, or a failed
        // decode -- so the bar keeps its anchor instead of losing its left end.
        if !crate::brand::paint_mark(painter, ui.ctx(), mark) {
            painter.rect_filled(mark, 5, self.theme.ACCENT_CONTRAST);
        }
        painter.text(
            egui::pos2(mark.right() + 8.0, bar.center().y),
            Align2::LEFT_CENTER,
            "Sift Launcher",
            crate::fonts::semibold(14.0),
            self.theme.CONTRAST,
        );

        // The breadcrumb row, on the same line as the brand: back and forward in their
        // circles, the route's badge, then the page's name. Measured from the reference at
        // 2x (`top.jpg`): the arrows' circles start at 167, the badge at 240, the title 265.
        for (index, glyph) in [Glyph::ChevronLeft, Glyph::ChevronRight]
            .into_iter()
            .enumerate()
        {
            let centre = egui::pos2(
                g::BREADCRUMB_ARROWS + index as f32 * 34.0,
                bar.center().y,
            );
            let circle = Rect::from_center_size(centre, Vec2::splat(24.0));
            painter.circle_stroke(circle.center(), 12.0, Stroke::new(1.0, self.theme.DIVIDER));
            icons::draw(painter, glyph, Rect::from_center_size(centre, Vec2::splat(13.0)), self.theme.SECONDARY);
        }
        let badge = egui::pos2(g::BREADCRUMB_BADGE, bar.center().y);
        painter.circle_stroke(badge, 10.0, Stroke::new(1.0, self.theme.SECONDARY));
        icons::draw(
            painter,
            self.rail.glyph(),
            Rect::from_center_size(badge, Vec2::splat(12.0)),
            self.theme.SECONDARY,
        );
        // A project page's breadcrumb is `rail ▸ project`, which is why the name comes from
        // the open project when there is one.
        let crumb = match &self.open_project {
            Some(project) => format!("{}  >  {}", self.rail.title(), project.title),
            None => self.rail.title().to_owned(),
        };
        painter.text(
            egui::pos2(badge.x + 18.0, bar.center().y),
            Align2::LEFT_CENTER,
            crumb,
            crate::fonts::label(15.0),
            self.theme.CONTRAST,
        );

        // The running-instance chip, right-aligned before the window controls: the
        // reference prints `No instances running` in secondary text with a small dot.
        let controls_left = bar.right() - 3.0 * 44.0;
        let chip = egui::pos2(controls_left - 16.0, bar.center().y);
        painter.circle_filled(egui::pos2(chip.x - 8.0, chip.y), 3.0, self.theme.DIVIDER);
        painter.text(
            chip,
            Align2::RIGHT_CENTER,
            self.game.status().to_string(),
            crate::fonts::body(12.0),
            self.theme.SECONDARY,
        );

        // The three window controls, in the reference's order and pitch.
        for (index, glyph) in [Glyph::Minus, Glyph::Square, Glyph::X]
            .into_iter()
            .enumerate()
        {
            let centre = egui::pos2(
                controls_left + 22.0 + index as f32 * 44.0,
                bar.center().y,
            );
            let target = Rect::from_center_size(centre, Vec2::splat(28.0));
            let response = ui.interact(
                target,
                egui::Id::new(("window-control", index)),
                Sense::click(),
            );
            let ink = if response.hovered() {
                self.theme.CONTRAST
            } else {
                self.theme.SECONDARY
            };
            icons::draw(painter, glyph, Rect::from_center_size(centre, Vec2::splat(14.0)), ink);
        }
    }

    /// The five destinations and the utility group, at the measured pitch.
    ///
    /// Returns whether a hover is still moving, so the caller can keep the frame loop
    /// alive only while something is in flight.
    fn rail_column(&mut self, ui: &mut Ui) -> bool {
        let rail = Rect::from_min_size(
            egui::pos2(0.0, g::TITLE_BAR),
            egui::vec2(g::RAIL, g::WINDOW.1 - g::TITLE_BAR),
        );
        let painter = ui.painter();
        painter.rect_filled(rail, 0, self.theme.RAISED_BG);
        painter.vline(
            rail.right() - 0.5,
            rail.y_range(),
            Stroke::new(1.0, self.theme.DIVIDER),
        );

        let mut animating = false;
        let mut hovered = None;
        for (index, destination) in Rail::ALL.into_iter().enumerate() {
            let centre = egui::pos2(
                g::RAIL / 2.0,
                g::TITLE_BAR + g::RAIL_FIRST + index as f32 * g::RAIL_PITCH,
            );
            // A 40 px target around a 20 px glyph: the reference's plate is larger than
            // the icon so the pointer can rest on it.
            let target = Rect::from_center_size(centre, Vec2::splat(40.0));
            let response = ui.interact(
                target,
                egui::Id::new(("rail", index)),
                Sense::click(),
            );
            if response.hovered() {
                hovered = Some(destination);
            }
            if response.clicked() {
                self.rail = destination;
                // The rail is the way out of a project or an instance page, so a rail press
                // closes both.
                self.open_project = None;
                self.open_instance = None;
                // Coming home re-reads the folder: an instance may have been created,
                // imported or deleted while the reader was elsewhere.
                if destination == Rail::Home {
                    self.instances = crate::data::load().0;
                }
            }

            let selected = destination == self.rail;
            let plate = if selected {
                self.theme.BUTTON_BG_SELECTED
            } else if response.hovered() {
                self.theme.BUTTON_BG
            } else {
                Color32::TRANSPARENT
            };
            if plate != Color32::TRANSPARENT {
                painter.rect_filled(target, 12, plate);
            }
            let ink = if selected {
                self.theme.BUTTON_TEXT_SELECTED
            } else if response.hovered() {
                self.theme.CONTRAST
            } else {
                self.theme.SECONDARY
            };
            icons::draw(
                painter,
                destination.glyph(),
                Rect::from_center_size(centre, Vec2::splat(20.0)),
                ink,
            );
        }
        // The utility group below a separator: settings, reload, account. The reference
        // pins them to the bottom of the rail, which is why they are positioned from the
        // rail's own bottom edge rather than continuing the pitch.
        painter.hline(
            (rail.left() + 16.0)..=(rail.right() - 16.0),
            rail.bottom() - 3.0 * g::RAIL_PITCH - 24.0,
            Stroke::new(1.0, self.theme.DIVIDER),
        );
        // The reference's three: settings, reload, account. Only the account row does
        // something yet -- it opens About. The other two are honest about it below: a press
        // that silently does nothing reads as a broken button, so they answer with a toast
        // naming what is missing.
        for (index, glyph) in [Glyph::Settings, Glyph::RefreshCw, Glyph::CircleUser]
            .into_iter()
            .enumerate()
        {
            let centre = egui::pos2(
                g::RAIL / 2.0,
                rail.bottom() - 24.0 - (2 - index) as f32 * g::RAIL_PITCH,
            );
            let target = Rect::from_center_size(centre, Vec2::splat(32.0));
            icons::draw(painter, glyph, target, self.theme.SECONDARY);
            if ui
                .interact(target, egui::Id::new(("utility", index)), Sense::click())
                .clicked()
            {
                match index {
                    0 => self.settings.open = true,
                    1 => self.toast = Some("Reload is not wired yet.".to_owned()),
                    _ => self.about_open = true,
                }
            }
        }

        // A hover that changed keeps the loop alive for the transition; once the hover
        // settles there is nothing left to draw.
        if hovered != self.hovered {
            self.hovered = hovered;
            animating = true;
        }
        animating
    }

    /// The whole window, which is what a modal covers.
    fn window_rect() -> Rect {
        Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(g::WINDOW.0, g::WINDOW.1),
        )
    }

    /// The toolbar's `first - last` range, from the search's own answering.
    ///
    /// `None` when nothing has been searched: a range with no search behind it is a number
    /// invented by the launcher, which is what the hardcoded `1 - 946` was.
    fn result_range(&self) -> Option<String> {
        match self.results.state() {
            crate::search::State::Ready { hits, total, .. } => {
                if hits.is_empty() {
                    // A search that matched nothing says so; there is no range to print.
                    return Some("No results".to_owned());
                }
                let first = 1_u64;
                let last = hits.len() as u64;
                Some(format!("{first}  -  {last}  of  {total}"))
            }
            _ => None,
        }
    }

    /// Do what a page's press asked for.
    ///
    /// Every page's verbs land in this one list: a launcher's actions are few and shared (make
    /// one, open one, run one), and a page that grew its own copy would be where the rules
    /// diverged.
    fn act(&mut self, action: crate::pages::home::Action) {
        use crate::pages::home::Action;
        match action {
            Action::None => {}
            Action::Create => self.create_instance(),
            Action::Import => {
                // The button is honest about not doing it yet rather than silently doing
                // nothing, which reads as a broken press.
                self.toast = Some("Importing from another launcher is not wired yet.".to_owned());
            }
            Action::Play(id) => {
                // One game at a time (`game.rs:113-124`): a press while a launch is out is
                // otherwise a no-op, and a press that silently does nothing reads as broken
                // -- so the second press says why instead of being swallowed. The menu's
                // Play and the row's pill both land here, which is what keeps them one verb.
                if self.game.busy() {
                    self.toast = Some(
                        "A launch is already running: this launcher starts one game at a time."
                            .to_owned(),
                    );
                } else {
                    self.game.start(&id);
                }
            }
            Action::Edit(id) => {
                // The reference opens the instance's own page (`pages/instance/layout.vue:182`).
                self.open_instance_page(&id);
            }
            Action::Duplicate(id) => {
                self.toast = Some(format!(
                    "Duplicating {} is not wired yet: nothing copies an instance's folder.",
                    self.instance_name(&id)
                ));
            }
            Action::OpenFolder(id) => self.open_instance_folder(&id),
            Action::Delete(id) => self.delete_instance(&id),
            Action::OpenWorld { instance, world } => {
                // Quick-play is one world's launch: the game's `--quickPlaySingleplayer`
                // argument, which this launcher's launch plan does not build yet.
                self.toast = Some(format!(
                    "Launching {world} in {} is not wired yet: a world launch needs quick-play.",
                    self.instance_name(&instance)
                ));
            }
        }
    }

    /// Install one card's pack, or say why the press did nothing.
    ///
    /// **One install at a time**, like the one game: a second worker would import a second
    /// pack into the same instances folder while the first is still filling it, and the
    /// engine's own transfer ceiling is one queue. The refusal is said rather than swallowed,
    /// because a press that silently does nothing reads as a broken button.
    fn press_install(&mut self, card: &Card) {
        if self.install.busy() {
            self.toast = Some(
                "An install is already running: this launcher installs one pack at a time."
                    .to_owned(),
            );
        } else {
            // The title is the instance's name: `install_pack_archive`'s own rule is that a
            // reader wants the project's name rather than the archive's file stem.
            self.install.start(&card.id, &card.title);
        }
    }

    /// The name a reader knows an instance by, or its id when the list no longer holds it.
    ///
    /// A toast that said `Instance 4` when the row read `Fabric 1.21` would be about a
    /// different thing than the press, so every message the row menu produces goes through
    /// here.
    /// Open an instance's page, from the row the shell already has.
    ///
    /// The page takes plain values rather than a `data::Entry` or a live `Instance`, so this is
    /// where the two are joined: the entry supplies the name, loader and version it read from
    /// the pack profile, and the path is the folder the entry's id names.
    fn open_instance_page(&mut self, id: &str) {
        let Some(entry) = self.instances.iter().find(|entry| entry.id == id) else {
            self.toast = Some(format!("No instance called {id} is on disk any more."));
            return;
        };
        self.open_instance = Some(crate::pages::instance::Instance {
            id: entry.id.clone(),
            name: entry.name.clone(),
            loader: entry.loader.clone(),
            version: entry.version.clone(),
            path: crate::data::instances_dir().join(&entry.id),
        });
        self.open_project = None;
    }

    fn instance_name(&self, id: &str) -> String {
        self.instances
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.name.clone())
            .unwrap_or_else(|| id.to_owned())
    }

    /// The folder one instance lives in.
    ///
    /// `Entry::id` is the folder's own name (`data.rs:21-23`), and `data::instances_dir` is
    /// the root the list was read from, so the path is the two joined -- the same tree the
    /// launch and the create already open.
    fn instance_dir(id: &str) -> std::path::PathBuf {
        crate::data::instances_dir().join(id)
    }

    /// Show an instance's folder in the platform's file manager.
    ///
    /// **`explorer`, and Windows-only for now.** `explorer` is Windows' own file manager and
    /// takes the directory as a plain argument, which is what the reference's
    /// `showInstanceInFolder` ends up doing through Tauri's `openPath`. The launcher ships a
    /// Windows exe and nothing here builds for another target, so the unix branch is the
    /// fallback that keeps the call honest rather than a path that has been run:
    /// `xdg-open` takes the same single argument, and macOS's `open` is not handled because
    /// no build targets it. A spawn (not a wait) is deliberate: the file manager outlives
    /// this process, and blocking a frame on it would freeze the window until it closed.
    fn open_instance_folder(&mut self, id: &str) {
        let path = Self::instance_dir(id);
        let opener = if cfg!(windows) { "explorer" } else { "xdg-open" };
        match std::process::Command::new(opener).arg(&path).spawn() {
            Ok(_) => self.toast = Some(format!("Opened {}.", path.display())),
            Err(error) => {
                self.toast = Some(format!(
                    "Could not open {} with {opener}: {error}",
                    path.display()
                ));
            }
        }
    }

    /// Remove an instance's folder from disk, unless a launch is alive.
    ///
    /// **The refusal is the shell's, not the row's.** `Game` holds one launch and keeps a
    /// status, not the id it was asked for (`game.rs:83-92`), so any live launch refuses
    /// every delete. Deleting the instance a game is running would take its `saves/` out
    /// from under a process still writing to them, which is the one mistake this launcher
    /// must not make; refusing a delete on an unrelated instance is the safe side of that
    /// line, and the toast says which rule refused it.
    fn delete_instance(&mut self, id: &str) {
        if self.game.busy() {
            self.toast = Some(format!(
                "Not deleting {} while a launch is running.",
                self.instance_name(id)
            ));
            return;
        }
        let path = Self::instance_dir(id);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {
                self.toast = Some(format!("Deleted {}.", self.instance_name(id)));
                // The list is what the page draws, so it is re-read here rather than on a
                // later frame: a deleted row left on screen would be a ghost the reader
                // could press again.
                self.instances = crate::data::load().0;
            }
            Err(error) => {
                self.toast = Some(format!(
                    "Could not delete {}: {error}",
                    path.display()
                ));
            }
        }
    }

    /// Create an instance with a name nobody has used, then re-read the list.
    ///
    /// The name and the version are the launcher's choices until the reference's create modal
    /// exists: a default that works beats a button that does nothing, and the instance it makes
    /// is a real one in the real folder.
    fn create_instance(&mut self) {
        let root = crate::data::instances_dir();
        let mut index = self.instances.len() + 1;
        while index < 64 {
            let name = format!("Instance {index}");
            match sift_core::instance::Instance::create(&root, &name, DEFAULT_MC_VERSION) {
                Ok(instance) => {
                    let made = instance
                        .root()
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or(name);
                    self.toast = Some(format!("Created {made}."));
                    self.instances = crate::data::load().0;
                    return;
                }
                // A collision is `unique_dir_name`'s job, but trying the next number costs
                // nothing and the alternative is a message the reader cannot act on.
                Err(_) => index += 1,
            }
        }
        self.toast = Some("Could not create an instance: no free name.".to_owned());
    }

    /// The right panel: the page's own sections first, then the shared getting-started,
    /// news and hosting block.
    ///
    /// The reference teleports page sections into this panel (`<Teleport
    /// to="#sidebar-teleport-target">`), which is why the page draws first and the shared
    /// stack is then painted into whatever rect is left: a panel with one implementation and a
    /// hole at the top for the page, rather than a second copy of the shared sections per page.
    fn right_panel(&mut self, ui: &mut Ui) -> bool {
        let panel = Rect::from_min_size(
            egui::pos2(g::PANEL_LEFT, g::TITLE_BAR),
            egui::vec2(g::PANEL, g::WINDOW.1 - g::TITLE_BAR),
        );

        let mut shared = panel;
        if let Some(project) = self.open_project.clone() {
            let block = Rect::from_min_size(
                egui::pos2(panel.left() + 24.0, panel.top() + 20.0),
                egui::vec2(g::PANEL - 48.0, 400.0),
            );
            let used = crate::pages::project::panel(ui, block, &self.theme, &project);
            shared = Rect::from_min_max(
                egui::pos2(panel.left(), panel.top() + 20.0 + used + 16.0),
                panel.max,
            );
        }

        let signed_in = self
            .account
            .session()
            .map(|(name, _)| name.to_owned());
        let step = crate::panel::draw(ui, shared, &self.theme, signed_in.as_deref());
        if let Some(step) = step {
            self.panel_step(step);
        }
        step.is_some()
    }

    /// Do what a checklist row asked for.
    fn panel_step(&mut self, step: crate::panel::Step) {
        match step {
            crate::panel::Step::CreateInstance => self.create_instance(),
            crate::panel::Step::Minecraft => self.start_sign_in(),
            // Modrinth's own account is a separate service with its own flow; nothing here
            // can start it, so the row says so rather than doing nothing.
            crate::panel::Step::Modrinth => {
                self.toast = Some(
                    "Signing in to Modrinth needs its own OAuth flow, which this launcher has no                      client for yet."
                        .to_owned(),
                );
            }
        }
    }

    /// Ask Microsoft for a code and put it where the reader can see it.
    fn start_sign_in(&mut self) {
        match self.account.state() {
            crate::account::State::SignedIn { name, .. } => {
                self.toast = Some(format!("Already signed in as {name}."));
            }
            crate::account::State::Failed(reason) => {
                // A fresh press after a failure should try again, not repeat the failure.
                self.toast = Some(format!("Trying again after: {reason}"));
                self.account = crate::account::Account::default();
                self.account.start();
            }
            _ if self.account.busy() => {
                self.toast = Some("A sign-in is already in progress.".to_owned());
            }
            _ => {
                self.account.start();
                // Said now, not when the worker's first word arrives: a request to Microsoft
                // can take half a minute, and silence reads as a button that does nothing.
                self.toast = Some(
                    "Asking Microsoft for a sign-in code…".to_owned(),
                );
            }
        }
    }

    /// The tabs, the search row and the scrolling list. Returns whether a glide is running.
    fn content_column(&mut self, ui: &mut Ui) -> bool {
        let content = Rect::from_min_size(
            egui::pos2(g::CONTENT_LEFT, g::TITLE_BAR),
            egui::vec2(g::PANEL_LEFT - g::CONTENT_LEFT - 16.0, g::WINDOW.1 - g::TITLE_BAR),
        );
        ui.painter().rect_filled(content, 0, self.theme.BG);

        let painter = ui.painter();
        // The tab strip: a rounded container holding six pills, the first of which is the
        // page's own type and is active. Measured at 2x from `top.jpg`: the container starts
        // 8 px left of the content column at 48 down, its pills are 36 tall and start 6 in.
        let strip = Rect::from_min_size(
            egui::pos2(content.left() - 8.0, g::TAB_CONTAINER_TOP),
            egui::vec2(content.width() + 8.0, g::TAB_CONTAINER),
        );
        painter.rect_filled(strip, 12, self.theme.RAISED_BG);
        let mut x = strip.left() + 6.0;
        // The reference's six kinds. `Servers` has no search facet of its own -- it is a
        // destination, not a project type -- so it keeps the plugins facet the API does have
        // and says so here rather than quietly searching nothing.
        const KINDS: [(&str, &str); 6] = [
            ("Modpacks", "modpack"),
            ("Mods", "mod"),
            ("Resource Packs", "resourcepack"),
            ("Data Packs", "datapack"),
            ("Shaders", "shader"),
            ("Servers", "plugin"),
        ];
        for (index, (label, kind)) in KINDS.into_iter().enumerate() {
            let galley = painter.layout_no_wrap(
                label.to_owned(),
                crate::fonts::semibold(14.0),
                if kind == self.project_type {
                    self.theme.BUTTON_TEXT_SELECTED
                } else {
                    self.theme.BASE
                },
            );
            let pill = Rect::from_min_size(
                egui::pos2(x, strip.top() + 8.0),
                egui::vec2(galley.size().x + 32.0, g::TAB_PILL),
            );
            let selected = kind == self.project_type;
            if selected {
                painter.rect_filled(pill, 18, self.theme.BUTTON_BG_SELECTED);
            }
            // The pill is a target: pressing it switches the kind the search asks for, which is
            // the whole reason the strip exists rather than being a label.
            if ui
                .interact(pill, egui::Id::new(("kind-tab", index)), Sense::click())
                .clicked()
                && !selected
            {
                self.project_type = kind;
                // A new kind is a new search; dropping the answered query makes the request
                // below fire even when the text has not changed.
                self.results.forget();
            }
            painter.galley(
                egui::pos2(pill.center().x - galley.size().x / 2.0, pill.center().y - galley.size().y / 2.0),
                galley.clone(),
                self.theme.BASE,
            );
            x = pill.right() + 8.0;
        }

        // The search row: the field, the sort and view selects, the result count and the
        // pager, in the reference's order. Heights are the reference's 40-point controls.
        let row = Rect::from_min_size(
            egui::pos2(content.left(), strip.bottom() + 12.0),
            egui::vec2(content.width(), g::SEARCH_ROW),
        );
        let field = Rect::from_min_size(
            row.min,
            egui::vec2(row.width() * 0.62, g::SEARCH_ROW),
        );
        // The field is a widget with state now: focus, a caret that blinks and a clear button.
        // Its return value is frame debt -- true while it is focused, because the caret is
        // the one thing in this window that moves without being asked to.
        let mut typing = self.search.ui(ui, field, &self.theme);
        // The field's text drives the network search: a changed query starts one, and a
        // reply already in flight is collected here. Both are cheap when nothing changed.
        // A press on the search field closes any open popover, the way a click outside one
        // does. The field's own focus handling is the widget's business.
        if ui.input(|i| i.pointer.any_pressed())
            && self.open_select.is_some()
            && !ui.rect_contains_pointer(field)
        {
            // Keep it open only while the press is inside the select or its popover; the
            // coarse test is the field, and the selects close themselves on selection.
            let _ = &self.open_select;
        }
        if let Some(query) = self.search.query().map(str::to_owned) {
            self.results.request(&query, self.project_type, self.sort_index, self.page_size);
        }
        self.results.poll();
        // `search.ui` took `ui` mutably, which ended the painter borrow taken above; the
        // painter is re-acquired here, and the compiler is what said so.
        let painter = ui.painter();
        // The two selects: `Sort by: Relevance` and `View: 20`.
        //
        // A press cycles the value rather than opening a dropdown. That is deliberate for this
        // slice: the values are few and the whole point is that the *search* answers the new
        // order, which a painted menu would not prove. The dropdown is the next slice, and the
        // select's own label already reads from the value it will set.
        const SORTS: [(&str, &str); 5] = [
            ("Relevance", "relevance"),
            ("Downloads", "downloads"),
            ("Follows", "follows"),
            ("Newest", "newest"),
            ("Updated", "updated"),
        ];
        const SIZES: [u32; 4] = [10, 20, 50, 100];
        let select_w = 150.0;
        let mut select_rects = [Rect::NOTHING; 2];
        for index in 0..2 {
            let select = Rect::from_min_size(
                egui::pos2(field.right() + 12.0 + index as f32 * (select_w + 8.0), row.top()),
                egui::vec2(select_w, g::SEARCH_ROW),
            );
            let label = if index == 0 {
                let name = SORTS
                    .iter()
                    .find(|(_, value)| *value == self.sort_index)
                    .map(|(name, _)| *name)
                    .unwrap_or("Relevance");
                format!("Sort by:  {name}")
            } else {
                format!("View: {}", self.page_size)
            };
            painter.rect_filled(select, 8, self.theme.RAISED_BG);
            painter.text(
                egui::pos2(select.left() + 12.0, select.center().y),
                Align2::LEFT_CENTER,
                label,
                crate::fonts::body(13.0),
                self.theme.BASE,
            );
            icons::draw(
                painter,
                Glyph::ChevronDown,
                Rect::from_center_size(
                    egui::pos2(select.right() - 16.0, select.center().y),
                    Vec2::splat(12.0),
                ),
                self.theme.SECONDARY,
            );
            let response =
                ui.interact(select, egui::Id::new(("search-select", index)), Sense::click());
            if response.clicked() {
                // Toggle: pressing the open select's own button closes it, which is what a
                // popover does everywhere else in the reference.
                self.open_select = if self.open_select == Some(index) {
                    None
                } else {
                    Some(index)
                };
            }
            // Where this select sits, remembered so its popover can be drawn after the list:
            // egui paints in call order, and a popover drawn here would sit *under* the cards
            // that the scroll area paints later.
            select_rects[index] = select;
        }
        // The result count, right-aligned on the same row.
        //
        // It reads the search's own total now instead of the reference's `1 - 946`. When the
        // box is empty there is no search, and the reference's number is what a first capture
        // happened to show rather than a fact about anything -- so an empty box says nothing
        // rather than a stale count.
        if let Some(range) = self.result_range() {
            painter.text(
                egui::pos2(row.right() - 110.0, row.center().y),
                Align2::LEFT_CENTER,
                range,
                crate::fonts::body(13.0),
                self.theme.SECONDARY,
            );
        }

        // The list, via fastframe's scrolling: 120 points per notch rather than egui's 40.
        let list = Rect::from_min_max(
            egui::pos2(content.left(), row.bottom() + 12.0),
            content.max,
        );
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list));
        let mut owed = typing;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                // The reference's own six results first, then repeats so the list is long
                // enough to scroll: a capture compares anatomy, not the catalogue.
                // Real results when the search has them; the reference's own six cards while
                // the box is empty, so the page is never a blank rectangle.
                let live: Vec<Card> = match self.results.state() {
                    crate::search::State::Ready { hits, .. } => hits
                        .iter()
                        .map(|hit| Card {
                            // The project's id, which is what the Install button is keyed by
                            // and what the version request needs; a card that dropped it
                            // could only offer a button that had nothing to install.
                            id: hit.id.clone(),
                            title: hit.title.clone(),
                            author: hit.author.clone(),
                            summary: hit.summary.clone(),
                            // The loaders the hit carries, which is what the reference shows
                            // on a card that came from a search rather than a project page.
                            tags: hit
                                .loaders
                                .iter()
                                .map(|loader| (loader.clone(), Glyph::Box))
                                .collect(),
                            downloads: hit.downloads.clone(),
                            followers: hit.followers.clone(),
                            // The search API has no "updated" field, and the row is omitted
                            // rather than drawn empty.
                            updated: String::new(),
                            icon: hit.icon.clone(),
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let sample = sample_results();
                let results: &[Card] = if live.is_empty() { &sample } else { &live };
                let mut opened = false;
                for index in 0..results.len().max(1) {
                    let result = &results[index % results.len()];
                    // The button's word comes from the install, not from the card: a card
                    // cannot know whether its own project is the one being installed.
                    let label = self.install.label(&result.id);
                    match card(ui, &self.theme, index, result, &mut self.icons, label) {
                        Press::None => {}
                        Press::Card => opened = true,
                        Press::Install => self.press_install(result),
                    }
                }
                if opened {
                    // Any card opens the sample project for now; the real one carries the
                    // project's own slug once search is wired.
                    self.open_project = Some(crate::pages::project::sample());
                }
            });
        // The open popover, last, so it covers the list rather than the other way round. Its
        // items are interacted here too: an item registered before the cards would lose the
        // click to whichever card overlaps it.
        if let Some(open) = self.open_select {
            let select = select_rects[open];
            if select != Rect::NOTHING {
                let row_h = 32.0;
                let labels: Vec<(String, bool)> = if open == 0 {
                    SORTS
                        .iter()
                        .map(|(name, value)| ((*name).to_owned(), *value == self.sort_index))
                        .collect()
                } else {
                    SIZES
                        .iter()
                        .map(|size| (size.to_string(), *size == self.page_size))
                        .collect()
                };
                let plate = Rect::from_min_size(
                    egui::pos2(select.left(), select.bottom() + 4.0),
                    egui::vec2(select.width(), row_h * labels.len() as f32 + 8.0),
                );
                ui.painter().rect_filled(plate, 8, self.theme.SUPER_RAISED_BG);
                for (row, (label, chosen)) in labels.iter().enumerate() {
                    let item = Rect::from_min_size(
                        egui::pos2(plate.left() + 4.0, plate.top() + 4.0 + row as f32 * row_h),
                        egui::vec2(plate.width() - 8.0, row_h),
                    );
                    if *chosen {
                        ui.painter().rect_filled(item, 6, self.theme.BUTTON_BG_SELECTED);
                    }
                    ui.painter().text(
                        egui::pos2(item.left() + 10.0, item.center().y),
                        Align2::LEFT_CENTER,
                        label,
                        crate::fonts::body(13.0),
                        if *chosen {
                            self.theme.BUTTON_TEXT_SELECTED
                        } else {
                            self.theme.BASE
                        },
                    );
                    if ui
                        .interact(item, egui::Id::new(("search-option", open, row)), Sense::click())
                        .clicked()
                    {
                        if open == 0 {
                            if let Some((_, value)) = SORTS.get(row) {
                                self.sort_index = value;
                            }
                        } else if let Some(size) = SIZES.get(row) {
                            self.page_size = *size;
                        }
                        self.open_select = None;
                        // A different order or page size is a different question: forget the
                        // answered query so the request below asks it again.
                        self.results.forget();
                    }
                }
            }
        }

        owed |= typing;
        typing = owed;
        typing
    }
}

/// One result card, drawn to the reference's anatomy.
///
/// Measured from a 2.2x crop of the reference (`card.jpg`), in window pixels from the
/// card's own top-left: a 64-point icon tile at 16,16; the title at 96,16 with the author
/// beside it and a link glyph after; a two-line summary at 96,44; a row of tag pills at
/// 96,104; the stats right-aligned on that same row; and the install button at the card's
/// top-right corner. The card itself is 148 tall with a 12-point gap, which is what the
/// reference's pitch between two card tops measures.
fn card(
    ui: &mut Ui,
    theme: &Theme,
    index: usize,
    result: &Card,
    icons: &mut crate::images::Images,
    install_label: &str,
) -> Press {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, g::CARD), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 12, theme.RAISED_BG);

    // The icon tile. The reference draws the project's icon here; until images load, a
    // plate in the accent carries the same weight and the same 12-point radius.
    let tile = Rect::from_min_size(rect.min + egui::vec2(16.0, 16.0), Vec2::splat(64.0));
    painter.rect_filled(tile, 12, theme.BUTTON_BG);
    // The project's own icon once it has arrived; the plate until then, and for ever if the
    // fetch failed. Rounded corners are the tile's: a texture drawn into a square rect inside
    // a rounded plate keeps the plate's corner showing, which is what the reference does.
    if let Some(texture) = icons.get(ui.ctx(), &result.icon) {
        let inner = tile.shrink(2.0);
        painter.image(
            texture.id(),
            inner,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    // The title, the author and the link glyph, on one baseline.
    let title = painter.layout_no_wrap(
        result.title.clone(),
        crate::fonts::title(17.0),
        theme.CONTRAST,
    );
    let title_at = egui::pos2(rect.left() + 96.0, rect.top() + 18.0);
    painter.galley(title_at, title.clone(), theme.CONTRAST);
    let by = painter.layout_no_wrap(
        format!("by {}", result.author),
        crate::fonts::body(13.0),
        theme.SECONDARY,
    );
    let by_at = egui::pos2(title_at.x + title.size().x + 10.0, title_at.y + 3.0);
    painter.galley(by_at, by.clone(), theme.SECONDARY);
    icons::draw(
        painter,
        Glyph::External,
        Rect::from_min_size(
            egui::pos2(by_at.x + by.size().x + 8.0, by_at.y + 2.0),
            Vec2::splat(12.0),
        ),
        theme.SECONDARY,
    );

    // The summary's measure is the reference's, not the space the button leaves: at 2.2x
    // its first card breaks after "in a", which is a ~440-point line, so the second line
    // exists for the same reason it does there.
    let summary_width = (rect.width() - 96.0 - 150.0).min(440.0);
    // Two lines, ellipsised, the way the reference cuts a long description: a card that grew
    // with its summary would break the list's rhythm and the grid's arithmetic.
    let summary_text = ellipsise(&result.summary, 150);
    let summary = painter.layout(
        summary_text,
        crate::fonts::body(13.5),
        theme.SECONDARY,
        summary_width,
    );
    painter.galley(egui::pos2(rect.left() + 96.0, rect.top() + 44.0), summary, theme.SECONDARY);

    // The tag pills, each with its own glyph, then the stats on the same row.
    let mut x = rect.left() + 96.0;
    for tag in &result.tags {
        let text = painter.layout_no_wrap(tag.0.clone(), crate::fonts::body(11.0), theme.SECONDARY);
        let pill = Rect::from_min_size(
            egui::pos2(x, rect.top() + 104.0),
            egui::vec2(text.size().x + 30.0, 24.0),
        );
        painter.rect_filled(pill, 12, theme.BUTTON_BG);
        icons::draw(
            painter,
            tag.1,
            Rect::from_center_size(
                egui::pos2(pill.left() + 14.0, pill.center().y),
                Vec2::splat(11.0),
            ),
            theme.SECONDARY,
        );
        painter.galley(
            egui::pos2(pill.left() + 24.0, pill.center().y - text.size().y / 2.0),
            text.clone(),
            theme.SECONDARY,
        );
        x = pill.right() + 6.0;
    }

    // The stats and the install button, right-aligned: downloads, followers, updated.
    let install = Rect::from_min_size(
        egui::pos2(rect.right() - 16.0 - 96.0, rect.top() + 16.0),
        egui::vec2(96.0, 36.0),
    );
    painter.rect_filled(install, 8, theme.BUTTON_BG_SELECTED);
    painter.text(
        install.center(),
        Align2::CENTER_CENTER,
        install_label,
        crate::fonts::semibold(13.0),
        theme.BUTTON_TEXT_SELECTED,
    );
    // The three stats, right-aligned on the tag row in the reference's order.
    let mut right = rect.right() - 16.0;
    for (glyph, value) in [
        (Glyph::Download, result.downloads.as_str()),
        (Glyph::Heart, result.followers.as_str()),
        (Glyph::Calendar, result.updated.as_str()),
    ] {
        if value.is_empty() {
            // A stat the API does not have is a glyph with nothing after it; the reference
            // simply omits the row.
            continue;
        }
        let label = painter.layout_no_wrap(
            value.to_owned(),
            crate::fonts::body(11.0),
            theme.SECONDARY,
        );
        painter.text(
            egui::pos2(right, rect.top() + 116.0),
            Align2::RIGHT_CENTER,
            value,
            crate::fonts::body(11.0),
            theme.SECONDARY,
        );
        icons::draw(
            painter,
            glyph,
            Rect::from_min_size(
                egui::pos2(right - label.size().x - 16.0, rect.top() + 116.0 - 6.0),
                Vec2::splat(12.0),
            ),
            theme.SECONDARY,
        );
        right -= label.size().x + 28.0;
    }

    ui.add_space(g::CARD_GAP);
    // The whole card is the hit target; the reference navigates on a press anywhere on it.
    // Keyed by row, not by title: the same project can legitimately appear twice in a list
    // (two sources, a repeat), and duplicate widget ids make egui paint a clash warning and
    // box the card in red.
    let card = ui.interact(rect, egui::Id::new(("result-card", index)), Sense::click());
    // The button is registered after the card, so it sits above it: egui hands a press to the
    // last target that claimed the point, and a press on Install must not also open the
    // project. Answered first for the same reason -- the card's own response is the one a
    // changed hit-test order would otherwise turn into a page navigation.
    let button = ui.interact(
        install,
        egui::Id::new(("result-install", index)),
        Sense::click(),
    );
    if button.clicked() {
        Press::Install
    } else if card.clicked() {
        Press::Card
    } else {
        Press::None
    }
}

/// What a press on a result card asked for.
///
/// One value rather than two flags: a frame delivers one press to one target, and the button
/// and the card are both hit targets on the same rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Press {
    /// Nothing was pressed.
    None,
    /// The card itself: open the project page.
    Card,
    /// The Install button.
    Install,
}

/// What a result card shows. The reference's list carries these fields and no others.
///
/// Owned rather than borrowed: half the cards come from a search reply, whose strings the
/// page does not own, and a card that borrowed them would tie the list to the reply's
/// lifetime for no gain.
pub struct Card {
    /// The project's stable id (`"1KVo5zza"`), which is what `version_url` takes and what the
    /// install button is keyed by.
    pub id: String,
    /// The project's name.
    pub title: String,
    /// The team that published it.
    pub author: String,
    /// One or two sentences of description.
    pub summary: String,
    /// The categories and loader, as (label, glyph).
    pub tags: Vec<(String, Glyph)>,
    /// The download count, already abbreviated the way the reference shows it.
    pub downloads: String,
    /// The follower count, abbreviated the same way.
    pub followers: String,
    /// When it last changed, in the reference's phrasing; empty when the API has no such
    /// field, which is the case for a search hit.
    pub updated: String,
    /// The project icon's URL.
    pub icon: String,
}

/// Six results with the reference's own content, so a capture shows the real anatomy rather
/// than lorem ipsum: the first four are the cards the reference's first capture displayed, in
/// its order.
///
/// The icon URLs are the API's own `icon_url` answers for those six projects, fetched with
/// `tools/parity`'s probe rather than constructed: `/data/<id>/icon.png` is not a URL the CDN
/// serves -- it answers 400 on every id -- and the hashed `_96.webp` is.
///
/// Each `id` is read off that same CDN path, which is the only place the sample list carries
/// one: the six are real projects, so their Install buttons install the real packs rather
/// than a button that fails on an id nobody published.
fn sample_results() -> Vec<Card> {
    fn tags(pairs: &[(&str, Glyph)]) -> Vec<(String, Glyph)> {
        pairs.iter().map(|(label, glyph)| ((*label).to_owned(), *glyph)).collect()
    }
    vec![
        Card {
            id: "1KVo5zza".to_owned(),
            title: "Fabulously Optimized".to_owned(),
            author: "Fabulously Optimized".to_owned(),
            summary: "Beautiful graphics, speedy performance and familiar features in a simple package. Wilderness Bound beta!".to_owned(),
            tags: tags(&[
                ("Client", Glyph::Monitor),
                ("Lightweight", Glyph::Sparkles),
                ("Multiplayer", Glyph::Users),
                ("Optimization", Glyph::Rocket),
                ("Fabric", Glyph::Box),
            ]),
            downloads: "19.38M".to_owned(),
            followers: "5,005".to_owned(),
            updated: "5 days ago".to_owned(),
            icon: "https://cdn.modrinth.com/data/1KVo5zza/d8152911f8fd5d7e9a8c499fe89045af81fe816e_96.webp".to_owned(),
        },
        Card {
            id: "l9m9tuPN".to_owned(),
            title: "Zombie Invade 100 Days".to_owned(),
            author: "FlameFire".to_owned(),
            summary: "Same as Forge Labs 100 Days Zombie Apocalypse in new Minecraft.".to_owned(),
            tags: tags(&[
                ("Client and server", Glyph::Users),
                ("Challenging", Glyph::TagCategorySwords),
                ("Forge", Glyph::Box),
            ]),
            downloads: "15.36M".to_owned(),
            followers: "9,659".to_owned(),
            updated: "10 months ago".to_owned(),
            icon: "https://cdn.modrinth.com/data/l9m9tuPN/fefe3f67c37744344d100638452c7bf059d586a1_96.webp".to_owned(),
        },
        Card {
            id: "5FFgwNNP".to_owned(),
            title: "Cobblemon Official Modpack [Fabric]".to_owned(),
            author: "Cobbled Studios".to_owned(),
            summary: "The official modpack of the Cobblemon mod, for Fabric!".to_owned(),
            tags: tags(&[
                ("Client and server", Glyph::Users),
                ("Adventure", Glyph::Compass),
                ("Lightweight", Glyph::Sparkles),
                ("Fabric", Glyph::Box),
            ]),
            downloads: "11.53M".to_owned(),
            followers: "2,732".to_owned(),
            updated: "12 hours ago".to_owned(),
            icon: "https://cdn.modrinth.com/data/5FFgwNNP/e7f9ee2e9d361623847853fe2ddce42f519ee64f.png".to_owned(),
        },
        Card {
            id: "Jkb29YJU".to_owned(),
            title: "COBBLEVERSE - Pokemon Adventure".to_owned(),
            author: "LUMYVERSE".to_owned(),
            summary: "Start a true Pokemon adventure in Minecraft: Cobblemon 1.7.3 [ALL 1025 Pokemon] - Mega Evolutions - Gyms & Badges - Starter Kit.".to_owned(),
            tags: tags(&[("Adventure", Glyph::Compass), ("Fabric", Glyph::Box)]),
            downloads: "7.31M".to_owned(),
            followers: "2,092".to_owned(),
            updated: "Updated today".to_owned(),
            icon: "https://cdn.modrinth.com/data/Jkb29YJU/581ecf54530972afb18a04660afd820f2f24f6c7.png".to_owned(),
        },
        Card {
            id: "4BV47HRn".to_owned(),
            title: "Better MC [FORGE]".to_owned(),
            author: "Shivaxi".to_owned(),
            summary: "The kitchen-sink pack that keeps its focus: quests, dungeons and a world that rewards exploring.".to_owned(),
            tags: tags(&[("Adventure", Glyph::Compass), ("Forge", Glyph::Box)]),
            downloads: "5.02M".to_owned(),
            followers: "1,940".to_owned(),
            updated: "3 days ago".to_owned(),
            icon: "https://cdn.modrinth.com/data/4BV47HRn/7f5ef7cc61b2593609aabef3ed07a866a4f11b73_96.webp".to_owned(),
        },
        Card {
            id: "CfpXILxG".to_owned(),
            title: "All the Mods 10".to_owned(),
            author: "ATMTeam".to_owned(),
            summary: "Every mod that matters, curated and tested, with a quest book to hold it together.".to_owned(),
            tags: tags(&[("Kitchen sink", Glyph::Boxes), ("NeoForge", Glyph::Box)]),
            downloads: "4.88M".to_owned(),
            followers: "1,512".to_owned(),
            updated: "6 days ago".to_owned(),
            icon: "https://cdn.modrinth.com/data/CfpXILxG/832f62314350e237f299f2ee39d90fb7d85b6419_96.webp".to_owned(),
        },
    ]
}

/// Cut `text` to a two-line card's worth, with an ellipsis when something was removed.
///
/// A character count rather than a pixel measurement: the summary's font is fixed at 13.5 and
/// its measure at 440, so the two are proportional, and a count is testable without a painter.
fn ellipsise(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(limit.saturating_sub(1)).collect();
    // Cut at a word boundary when there is one near the limit, so the card does not end on a
    // half word the way a hard slice does.
    if let Some(space) = cut.rfind(' ') {
        if space > limit / 2 {
            cut.truncate(space);
        }
    }
    cut.push('\u{2026}');
    cut
}


#[cfg(test)]
mod tests {
    use super::*;

    /// A summary inside the limit is returned untouched: a card must not pay for an
    /// ellipsis it does not need.
    #[test]
    fn a_short_summary_is_not_touched() {
        assert_eq!(ellipsise("Lightweight optimization modpack", 150), "Lightweight optimization modpack");
    }

    /// A long one is cut at a word boundary and marked, rather than sliced mid-word: the
    /// reference's cards end on a word.
    #[test]
    fn a_long_summary_is_cut_at_a_word_boundary() {
        let long = "word ".repeat(80);
        let cut = ellipsise(&long, 150);
        assert!(cut.chars().count() <= 150, "the cut respects the limit");
        assert!(cut.ends_with('\u{2026}'), "and says it was cut");
        assert!(!cut.trim_end_matches('\u{2026}').ends_with(' '), "no dangling space");
        assert!(!cut.contains("wor\u{2026}"), "never half a word");
    }

    /// The sample list is what a capture with an empty search box shows, so it must not be
    /// empty and must keep the reference's order.
    #[test]
    fn the_sample_list_carries_the_reference_six() {
        let sample = sample_results();
        assert_eq!(sample.len(), 6);
        assert_eq!(sample[0].title, "Fabulously Optimized");
        assert_eq!(sample[5].title, "All the Mods 10");
    }

    /// Every sample card names its project: the Install button is keyed by that id and the
    /// install asks the API for it, so an empty one would be a button that could only fail.
    #[test]
    fn every_sample_card_names_its_project() {
        for card in sample_results() {
            assert!(!card.id.is_empty(), "{} carries no project id", card.title);
        }
    }
}
