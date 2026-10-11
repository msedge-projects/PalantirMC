use crate::design::input;
use crate::design::{self, Button, label, muted, subtitle};
use crate::model::{self, Loader};
use crate::{App, Dialog, Message, ProjectTab, action, byte_size, card, markdown};
use iced::widget::{button, checkbox, column, container, row, scrollable, text_input};
use iced::{Element, Length};

impl App {
    pub(crate) fn extended_dialog(&self, dialog: &Dialog) -> Option<Element<'_, Message>> {
        let (heading, body): (&str, Element<'_, Message>) = match dialog {
            Dialog::Create => {
                let mut body = column![muted("Choose the game and loader for an independent collection."),label("Collection name"),input("Quiet valley", &self.name).id(text_input::Id::new("dialog-first")).on_input(Message::Name).padding(12),label("Minecraft version"),row![input("Version identifier", &self.game).on_input(Message::Game).padding(12),action("Choose version…",Message::Open(Dialog::Versions),Button::Quiet)].spacing(8),label("Loader"),crate::controls::picker(Loader::ALL,Some(self.loader),Message::Loader)].spacing(12);
                if self.loader != Loader::Vanilla {
                    body = body.push(label("Loader release"));
                    let mut choices = vec!["Automatic".to_string()]; choices.extend(self.loader_versions.clone());
                    body = body.push(crate::controls::picker(choices, Some(if self.loader_pin.is_empty() { "Automatic".into() } else { self.loader_pin.clone() }), Message::LoaderPin));
                    if self.loader_loading { body = body.push(muted("Reading loader releases…")); }
                    if !self.loader.desktop_supported() { body = body.push(muted("This loader is represented in the frontend. Its processor-based desktop backend is unavailable; this saved collection cannot play until that integration exists.")); }
                }
                let valid = !self.name.trim().is_empty() && model::safe_component(self.game.trim()).is_ok();
                body = body.push(muted("The collection is saved now. Game files download only when you choose Play."));
                body = body.push(if self.pending.contains_key("create") { disabled("Creating…") } else if !valid { disabled("Enter a name and game version") } else { action("Create collection", Message::Create, Button::Primary) });
                ("A new collection",body.into())
            }
            Dialog::Versions => {
                let mut body = column![row![action("← Collection setup",Message::Open(Dialog::Create),Button::Quiet),action("Refresh versions",Message::LoadVersions,Button::Quiet)].spacing(8),crate::controls::toggle("Include snapshots and historical builds",self.include_snapshots,Message::IncludeSnapshots),input("Filter version identifiers",&self.versions_query).on_input(Message::VersionsQuery).padding(12)].spacing(12);
                if self.versions_loading { body=body.push(muted("Loading the public game manifest…")); }
                if self.game_choices.is_empty() && !self.versions_loading { body=body.push(muted("Version listing unavailable. Return to setup and enter a version manually, or refresh.")); }
                for choice in self.game_choices.iter().filter(|c|(self.include_snapshots || c.kind=="release")&&c.id.to_lowercase().contains(&self.versions_query.to_lowercase())).take(300) { body=body.push(action(&choice.to_string(),Message::ChooseGame(choice.id.clone()),Button::Quiet)); }
                ("Choose a game version",body.into())
            }
            Dialog::Import => ("Bring a collection",column![muted("Browse to a plain game directory, Prism/MultiMC directory, or .mrpack archive. Inspection does not copy or download anything."),label("Source path"),input("Directory or .mrpack path", &self.import_path).id(text_input::Id::new("dialog-first")).on_input(Message::ImportPath).on_submit(Message::InspectImport).padding(12),row![action("Browse…",Message::Open(Dialog::Browser),Button::Quiet),if self.import_loading {disabled("Inspecting…")}else{action("Inspect source",Message::InspectImport,Button::Primary)}].spacing(8),muted("Directory copies are connected. Archives can be reviewed, including optional files, but backend pack installation is not yet available.")].spacing(14).into()),
            Dialog::Browser => {
                let mut body=column![label("Folder"),input("Absolute folder path",&self.browser_path).on_input(Message::BrowsePath).on_submit(Message::BrowseFolder).padding(12),row![action("Read folder",Message::BrowseFolder,Button::Quiet),action("Use this directory",Message::ChooseSource(self.browser_path.clone()),Button::Primary)].spacing(8)].spacing(10);
                if let Some(parent)=std::path::Path::new(&self.browser_path).parent() {body=body.push(action("↑ Parent folder",Message::BrowsePathAndRead(parent.display().to_string()),Button::Quiet));}
                if self.browse_loading {body=body.push(muted("Reading directory…"));}
                for entry in self.directory.iter().take(300) { let line=if entry.directory {row![design::icon("folder",false),action(&entry.name,Message::BrowsePathAndRead(entry.path.clone()),Button::Quiet),action("Select",Message::ChooseSource(entry.path.clone()),Button::Primary)].spacing(8)}else{row![design::icon("download",false),action(&entry.name,Message::ChooseSource(entry.path.clone()),Button::Quiet)].spacing(8)};body=body.push(line);}
                ("Choose an import source",body.into())
            }
            Dialog::ImportReview => {
                let mut body=column![action("← Change source",Message::Open(Dialog::Import),Button::Quiet)].spacing(12);
                if let Some(review)=&self.import_review {
                    body=body.push(design::emblem("download")).push(subtitle(&review.name)).push(muted(&review.source)).push(label(format!("{} files · {} · {} / {}",review.files,byte_size(review.bytes),if review.game.is_empty(){self.game.as_str()}else{&review.game},review.loader))).push(label("New collection name")).push(input("Imported collection",&self.name).on_input(Message::Name).padding(12));
                    if review.game.is_empty(){body=body.push(label("Game version / loader")).push(input("Minecraft version",&self.game).on_input(Message::Game).padding(12)).push(crate::controls::picker(Loader::ALL,Some(self.loader),Message::Loader));}
                    for note in &review.notes{body=body.push(muted(note));}
                    if !review.optional.is_empty(){body=body.push(subtitle("Optional client files"));for path in &review.optional{body=body.push(checkbox(path,self.optional_files.contains(path)).on_toggle(move|v|Message::OptionalFile(path.clone(),v)));}}
                    body=body.push(if self.pending.contains_key("import"){disabled("Copying…")}else if review.copy_supported&&self.loader.desktop_supported(){action("Copy into a new collection",Message::Import,Button::Primary)}else{disabled("Installation backend unavailable")});
                }else{body=body.push(muted("No source review. Go back and inspect a source."));}
                ("Review before importing",body.into())
            }
            Dialog::SignIn => ("Microsoft device-code sign-in",self.authentication_view()),
            Dialog::RemoveContent(id,folder,name)=>("Remove this addition?",column![subtitle(name),muted(format!("Remove this file from {folder}? Saved worlds and other files are not touched. This cannot be undone by the launcher.")),action("Remove file permanently",Message::RemoveContent(id.clone(),folder.clone(),name.clone()),Button::Danger)].spacing(14).into()),
            Dialog::PackSetup=>{
                let mut body=column![design::emblem("download"),subtitle("A new collection for this pack"),input("Collection name",&self.name).on_input(Message::Name).padding(12)].spacing(14);
                if let Some(release)=self.selected_release(){body=body.push(label(format!("Release: {} / {}",release.name,release.version_number))).push(muted(format!("Game versions: {}\nLoaders: {}",release.game_versions.join(", "),release.loaders.join(", "))));}
                body=body.push(muted("The frontend selection is ready. The backend must inspect the downloaded pack index, verify files, apply overrides, and resolve optional choices before creating or launching this collection. Archive coordination is not available; existing worlds will not be overwritten.")).push(action("Review a local .mrpack…",Message::Open(Dialog::Import),Button::Quiet)).push(disabled("Pack installation backend unavailable"));
                ("Review modpack installation",body.into())
            },
            Dialog::LoaderStatus => ("Loader integration",column![design::emblem("download"),subtitle("Forge / NeoForge"),muted("The frontend supports choosing and saving these loader identities. Launching requires the backend to resolve its installer, execute processors, verify outputs, and prepare the resulting version."),muted("That desktop coordinator is not connected yet. Your collection is preserved; no incomplete install or fabricated success is shown."),action("Create a supported collection",Message::Open(Dialog::Create),Button::Quiet)].spacing(14).into()),
            Dialog::Preferences => ("Launcher preferences",self.settings()),
            Dialog::Credits => ("Notices & diagnostics",column![subtitle("Original product assets"),muted("Wayfinder, terrain, shelf, identity card, transfer waypoints, modular additions and navigation geometry were authored for this rewrite. The product licence remains the owner's UNLICENSED placeholder."),subtitle("Runtime dependencies"),label("iced: MIT\nserde / serde_json / reqwest / sha1: MIT OR Apache-2.0\nzip: MIT\nSystem fonts: provided by your operating system, not bundled"),muted("The complete graph and asset provenance are recorded in THIRD_PARTY_NOTICES.md and Cargo.lock. The audit does not replace trademark/design legal review."),action("Copy launcher diagnostics",Message::CopyDiagnostics,Button::Primary),action("Open data directory",Message::OpenFolder(None,String::new()),Button::Quiet)].spacing(16).into()),
            Dialog::InstallReview(id) => {
                let mut body=column![design::emblem("download"),action("← Choose collection",Message::Open(Dialog::Install),Button::Quiet)].spacing(12);
                if let(Some(project),Some(release),Some(item))=(&self.project,self.selected_release(),self.instances.iter().find(|i|&i.id==id)){
                    body=body.push(subtitle(&project.title)).push(label(format!("{} · {}",release.name,release.version_number))).push(label(format!("Destination: {} · {} / {}",item.name,item.game_version,item.loader)));
                    if let Some(file)=release.primary_file(){body=body.push(label(format!("{} · {}",file.filename,byte_size(file.size)))).push(muted("SHA-1 and size are verified. Existing files are never silently replaced."));}
                    for dep in &release.dependencies{body=body.push(muted(format!("{} dependency: {}",dep.dependency_type,dep.project_id.as_deref().or(dep.version_id.as_deref()).or(dep.file_name.as_deref()).unwrap_or("unspecified"))));}
                    let compatible=release.compatible(item,&project.project_type);
                    if !compatible{body=body.push(muted("This release does not match the selected game/loader."));}
                    if release.required_dependencies(){body=body.push(muted("Required dependency installation is unavailable in the backend. Review is complete; no partial installation is allowed."));}
                    body=body.push(if compatible&&!release.required_dependencies()&&project.project_type!="modpack"&&release.project_id==project.project_id&&!self.running.contains_key(id)&&!self.pending.contains_key(id){action("Confirm installation",Message::InstallContent(id.clone()),Button::Primary)}else{disabled("Resolve backend requirements before installing")});
                }else{body=body.push(muted("Choose a release and collection before reviewing the installation."));}
                ("Review installation",body.into())
            }
            Dialog::Install => {
                let mut body=column![muted("Choose a stopped collection matching the selected release. Every choice opens a final compatibility/dependency review.")].spacing(12);
                if let Some(project)=&self.project {
                    body=body.push(subtitle(&project.title));
                    if let Some(release)=self.selected_release(){body=body.push(label(format!("Release: {} / {}",release.name,release.version_number)));}
                    if project.project_type=="modpack"{body=body.push(muted("A modpack needs a new collection. Configure the frontend below; backend pack installation is unavailable."));body=body.push(action("Review pack releases",Message::ShowReleases,Button::Quiet));body=body.push(action("Configure new pack collection",Message::Open(Dialog::PackSetup),Button::Primary));}
                    let mut found=false;
                    for item in &self.instances {
                        if project.project_type=="modpack" { continue; }
                        if self.running.contains_key(&item.id)||self.pending.contains_key(&item.id){continue;}
                        if let Some(release)=self.selected_release(){if !release.compatible(item,&project.project_type){continue;}}
                        if project.project_type=="mod"&&item.loader==Loader::Vanilla{continue;}
                        found=true;body=body.push(action(&format!("{} · {} / {}",item.name,item.game_version,item.loader),Message::Open(Dialog::InstallReview(item.id.clone())),Button::Primary));
                    }
                    if !found {body=body.push(muted("No compatible stopped collection. Choose another release or create a matching collection."));body=body.push(action("Create a collection",Message::Open(Dialog::Create),Button::Quiet));}
                }
                ("Add to a collection",body.into())
            }
            _=>return None,
        };
        let mut panel = column![
            row![
                subtitle(heading).width(Length::Fill),
                action("Close", Message::CloseDialog, Button::Quiet)
            ]
            .spacing(10)
        ]
        .spacing(14);
        if let Some(notice) = &self.notice {
            panel = panel.push(container(label(notice)).padding(12).style(design::well));
        }
        panel = panel.push(
            scrollable(body)
                .id(scrollable::Id::new("dialog-body"))
                .height(Length::Fill),
        );
        Some(
            container(panel)
                .padding(20)
                .width(if matches!(dialog, Dialog::Preferences) {
                    850
                } else {
                    610
                })
                .height(Length::Fill)
                .max_height(if matches!(dialog, Dialog::Preferences) {
                    700
                } else {
                    600
                })
                .style(design::panel)
                .into(),
        )
    }
    pub(crate) fn project_links(
        &self,
        project: &model::Project,
        detail: &model::ProjectDetail,
    ) -> Element<'_, Message> {
        let mut links = column![action(
            "Open project website",
            Message::OpenUrl(format!(
                "https://modrinth.com/project/{}",
                project.project_id
            )),
            Button::Quiet
        )]
        .spacing(8);
        for (caption, url) in [
            ("Source code", &detail.source_url),
            ("Report an issue", &detail.issues_url),
            ("Documentation", &detail.wiki_url),
        ] {
            if let Some(url) = url {
                links = links.push(action(
                    caption,
                    Message::OpenUrl(url.clone()),
                    Button::Quiet,
                ));
            }
        }
        links
            .push(action(
                "Retry information",
                Message::RetryProject,
                Button::Quiet,
            ))
            .into()
    }
    fn authentication_view(&self) -> Element<'_, Message> {
        use model::Authentication;
        let mut body = column![design::emblem("account")].spacing(14);
        match &self.authentication {
            Authentication::Offline | Authentication::Cancelled => {
                body=body.push(subtitle("Connect your owned game")).push(muted("Request a device code, open Microsoft's verification page, then wait for ownership and player-profile verification. This backend currently has no registered authentication client; no code or authenticated success will be invented.")).push(action("Check sign-in availability",Message::AuthStart,Button::Primary));
            }
            Authentication::Requesting => {
                body = body
                    .push(subtitle("Requesting a device code…"))
                    .push(muted("Waiting for the authentication backend."))
                    .push(action("Cancel sign-in", Message::AuthCancel, Button::Quiet));
            }
            Authentication::DeviceCode {
                code,
                url,
                expires_in,
            } => {
                body=body.push(subtitle("Verify in your browser")).push(label(code).size(28)).push(muted(format!("Code expires in {expires_in} seconds. This code was supplied by the authentication backend."))).push(action("Copy device code",Message::CopyDeviceCode,Button::Quiet)).push(action("Open verification page",Message::OpenUrl(url.clone()),Button::Primary)).push(action("Cancel sign-in",Message::AuthCancel,Button::Quiet));
            }
            Authentication::Verifying => {
                body = body
                    .push(subtitle("Checking account and game ownership…"))
                    .push(muted(
                        "The browser step is complete; waiting for the verified player profile.",
                    ))
                    .push(action("Cancel sign-in", Message::AuthCancel, Button::Quiet));
            }
            Authentication::Connected { player } => {
                body=body.push(subtitle(format!("Connected as {player}"))).push(muted("Verified account state supplied by the authentication backend. Session tokens are not displayed.")).push(action("Disconnect account",Message::AuthCancel,Button::Danger));
            }
            Authentication::Expired => {
                body = body
                    .push(subtitle("The device code expired"))
                    .push(muted("Request a fresh code. No account was changed."))
                    .push(action(
                        "Try sign-in again",
                        Message::AuthStart,
                        Button::Primary,
                    ));
            }
            Authentication::Failed(error) => {
                body = body
                    .push(subtitle("Authentication unavailable"))
                    .push(muted(error))
                    .push(action(
                        "Check availability again",
                        Message::AuthStart,
                        Button::Primary,
                    ));
            }
        }
        body.push(action(
            "Use offline identity",
            Message::Open(Dialog::Account),
            Button::Quiet,
        ))
        .into()
    }
    pub(crate) fn selected_release(&self) -> Option<&model::Release> {
        self.release_id
            .as_ref()
            .and_then(|id| self.releases.iter().find(|r| &r.id == id))
    }
    pub(crate) fn project_tabs(&self) -> Element<'_, Message> {
        let mut body = column![
            row(ProjectTab::ALL.into_iter().map(|tab| action(
                tab.name(),
                Message::ProjectTab(tab),
                if self.project_tab == tab {
                    Button::Selected
                } else {
                    Button::Quiet
                }
            )))
            .spacing(8)
        ]
        .spacing(16);
        if self.releases_loading {
            body = body.push(muted("Loading releases…"));
        }
        match self.project_tab {
            ProjectTab::Description => {
                if let Some(detail) = &self.detail {
                    body = body.push(markdown(&detail.body));
                }
            }
            ProjectTab::Releases => {
                body = body.push(
                    input(
                        "Filter releases by game, loader or version",
                        &self.release_filter,
                    )
                    .on_input(Message::ReleaseFilter)
                    .padding(12),
                );
                if self.releases.is_empty() && !self.releases_loading {
                    body = body.push(muted("No releases available.")).push(action(
                        "Retry project information",
                        Message::RetryProject,
                        Button::Quiet,
                    ));
                }
                let query = self.release_filter.to_lowercase();
                let filtered: Vec<_> = self
                    .releases
                    .iter()
                    .filter(|r| {
                        format!(
                            "{} {} {} {}",
                            r.name,
                            r.version_number,
                            r.game_versions.join(" "),
                            r.loaders.join(" ")
                        )
                        .to_lowercase()
                        .contains(&query)
                    })
                    .collect();
                for release in filtered.iter().take(self.visible_releases) {
                    body = body.push(card(
                        column![
                            subtitle(&release.name),
                            label(format!(
                                "{} · {} · {}",
                                release.version_number,
                                release.version_type,
                                release.date_published.split('T').next().unwrap_or("")
                            )),
                            muted(format!(
                                "Game: {}\nLoaders: {}",
                                release.game_versions.join(", "),
                                release.loaders.join(", ")
                            )),
                            row![
                                action(
                                    "Select release",
                                    Message::ChooseRelease(release.id.clone()),
                                    Button::Primary
                                ),
                                action(
                                    "Read changelog",
                                    Message::ReleaseChangelog(release.id.clone()),
                                    Button::Quiet
                                )
                            ]
                            .spacing(8)
                        ]
                        .spacing(12),
                    ));
                }
                if filtered.len() > self.visible_releases {
                    body = body.push(action(
                        "Show more releases",
                        Message::MoreReleases,
                        Button::Quiet,
                    ));
                }
                if filtered.is_empty() && !self.releases_loading {
                    body = body.push(muted("No releases match this filter."));
                }
            }
            ProjectTab::Changelog => {
                if let Some(release) = self.selected_release() {
                    body = body
                        .push(subtitle(&release.name))
                        .push(markdown(&release.changelog));
                    if release.changelog.trim().is_empty() {
                        body = body.push(muted("The author supplied no changelog."));
                    }
                } else {
                    body = body.push(muted("Select a release to read its changelog."));
                }
            }
        }
        body.into()
    }
}
fn disabled(caption: &str) -> Element<'_, Message> {
    button(label(caption))
        .padding([10, 14])
        .style(Button::Quiet)
        .into()
}
