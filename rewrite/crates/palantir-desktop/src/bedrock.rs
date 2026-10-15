use crate::design::{self, Button, label, muted, subtitle, title};
use crate::model::{Edition, Result};
use crate::{App, Dialog, Message, action, card};
use iced::widget::{Space, button, column, row};
use iced::{Element, Length};
use palantir_bedrock::{BedrockInstall, Channel};
use std::path::PathBuf;

pub const FOLDERS: [&str; 3] = ["minecraftWorlds", "resource_packs", "behavior_packs"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Overview,
    Worlds,
    Content,
    Access,
}
impl Tab {
    pub const ALL: [Self; 4] = [Self::Overview, Self::Worlds, Self::Content, Self::Access];
    pub fn name(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Worlds => "Worlds",
            Self::Content => "Content",
            Self::Access => "Accounts & access",
        }
    }
}
#[derive(Default)]
pub struct State {
    pub tab: Tab,
    pub query: String,
    pub installs: Vec<BedrockInstall>,
    pub loading: bool,
    pub checked: bool,
    pub error: Option<String>,
    pub selected: Option<Channel>,
    pub reading: bool,
    pub entries: Vec<Entry>,
    pub worlds: Vec<palantir_bedrock::worlds::WorldSummary>,
    pub files_error: Option<String>,
}
impl State {
    fn matching_worlds(&self) -> Vec<&palantir_bedrock::worlds::WorldSummary> {
        let query = self.query.to_lowercase();
        self.worlds
            .iter()
            .filter(|world| {
                format!("{} {}", world.name, world.folder)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect()
    }
}
#[derive(Debug)]
pub struct Entry {
    pub name: String,
    pub folder: String,
}

/// Only existing, detected folders can be opened. Unlike Java's folder action,
/// this must not create a guessed player-data layout for Microsoft's game.
pub fn folder_path(install: &BedrockInstall, folder: &str) -> Result<PathBuf> {
    if !folder.is_empty() && !FOLDERS.contains(&folder) {
        return Err("Unknown Bedrock data folder.".into());
    }
    let root = install.data_path.as_ref().ok_or_else(|| {
        "No player-data folder detected. Open the game once, then refresh. Preview data is not yet located by this launcher.".to_string()
    })?;
    for path in [root.clone(), root.join(folder)] {
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("Refusing a linked or non-directory Bedrock data folder.".into());
        }
    }
    Ok(root.join(folder))
}

#[derive(Debug)]
pub struct Inventory {
    pub entries: Vec<Entry>,
    pub worlds: Vec<palantir_bedrock::worlds::WorldSummary>,
}
pub fn inventory(install: &BedrockInstall) -> Result<Inventory> {
    folder_path(install, "")?;
    let mut entries = Vec::new();
    for folder in FOLDERS {
        let root = install
            .data_path
            .as_ref()
            .ok_or("No player-data folder detected.")?;
        if !root.join(folder).try_exists().map_err(|e| e.to_string())? {
            continue;
        }
        for item in std::fs::read_dir(folder_path(install, folder)?).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            if item.file_type().map_err(|e| e.to_string())?.is_dir() {
                entries.push(Entry {
                    name: item.file_name().to_string_lossy().into_owned(),
                    folder: folder.into(),
                });
            }
        }
    }
    entries.sort_by_key(|entry| (entry.folder.clone(), entry.name.to_lowercase()));
    let root = folder_path(install, "")?;
    let worlds = palantir_bedrock::worlds::list_worlds(&root.join("minecraftWorlds"))?;
    Ok(Inventory { entries, worlds })
}

pub fn open_folder(install: &BedrockInstall, folder: &str) -> Result<()> {
    let path = folder_path(install, folder)?;
    let mut command = std::process::Command::new(if cfg!(windows) {
        "explorer.exe"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    });
    command
        .arg(path)
        .spawn()
        .map_err(|e| format!("Cannot open Bedrock data: {e}"))?;
    Ok(())
}

