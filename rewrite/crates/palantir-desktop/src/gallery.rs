//! Explicit developer gallery: opt-in sample state, never a production fallback.
use crate::design::Button;
use crate::model::{Instance, Loader, Release};
use crate::surfaces::Panel;
use crate::{App, Dialog, Message, action};
use iced::Element;
use iced::widget::{column, row};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    Home,
    Empty,
    Busy,
    Error,
    Project,
    Pack,
    Optional,
    SignIn,
    Code,
    Verified,
    Expired,
    World,
    Content,
    Bedrock,
    MissingData,
    Help,
}
impl Scene {
    pub const ALL: [Self; 16] = [
        Self::Home,
        Self::Empty,
        Self::Busy,
        Self::Error,
        Self::Project,
        Self::Pack,
        Self::Optional,
        Self::SignIn,
        Self::Code,
        Self::Verified,
        Self::Expired,
        Self::World,
        Self::Content,
        Self::Bedrock,
        Self::MissingData,
        Self::Help,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Empty => "empty",
            Self::Busy => "busy",
            Self::Error => "error",
            Self::Project => "project",
            Self::Pack => "pack",
            Self::Optional => "optional",
            Self::SignIn => "signin",
            Self::Code => "code",
            Self::Verified => "verified",
            Self::Expired => "expired",
            Self::World => "world",
            Self::Content => "content",
            Self::Bedrock => "bedrock",
            Self::MissingData => "missing-data",
            Self::Help => "help",
        }
    }
}
impl std::fmt::Display for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}
pub fn scene(app: &mut App, scene: Scene) {
    populate(app);
    app.page = crate::Page::Home;
    app.edition = crate::model::Edition::Java;
    app.dialog = None;
    app.notice = None;
    app.selected = None;
    match scene {
        Scene::Home => {}
        Scene::Empty => {
            app.instances.clear();
            app.pending.clear();
            app.bedrock.installs.clear();
        }
        Scene::Busy => {
            app.dialog = Some(Dialog::Surface(Panel::Task("sample-task".into())));
        }
        Scene::Error => {
            app.notice = Some(
                "Sample / service unavailable. Check the connection and retry the action.".into(),
            );
            app.dialog = Some(Dialog::Surface(Panel::Addition(String::new())));
        }
        Scene::Project | Scene::Pack => {
            app.project = Some(crate::model::Project {
                project_id: "sample-pack".into(),
                title: "Sample project / UI gallery".into(),
                description: "Sample project description, not live catalogue data.".into(),
                author: "Sample author".into(),
                downloads: 0,
                project_type: if scene == Scene::Pack {
                    "modpack"
                } else {
                    "mod"
                }
                .into(),
            });
            app.page = crate::Page::Discover;
            app.project_tab = crate::ProjectTab::Releases;
            if scene == Scene::Pack {
                app.dialog = Some(Dialog::PackSetup);
            }
        }
        Scene::Optional => {
            app.import_review = Some(crate::model::ImportReview {
                source: "sample-only.mrpack".into(),
                name: "Sample optional pack".into(),
                game: "1.20.1".into(),
                loader: Loader::Fabric,
                files: 3,
                bytes: 4096,
                optional: vec![
                    "mods/sample-optional.jar".into(),
                    "resourcepacks/sample-colours.zip".into(),
                ],
                notes: vec!["Sample metadata; no archive was opened.".into()],
                copy_supported: false,
                archive_sha1: None,
            });
            app.dialog = Some(Dialog::ImportReview);
        }
        Scene::SignIn | Scene::Code | Scene::Verified | Scene::Expired => {
            app.authentication = match scene {
                Scene::Code => crate::model::Authentication::DeviceCode {
                    code: "SAMPLE-ONLY".into(),
                    url: "https://example.com/verify".into(),
                    expires_in: 600,
                },
                Scene::Verified => crate::model::Authentication::Connected {
                    player: "SamplePlayer".into(),
                },
                Scene::Expired => crate::model::Authentication::Expired,
                _ => crate::model::Authentication::Offline,
            };
            app.dialog = Some(Dialog::SignIn);
        }
        Scene::World => {
            app.dialog = Some(Dialog::Surface(Panel::World(
                "sample-1".into(),
                "Sample world".into(),
            )));
        }
        Scene::Content => {
            app.dialog = Some(Dialog::Surface(Panel::Addition("sample-1".into())));
        }
        Scene::Bedrock | Scene::MissingData => {
            app.edition = crate::model::Edition::Bedrock;
            if scene == Scene::MissingData {
                app.bedrock.files_error = Some(
                    "Sample / no player data detected. Open the game once and refresh.".into(),
                );
            }
        }
        Scene::Help => app.dialog = Some(Dialog::Surface(Panel::Help)),
    }
}
pub fn preview_request(app: &mut App, request: &crate::backend::Request) {
    match request {
        crate::backend::Request::Inventory(_) => app.inventory_loading = false,
        crate::backend::Request::GameVersions => app.versions_loading = false,
        crate::backend::Request::LoaderVersions(_, _) => app.loader_loading = false,
        crate::backend::Request::Releases(_) => app.releases_loading = false,
        crate::backend::Request::Project(_) => app.detail_loading = false,
        crate::backend::Request::Search(_, _, _, _, _, _) => app.searching = false,
        crate::backend::Request::BedrockDetect => app.bedrock.loading = false,
        crate::backend::Request::BedrockInventory(_) => app.bedrock.reading = false,
        crate::backend::Request::ReviewLocal(_, _) => app.local.loading = false,
        crate::backend::Request::FetchPack(_, _, _) => app.pack_loading = false,
        _ => {}
    }
}
pub fn populate(app: &mut App) {
    app.gallery = true;
    app.root = "UI gallery / no data directory".into();
    app.instances = [
        Loader::Vanilla,
        Loader::Fabric,
        Loader::Quilt,
        Loader::Forge,
        Loader::NeoForge,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, loader)| Instance {
        id: format!("sample-{index}"),
        name: format!("Sample / {loader}"),
        game_version: "1.20.1".into(),
        loader,
        installed_version: None,
        loader_version: None,
        window: None,
        memory_mb: None,
        java: None,
        custom: Default::default(),
    })
    .collect();
    app.inventory = vec![
        crate::files::Entry {
            name: "Sample world".into(),
            folder: "saves".into(),
            bytes: 0,
            directory: true,
        },
        crate::files::Entry {
            name: "sample-mod.jar".into(),
            folder: "mods".into(),
            bytes: 4096,
            directory: false,
        },
    ];
    app.project = None;
    app.releases = vec![Release {
        id: "sample-release".into(),
        project_id: "sample-pack".into(),
        name: "Sample pack release".into(),
        version_number: "1.0".into(),
        version_type: "release".into(),
        date_published: String::new(),
        changelog: "Sample notes; not catalogue data.".into(),
        game_versions: vec!["1.20.1".into()],
        loaders: vec!["fabric".into()],
        files: vec![],
        dependencies: vec![],
    }];
    app.release_id = Some("sample-release".into());
    app.name = "Sample pack collection".into();
    app.game_choices = vec![crate::model::GameChoice {
        id: "1.20.1".into(),
        kind: "release".into(),
    }];
    app.preferences.offline_accounts = vec![crate::model::OfflineAccount::new("SamplePlayer")];
    app.pending.insert(
        "sample-task".into(),
        "Sample / preparing verified files".into(),
    );
    app.bedrock.checked = true;
    app.bedrock.installs = vec![palantir_bedrock::BedrockInstall {
        channel: palantir_bedrock::Channel::Release,
        package_name: "Sample package / not installed".into(),
        version: "Sample version".into(),
        install_path: None,
        data_path: None,
    }];
    app.bedrock.selected = Some(palantir_bedrock::Channel::Release);
    app.bedrock.worlds = vec![palantir_bedrock::worlds::WorldSummary {
        folder: "sample-folder-id".into(),
        name: "Sample Copper Valley".into(),
        last_played: None,
        game_version: Some("Sample version".into()),
        readable: true,
    }];
    app.bedrock.entries = vec![
        crate::bedrock::Entry {
            name: "Sample world".into(),
            folder: "minecraftWorlds".into(),
        },
        crate::bedrock::Entry {
            name: "Sample resource pack".into(),
            folder: "resource_packs".into(),
        },
    ];
}
impl App {
    pub(crate) fn gallery_links(&self) -> Element<'_, Message> {
        let panels = [
            ("Pack setup", Dialog::PackSetup),
            ("Pack review", Dialog::Surface(Panel::PackReview)),
            (
                "Loader setup",
                Dialog::Surface(Panel::Loader("sample-3".into())),
            ),
            ("Session", Dialog::Surface(Panel::Launch("sample-1".into()))),
            (
                "World",
                Dialog::Surface(Panel::World("sample-1".into(), "Sample world".into())),
            ),
            (
                "Content",
                Dialog::Surface(Panel::Addition("sample-1".into())),
            ),
            ("Task", Dialog::Surface(Panel::Task("sample-task".into()))),
        ];
        let rows = panels.chunks(3).map(|chunk| {
            row(chunk
                .iter()
                .map(|(name, dialog)| action(name, Message::Open(dialog.clone()), Button::Quiet)))
            .spacing(6)
            .into()
        });
        column(rows)
            .push(crate::controls::picker(
                Scene::ALL,
                Some(Scene::Home),
                Message::GalleryScene,
            ))
            .spacing(6)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edition, Project};
    use crate::surfaces::{Draft, Edit, LaunchMode, WorldOperation};
    use crate::{InstanceTab, Page, SettingsTab};
    use iced::Application;
    use std::sync::mpsc;
    fn app() -> (App, mpsc::Receiver<crate::backend::Request>) {
        let (worker, requests) = mpsc::channel();
        let (_sender, events) = mpsc::channel();
        let mut app = App::with_worker(worker, events);
        populate(&mut app);
        (app, requests)
    }
    #[test]
    fn all_outer_surfaces_render_across_editions_sizes_and_scales() {
        let (mut app, _) = app();
        for width in [840.0, 1280.0, 1800.0] {
            app.width = width;
            for scale in crate::model::Scale::ALL {
                app.preferences.scale = scale;
                for edition in [Edition::Java, Edition::Bedrock] {
                    app.edition = edition;
                    for page in [Page::Home, Page::Library, Page::Discover, Page::About] {
                        app.page = page;
                        let _ = app.view();
                    }
                    for tab in SettingsTab::ALL {
                        app.settings_tab = tab;
                        let _ = app.settings();
                    }
                    for tab in InstanceTab::ALL {
                        app.instance_tab = tab;
                        let _ = app.instance_detail(&app.instances[1]);
                    }
                    for panel in [
                        Panel::Help,
                        Panel::Launch("sample-1".into()),
                        Panel::World("sample-1".into(), "世界 / long world name".into()),
                        Panel::WorldReview("sample-1".into(), "世界".into()),
                        Panel::Addition("sample-1".into()),
                        Panel::AdditionReview("sample-1".into()),
                        Panel::PackReview,
                        Panel::Loader("sample-3".into()),
                        Panel::Task("sample-task".into()),
                        Panel::RemoveAccount("absent".into()),
                        Panel::BedrockAccess,
                    ] {
                        app.dialog = Some(Dialog::Surface(panel));
                        let _ = app.view();
                    }
                    for tab in crate::bedrock::Tab::ALL {
                        app.bedrock.tab = tab;
                        let _ = app.bedrock_view();
                    }
                    app.dialog = None;
                }
            }
        }
    }
    #[test]
    fn all_gallery_scenes_render_and_never_start_the_worker() {
        let (mut app, requests) = app();
        for scene_value in Scene::ALL {
            scene(&mut app, scene_value);
            let _ = app.view();
        }
        assert!(requests.try_recv().is_err());
        assert_eq!(app.preferences.offline_accounts.len(), 1);
    }
    #[test]
    fn frontend_drafts_and_gallery_actions_cannot_invoke_the_engine() {
        let (mut app, requests) = app();
        let original = app.preferences.clone();
        for edit in [
            Edit::Source("/example/pack.mcaddon".into()),
            Edit::Output("/example/world.mcworld".into()),
            Edit::WorldOperation(WorldOperation::Export),
            Edit::LaunchMode(LaunchMode::Microsoft),
        ] {
            let _ = app.update(Message::Draft(edit));
        }
        assert_eq!(app.preferences, original);
        assert!(Draft::path_valid(&app.draft.source));
        assert!(!Draft::path_valid("\n"));
        let _ = app.update(Message::Launch("sample-1".into()));
        assert!(requests.try_recv().is_err());
        assert!(app.notice.as_deref().unwrap().contains("GALLERY"));
    }
    #[test]
    fn every_auth_surface_is_renderable_with_real_backend_state_shapes() {
        let (mut app, _) = app();
        for state in [
            crate::model::Authentication::Offline,
            crate::model::Authentication::Requesting,
            crate::model::Authentication::DeviceCode {
                code: "SAMPLE-ONLY".into(),
                url: "https://example.com/verify".into(),
                expires_in: 600,
            },
            crate::model::Authentication::Verifying,
            crate::model::Authentication::Connected {
                player: "Sample".into(),
            },
            crate::model::Authentication::Expired,
            crate::model::Authentication::Failed("Sample failure".into()),
            crate::model::Authentication::Cancelled,
        ] {
            app.authentication = state;
            let _ = app.dialog_view(&Dialog::SignIn);
        }
    }
    #[test]
    fn gallery_sample_data_is_explicit_and_pack_selection_is_not_live_content() {
        let (app, _) = app();
        assert!(app.gallery);
        assert!(app.root.contains("gallery"));
        assert!(app.instances.iter().all(|i| i.name.starts_with("Sample")));
        let _project = Project {
            project_id: "sample-pack".into(),
            title: "Sample".into(),
            description: "Not live catalogue data".into(),
            author: "Sample".into(),
            downloads: 0,
            project_type: "modpack".into(),
        };
    }
}
