use crate::design::input;
use crate::design::{self, Button, label, muted, subtitle};
use crate::model::{self, Loader};
use crate::{App, Dialog, Message, ProjectTab, action, byte_size, card, markdown};
use iced::Element;
use iced::widget::{button, checkbox, column, row, text_input};

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
                }
                let valid = !self.name.trim().is_empty() && self.name.chars().count() <= 80 && !self.name.chars().any(char::is_control) && model::safe_component(self.game.trim()).is_ok();
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
            Dialog::Import => ("Bring a collection",column![muted("Browse to a plain game directory, Prism/MultiMC directory, or .mrpack archive. Inspection does not copy or download anything."),label("Source path"),input("Directory or .mrpack path", &self.import_path).id(text_input::Id::new("dialog-first")).on_input(Message::ImportPath).on_submit(Message::InspectImport).padding(12),row![action("Browse…",Message::Open(Dialog::Browser),Button::Quiet),if self.import_loading {disabled("Inspecting…")}else{action("Inspect source",Message::InspectImport,Button::Primary)}].spacing(8),muted("A directory is copied as plain files. An archive is installed from its own index into a new collection, with its optional client files chosen on the review.")].spacing(14).into()),
            Dialog::Browser => {
                let mut body=column![label("Folder"),input("Absolute folder path",&self.browser_path).on_input(Message::BrowsePath).on_submit(Message::BrowseFolder).padding(12),self.actions(vec![action("Read folder",Message::BrowseFolder,Button::Quiet),action("Use this directory",Message::ChooseSource(self.browser_path.clone()),Button::Primary)])].spacing(10);
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
                    if !review.optional.is_empty(){body=body.push(subtitle("Optional client files"));for path in &review.optional{let message=Message::OptionalFile(path.clone(), !self.optional_files.contains(path));body=body.push(crate::controls::content(checkbox(path,self.optional_files.contains(path)).on_toggle(move|v|Message::OptionalFile(path.clone(),v)).into(),message));}}
                    if !review.copy_supported { body = body.push(action("Final pack review", Message::Open(Dialog::Surface(crate::surfaces::Panel::PackReview)), Button::Primary)); }
                    let archive=review.source.to_ascii_lowercase().ends_with(".mrpack");
                    body=body.push(if self.pending.contains_key("import"){disabled("Importing…")}else if self.name.trim().is_empty() { disabled("Enter a new collection name") } else if review.copy_supported{action("Copy into a new collection",Message::Import,Button::Primary)}else if archive{action("Install the pack archive",Message::Import,Button::Primary)}else{disabled("This source cannot be installed as a collection")});
                }else{body=body.push(muted("No source review. Go back and inspect a source."));}
                ("Review before importing",body.into())
            }
            Dialog::Account | Dialog::SignIn if self.edition == model::Edition::Bedrock => ("Bedrock accounts & access", self.bedrock_account_view()),
            Dialog::SignIn => ("Microsoft device-code sign-in",self.authentication_view()),
            Dialog::RemoveContent(id,folder,name)=>("Remove this addition?",column![subtitle(name),muted(format!("Remove this file from {folder}? Saved worlds and other files are not touched. This cannot be undone by the launcher.")),action("Remove file permanently",Message::RemoveContent(id.clone(),folder.clone(),name.clone()),Button::Danger)].spacing(14).into()),
            Dialog::PackSetup=>{
                let mut body=column![design::emblem("download"),subtitle("A new collection for this pack"),input("Collection name",&self.name).on_input(Message::Name).padding(12)].spacing(14);
                if let Some(release)=self.selected_release(){body=body.push(label(format!("Release: {} / {}",release.name,release.version_number))).push(muted(format!("Game versions: {}\nLoaders: {}",release.game_versions.join(", "),release.loaders.join(", "))));} else { body=body.push(muted("Choose a project release or inspect a local archive before installing.")); }
                body=body.push(label("Destination: a new Java collection; existing worlds stay separate"))
                    .push(muted("Download the selected archive for inspection, then choose optional client files before installation. This step only caches the verified archive; it does not create a collection or download its indexed game content."))
                    .push(if self.pack_loading { disabled("Downloading and verifying pack archive…") } else if self.selected_release().is_some() { action("Download selected pack for review", Message::FetchPack, Button::Primary) } else { disabled("Choose a pack release first") })
                    .push(action("Review a local .mrpack…",Message::Open(Dialog::Import),Button::Quiet))
                    .push(if self.import_review.as_ref().is_some_and(|review| review.source == self.import_path && review.archive_sha1.is_some()) && !self.name.trim().is_empty() && self.name.chars().count() <= 80 { action("Continue to final pack review", Message::Open(Dialog::Surface(crate::surfaces::Panel::PackReview)), Button::Quiet) } else { disabled("Download or inspect the archive before final review") });
                ("Review modpack installation",body.into())
            },

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
                    body = body.push(subtitle("Files in this installation"));
                    if self.content_review_loading {
                        body = body.push(muted("Resolving the release and all required dependencies… No game files are downloaded during review."));
                    } else if let Some(error) = &self.content_review_error {
                        body = body.push(label("File review could not finish")).push(muted(error))
                            .push(action("Retry file review", Message::ReviewContent(id.clone()), Button::Quiet));
                    } else if let Some(plan) = &self.content_plan {
                        if self.content_plan_matches(id, plan) {
                            let mut bytes = 0u64;
                            let mut unknown_size = false;
                            let mut downloads = 0usize;
                            for file in &plan.files {
                                let status = if file.already_present { "Already present · verified" } else { downloads += 1; "Will download" };
                                if !file.already_present {
                                    if let Some(size) = file.size { bytes = bytes.saturating_add(size); } else { unknown_size = true; }
                                }
                                body = body.push(card(column![
                                    label(format!("{}/{}", file.folder, file.file_name)),
                                    muted(format!("{status} · {}", file.size.map(byte_size).unwrap_or_else(|| "Size not supplied".into()))).size(12),
                                    muted(format!("SHA-1: {}", file.sha1)).size(11),
                                ].spacing(6)));
                            }
                            body = body.push(label(format!("{} files · {} required dependencies · {downloads} downloads", plan.files.len(), plan.files.len().saturating_sub(1))))
                                .push(muted(format!("Download size: {}{}", byte_size(bytes), if unknown_size { " + files with unknown size" } else { "" })))
                                .push(muted("Matching existing files are kept after verification. Different files are never overwritten. The exact plan is rechecked at confirmation; a changed release or dependency requires another review."));
                        } else {
                            body = body.push(muted("The selection changed. Resolve the files again."))
                                .push(action("Refresh file review", Message::ReviewContent(id.clone()), Button::Quiet));
                        }
                    } else {
                        body = body.push(muted("Resolve the required files before confirming."))
                            .push(action("Resolve file review", Message::ReviewContent(id.clone()), Button::Quiet));
                    }
                    body=body.push(if compatible && self.content_plan.as_ref().is_some_and(|plan| self.content_plan_matches(id, plan)) && !self.content_review_loading && self.worker.is_some() && project.project_type!="modpack" && release.project_id==project.project_id && !self.running.contains_key(id) && !self.pending.contains_key(id){action("Confirm installation",Message::InstallContent(id.clone()),Button::Primary)}else{disabled("Waiting for a valid file review")});
                }else{body=body.push(muted("Choose a release and collection before reviewing the installation."));}
                ("Review installation",body.into())
            }
            Dialog::Install => {
                let mut body=column![muted("Choose a stopped collection matching the selected release. Every choice opens a final compatibility/dependency review.")].spacing(12);
                if let Some(project)=&self.project {
                    body=body.push(subtitle(&project.title));
                    if let Some(release)=self.selected_release(){body=body.push(label(format!("Release: {} / {}",release.name,release.version_number)));}
                    if project.project_type=="modpack"{body=body.push(muted("A modpack needs its own collection. Choose a release below or inspect a local archive; the pack's files are installed from its own index, not merged into an existing collection."));body=body.push(action("Review pack releases",Message::ShowReleases,Button::Quiet));body=body.push(action("Configure new pack collection",Message::Open(Dialog::PackSetup),Button::Primary));}
                    let mut found=false;
                    for item in &self.instances {
                        if project.project_type=="modpack" { continue; }
                        if self.running.contains_key(&item.id)||self.pending.contains_key(&item.id){continue;}
                        if let Some(release)=self.selected_release(){if !release.compatible(item,&project.project_type){continue;}}
                        if project.project_type=="mod"&&item.loader==Loader::Vanilla{continue;}
                        found=true;body=body.push(action(&format!("{} · {} / {}",item.name,item.game_version,item.loader),Message::Open(Dialog::InstallReview(item.id.clone())),Button::Primary));
                    }
                    if !found && project.project_type != "modpack" {body=body.push(muted("No compatible stopped collection. Choose another release or create a matching collection."));body=body.push(action("Create a collection",Message::Open(Dialog::Create),Button::Quiet));}
                }
                ("Add to a collection",body.into())
            }
            _=>return None,
        };
        Some(self.panel_shell(heading, body, matches!(dialog, Dialog::Preferences)))
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
                body=body.push(subtitle("Verify your Java account")).push(muted("Request a device code, open Microsoft's verification page, then wait for ownership and player-profile verification. Sign-in needs a registered client supplied through PALANTIR_MSA_CLIENT_ID; without it no code is requested. A completed sign-in is stored on this machine with the permissions of your user profile, and a session review set to the Microsoft account launches with it: when the token has expired the launcher renews it, and when it cannot it refuses the launch rather than quietly playing as your offline identity.")).push(action("Start Microsoft sign-in",Message::AuthStart,Button::Primary));
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
                body=body.push(subtitle(format!("Verified profile: {player}"))).push(muted("Java ownership and profile verification finished. Play still uses the selected offline identity; this verification does not enable authenticated launch or switch the Windows/Xbox account. Session tokens are not displayed or saved.")).push(action("Clear verification",Message::AuthCancel,Button::Quiet));
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
            self.actions(
                ProjectTab::ALL
                    .into_iter()
                    .map(|tab| action(
                        tab.name(),
                        Message::ProjectTab(tab),
                        if self.project_tab == tab {
                            Button::Selected
                        } else {
                            Button::Quiet
                        }
                    ))
                    .collect()
            )
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