impl App {
    pub(crate) fn edition_switch(&self) -> Element<'_, Message> {
        let mut choices = row![].spacing(6);
        for edition in Edition::ALL {
            choices = choices.push(action(
                edition.name(),
                Message::Edition(edition),
                if self.edition == edition {
                    Button::Selected
                } else {
                    Button::Quiet
                },
            ));
        }
        column![muted("EDITION").size(10), choices]
            .spacing(7)
            .into()
    }

    pub(crate) fn bedrock_view(&self) -> Element<'_, Message> {
        let mut page = column![
            muted("MINECRAFT / BEDROCK EDITION").size(11),
            title("The worlds already here."),
            muted("Open the copy Windows installed. Your worlds stay in the game's own folders, separate from Java collections."),
            self.actions(vec![
                if self.bedrock.loading {
                    button(label("Detecting…")).padding(12).style(Button::Quiet).into()
                } else {
                    action("Refresh installed copies", Message::BedrockRefresh, Button::Primary)
                },
                action("Accounts & access", Message::Open(Dialog::Account), Button::Quiet)
            ]),
        ].spacing(16);
        page = page.push(
            self.actions(
                Tab::ALL
                    .into_iter()
                    .map(|tab| {
                        action(
                            tab.name(),
                            Message::BedrockTab(tab),
                            if tab == self.bedrock.tab {
                                Button::Selected
                            } else {
                                Button::Quiet
                            },
                        )
                    })
                    .collect(),
            ),
        );
        if self.bedrock.tab == Tab::Access {
            return page.push(self.bedrock_account_view()).into();
        }
        if matches!(self.bedrock.tab, Tab::Worlds | Tab::Content) {
            let mut body = column![
                design::emblem(if self.bedrock.tab == Tab::Worlds {
                    "worlds"
                } else {
                    "additions"
                }),
                design::input("Filter folder names", &self.bedrock.query)
                    .on_input(Message::BedrockQuery)
                    .padding(12)
            ]
            .spacing(14);
            body = body.push(if self.bedrock.tab == Tab::Content {
                action("Import an add-on…", Message::Open(Dialog::Surface(crate::surfaces::Panel::Addition(String::new()))), Button::Primary)
            } else { muted("Close the game before making a reviewed .mcworld copy. World databases are never opened or edited.").into() });
            if self.bedrock.selected.is_none() {
                body = body.push(muted(
                    "Choose Inspect worlds & content on an installed copy to read its folders.",
                ));
            }
            if self.bedrock.reading {
                body = body.push(muted("Reading detected player folders…"));
            }
            if let Some(error) = &self.bedrock.files_error {
                body = body.push(muted(error));
            }
            let entries: Vec<_> = self
                .bedrock
                .entries
                .iter()
                .filter(|entry| {
                    (if self.bedrock.tab == Tab::Worlds {
                        entry.folder == "minecraftWorlds"
                    } else {
                        entry.folder != "minecraftWorlds"
                    }) && entry
                        .name
                        .to_lowercase()
                        .contains(&self.bedrock.query.to_lowercase())
                })
                .collect();
            let worlds = self.bedrock.matching_worlds();
            let empty = if self.bedrock.tab == Tab::Worlds {
                worlds.is_empty()
            } else {
                entries.is_empty()
            };
            if empty && !self.bedrock.reading {
                body = body.push(muted("No matching folders to show. Refresh or inspect an installed copy; no sample worlds are supplied."));
            }
            if self.bedrock.tab == Tab::Worlds {
                for world in worlds.iter().take(100) {
                    body = body.push(card(column![
                        subtitle(&world.name),
                        muted(format!("Folder: {} · Version: {}", world.folder, world.game_version.as_deref().unwrap_or("Not read"))),
                        muted(if world.readable { "Metadata read; the world database remains untouched." } else { "Title metadata could not be read. The folder is still present and can be exported." }),
                        action("Backup / export…", Message::Open(Dialog::Surface(crate::surfaces::Panel::World(String::new(), world.folder.clone()))), Button::Quiet)
                    ].spacing(8)));
                }
            }
            for entry in entries
                .iter()
                .filter(|e| e.folder != "minecraftWorlds")
                .take(100)
            {
                body = body.push(card(
                    column![label(&entry.name), muted(&entry.folder).size(12)].spacing(8),
                ));
            }
            page = page.push(card(body));
        }
        if let Some(error) = &self.bedrock.error {
            page = page.push(card(
                column![subtitle("Detection unavailable"), muted(error)].spacing(8),
            ));
        } else if self.bedrock.checked && self.bedrock.installs.is_empty() {
            page = page.push(card(column![
                design::collection(),
                subtitle("No installed copy found"),
                muted("Install your owned Bedrock copy through Microsoft Store or the official launcher, open it once, then refresh here. Preview is a separate installation."),
            ].spacing(12)));
        } else if !self.bedrock.checked && !self.bedrock.loading {
            page = page.push(muted(
                "Refresh to read installed packages. This does not install or register anything.",
            ));
        }
        for install in &self.bedrock.installs {
            let channel = install.channel;
            let launch = if self.pending.contains_key("bedrock-launch") {
                button(label("Opening…"))
                    .padding(12)
                    .style(Button::Quiet)
                    .into()
            } else {
                action(
                    "Open game",
                    Message::BedrockLaunch(channel),
                    Button::Primary,
                )
            };
            page = page.push(card(
                column![
                    row![
                        design::mark(32.0),
                        column![
                            subtitle(format!("Bedrock · {channel}")),
                            muted(if install.version.is_empty() {
                                "Version not reported"
                            } else {
                                &install.version
                            })
                        ]
                        .spacing(4)
                        .width(Length::Fill),
                    ]
                    .spacing(12),
                    muted(&install.package_name).size(12),
                    row![
                        launch,
                        action(
                            "Inspect worlds & content",
                            Message::BedrockInspect(channel),
                            Button::Quiet
                        )
                    ]
                    .spacing(8),
                ]
                .spacing(12),
            ));
        }
        if let Some(channel) = self.bedrock.selected {
            let install = self
                .bedrock
                .installs
                .iter()
                .find(|copy| copy.channel == channel);
            let mut files = column![subtitle(format!("{channel} / worlds & content"))].spacing(10);
            if let Some(path) = install.and_then(|copy| copy.data_path.as_ref()) {
                files = files.push(muted(path.display()).size(12));
            }
            if self.bedrock.reading {
                files = files.push(muted("Reading folder names…"));
            } else if let Some(error) = &self.bedrock.files_error {
                files = files.push(muted(error));
            } else {
                for (folder, caption) in [
                    ("minecraftWorlds", "Saved worlds"),
                    ("resource_packs", "Resource packs"),
                    ("behavior_packs", "Behaviour packs"),
                ] {
                    let entries: Vec<_> = self
                        .bedrock
                        .entries
                        .iter()
                        .filter(|entry| entry.folder == folder)
                        .collect();
                    files = files.push(
                        row![
                            label(format!("{caption} · {}", entries.len())).width(Length::Fill),
                            action(
                                "Open folder",
                                Message::BedrockFolder(channel, folder.into()),
                                Button::Quiet
                            ),
                        ]
                        .spacing(8),
                    );
                    for entry in entries.iter().take(50) {
                        files = files.push(muted(&entry.name).size(12));
                    }
                    if entries.len() > 50 {
                        files = files.push(muted(
                            "Showing the first 50 folders. Open the folder for the full list.",
                        ));
                    }
                    if entries.is_empty() {
                        files = files.push(muted(
                            "No folders found. The game may not have created this folder yet.",
                        ));
                    }
                }
            }
            files = files.push(muted("Inspection is read-only. The Worlds tab reads title metadata without opening databases; imports and exports require a separate file review and confirmation."));
            page = page.push(card(files));
        }
        page.push(card(column![
            subtitle("Managed, not modified"),
            muted("Windows and the game enforce ownership and Xbox sign-in. Open game does not switch accounts, guarantee offline access, or bypass licence checks."),
            muted("The launcher cannot track or stop Bedrock sessions. Close the game in its own window before confirming a world export or local pack import. Single and multi-pack imports show each manifest-defined destination before confirmation."),
        ].spacing(10))).into()
    }

    pub(crate) fn bedrock_account_view(&self) -> Element<'_, Message> {
        column![
            design::emblem("account"),
            subtitle("Bedrock / Windows & Xbox"),
            label("Local play"),
            muted("Open the installed game without changing account state. Windows still requires a valid entitlement; offline availability is decided by the game."),
            label("Microsoft account"),
            muted("Sign in to Microsoft Store and Xbox using the account that owns Bedrock, then manage multiplayer sign-in inside the game. Java device-code sign-in does not change Bedrock's account."),
            muted("This launcher has no Bedrock entitlement pre-check or account switcher yet. An installed package is not proof of ownership."),
            action("Return to installed copies", Message::CloseDialog, Button::Primary),
            action("Access checklist", Message::Open(Dialog::Surface(crate::surfaces::Panel::BedrockAccess)), Button::Quiet),
            action("Manage Java identities", Message::Edition(Edition::Java), Button::Quiet),
            Space::with_height(2),
        ].spacing(14).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn install(root: PathBuf) -> BedrockInstall {
        BedrockInstall {
            channel: Channel::Release,
            package_name: "test".into(),
            version: "1".into(),
            install_path: None,
            data_path: Some(root),
        }
    }
    #[test]
    fn world_filter_matches_title_or_folder_without_matching_unrelated_folders() {
        let mut state = State {
            entries: vec![Entry {
                name: "unrelated".into(),
                folder: "minecraftWorlds".into(),
            }],
            worlds: vec![palantir_bedrock::worlds::WorldSummary {
                folder: "opaque-id".into(),
                name: "Copper Valley".into(),
                last_played: None,
                game_version: None,
                readable: true,
            }],
            ..Default::default()
        };
        for query in ["COPPER", "opaque-id", ""] {
            state.query = query.into();
            assert_eq!(state.matching_worlds().len(), 1);
        }
        state.query = "unrelated".into();
        assert!(state.matching_worlds().is_empty());
        state.worlds.clear();
        state.query.clear();
        assert!(state.matching_worlds().is_empty());
    }
    #[test]
    fn bedrock_inventory_is_read_only_and_paths_are_allowlisted() {
        let root = std::env::temp_dir().join(format!("palantir-bedrock-ui-{}", std::process::id()));
        std::fs::create_dir_all(root.join("minecraftWorlds/world-id")).unwrap();
        std::fs::write(
            root.join("minecraftWorlds/world-id/level.dat"),
            b"unchanged",
        )
        .unwrap();
        let copy = install(root.clone());
        let entries = inventory(&copy).unwrap();
        assert_eq!(entries.entries.len(), 1);
        assert_eq!(entries.entries[0].name, "world-id");
        assert_eq!(entries.worlds.len(), 1);
        assert!(!entries.worlds[0].readable);
        assert_eq!(
            std::fs::read(root.join("minecraftWorlds/world-id/level.dat")).unwrap(),
            b"unchanged"
        );
        assert!(!root.join("resource_packs").exists());
        assert!(folder_path(&copy, "../outside").is_err());
        assert!(folder_path(&copy, "resource_packs").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn linked_bedrock_content_is_refused() {
        let root =
            std::env::temp_dir().join(format!("palantir-bedrock-ui-link-{}", std::process::id()));
        std::fs::create_dir_all(root.join("outside")).unwrap();
        std::os::unix::fs::symlink(root.join("outside"), root.join("resource_packs")).unwrap();
        assert!(
            inventory(&install(root.clone()))
                .unwrap_err()
                .contains("linked")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
