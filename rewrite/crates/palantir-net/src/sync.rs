//! Everything one version needs on disk, end to end.
//!
//! A version's needs are enumerable from its metadata and the platform:
//! the client jar, each allowed library and its natives jar, the asset
//! index, and every asset object the index names. This module turns that
//! list into scheduler jobs that land each byte string in the content
//! store (verified, deduplicated) and materialize it at its layout path.
//!
//! Jobs are deduplicated by content hash before they are scheduled. One
//! hash can appear under many names -- an asset index routinely names the
//! same object twice -- and two workers fetching one hash would race on a
//! single part file and a single store slot. One fetch, many names.
//!
//! What it deliberately does not do: extract natives jars (that is launch
//! preparation, phase 3), fetch a Java runtime (also phase 3), or fetch the
//! logging configuration (a launch argument, phase 5). `sync_version`
//! downloads a *game version*.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::client::Http;
use crate::download::{self, DownloadOptions, Transfer};
use crate::error::{Error, Result};
use crate::scheduler::Scheduler;
use crate::store::ContentStore;
use palantir_core::assets::{AssetDestination, AssetIndex, OBJECT_URL_BASE};
use palantir_core::library::DownloadRef;
use palantir_core::paths::DataRoot;
use palantir_core::rules::Platform;
use palantir_core::version::Version;

/// The pieces a sync moves, borrowed and shared across workers.
pub struct Syncer<'a> {
    pub http: &'a Http,
    pub scheduler: &'a Scheduler,
    pub store: &'a ContentStore,
    pub root: &'a DataRoot,
    pub options: DownloadOptions,
    /// Where asset objects are served from: the shared content service by
    /// default. Mirrors keep its layout and change only the base.
    pub asset_url_base: String,
}

/// What a sync did, in counts. `fetched` and `reused` count files (layout
/// paths); `resumed` and `bytes` count the transfers that moved them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Files landed by fetching their bytes.
    pub fetched: usize,
    /// Files answered from the content store without a request.
    pub reused: usize,
    /// Transfers whose winning attempt continued from a partial file.
    pub resumed: usize,
    /// Bytes the winning attempts wrote to the store.
    pub bytes: u64,
    pub libraries: usize,
    pub natives: usize,
    pub asset_index: usize,
    pub assets: usize,
}

/// One fetch, with every layout path its bytes land at.
struct Job {
    url: String,
    /// Never empty: the first is where a hash-less fetch writes; the rest
    /// are materialized copies.
    dests: Vec<PathBuf>,
    sha1: Option<String>,
    size: Option<u64>,
    counted: Counted,
}

/// Which counter a landed file moves.
#[derive(Clone, Copy)]
enum Counted {
    Client,
    Library,
    Natives,
    AssetIndex,
    Asset,
}

#[derive(Default)]
struct Tally {
    report: SyncReport,
    client_transfer: Option<Transfer>,
}

impl<'a> Syncer<'a> {
    pub fn new(
        http: &'a Http,
        scheduler: &'a Scheduler,
        store: &'a ContentStore,
        root: &'a DataRoot,
        options: DownloadOptions,
    ) -> Self {
        Self {
            http,
            scheduler,
            store,
            root,
            options,
            asset_url_base: OBJECT_URL_BASE.to_string(),
        }
    }

    /// Land everything `version` needs on `platform`.
    ///
    /// `game_dir` is required only by indexes that write into a game
    /// directory (`map_to_resources`); the common object-store layout lives
    /// under the data root and ignores it.
    pub fn sync_version(
        &self,
        version: &Version,
        platform: &Platform,
        game_dir: Option<&Path>,
    ) -> Result<(SyncReport, Option<Transfer>)> {
        let mut jobs = Vec::new();

        // The client jar: the one file named by the version itself.
        if let Some(client) = version.downloads.get("client") {
            jobs.push(Job {
                url: required_url(&client.url, &version.id)?,
                dests: vec![self.root.version_jar(&version.id)],
                sha1: client.sha1.clone(),
                size: client.size,
                counted: Counted::Client,
            });
        }

        // Libraries this platform actually gets, with their natives jars.
        for library in &version.libraries {
            let Some(resolved) = library.resolve(platform)? else {
                continue;
            };
            let owner = resolved.coord.group.clone();
            if let Some(artifact) = resolved.artifact.clone() {
                jobs.push(self.file_job(&artifact, &owner, Counted::Library)?);
            }
            if let Some(natives) = resolved.natives.clone() {
                jobs.push(self.file_job(&natives, &owner, Counted::Natives)?);
            }
        }

        // The asset index decides what the rest looks like; it must land
        // before its objects are known, so it syncs in its own pass.
        let mut tally = Tally::default();
        self.run_jobs(jobs, &mut tally)?;

        if let Some(index_ref) = &version.asset_index {
            let index_dest = self.root.asset_index_file(&index_ref.id);
            let index_job = Job {
                url: required_url(&index_ref.url, &index_ref.id)?,
                dests: vec![index_dest.clone()],
                sha1: index_ref.sha1.clone(),
                size: index_ref.size,
                counted: Counted::AssetIndex,
            };
            self.run_jobs(vec![index_job], &mut tally)?;

            let text = std::fs::read_to_string(&index_dest).map_err(|source| Error::Io {
                path: index_dest,
                source,
            })?;
            let index = AssetIndex::parse(&text)?;
            let asset_jobs = self.asset_jobs(&index, &index_ref.id, game_dir)?;
            self.run_jobs(asset_jobs, &mut tally)?;
        }

        Ok((tally.report, tally.client_transfer))
    }

    /// A library-tree file as a job: its Maven path under the data root.
    fn file_job(&self, download: &DownloadRef, owner: &str, counted: Counted) -> Result<Job> {
        Ok(Job {
            url: required_url(&download.url, owner)?,
            dests: vec![self.root.library_file(&download.rel_path)?],
            sha1: download.sha1.clone(),
            size: download.size,
            counted,
        })
    }

    /// The index's objects as jobs, one per content hash with every name it
    /// answers to, placed per the layout the index asks for.
    fn asset_jobs(
        &self,
        index: &AssetIndex,
        index_id: &str,
        game_dir: Option<&Path>,
    ) -> Result<Vec<Job>> {
        let base: PathBuf = match index.destination() {
            AssetDestination::Resources => {
                game_dir
                    .map(Path::to_path_buf)
                    .ok_or_else(|| Error::Invalid {
                        what: "sync",
                        why: "this asset index writes into a game directory, \
                         and none was given"
                            .to_string(),
                    })?
            }
            AssetDestination::ObjectStore | AssetDestination::Virtual => self.root.assets_dir(),
        };

        let mut by_hash: BTreeMap<String, (String, u64, Vec<PathBuf>)> = BTreeMap::new();
        for (name, object) in &index.objects {
            let url = object.url_at(&self.asset_url_base)?;
            let dest = base.join(index.rel_path(index_id, name)?);
            let entry =
                by_hash
                    .entry(object.hash.clone())
                    .or_insert((url, object.size, Vec::new()));
            entry.2.push(dest);
        }
        Ok(by_hash
            .into_iter()
            .map(|(hash, (url, size, dests))| Job {
                url,
                dests,
                sha1: Some(hash),
                size: Some(size),
                counted: Counted::Asset,
            })
            .collect())
    }

    /// Land every job under the concurrency ceiling, fail on the first
    /// error in job order (workers may have finished more; nothing
    /// unverified is kept either way).
    fn run_jobs(&self, jobs: Vec<Job>, tally: &mut Tally) -> Result<()> {
        let outcomes = self.scheduler.run(jobs, |job| self.land(job));
        for outcome in outcomes {
            let (landed, transfer, files) = outcome?;
            match landed {
                Counted::Client => tally.client_transfer = Some(transfer),
                Counted::Library => tally.report.libraries += files,
                Counted::Natives => tally.report.natives += files,
                Counted::AssetIndex => tally.report.asset_index += files,
                Counted::Asset => tally.report.assets += files,
            }
            if transfer.already_present {
                tally.report.reused += files;
            } else {
                tally.report.fetched += files;
                tally.report.bytes += transfer.bytes;
                if transfer.resumed_from > 0 {
                    tally.report.resumed += 1;
                }
            }
        }
        Ok(())
    }

    /// Land one job: store by hash when the metadata has one (dedup and a
    /// verified door), straight to its destination when it does not. Either
    /// way every name in `dests` ends up holding the bytes.
    fn land(&self, job: Job) -> Result<(Counted, Transfer, usize)> {
        let files = job.dests.len();
        let primary = job.dests.first().ok_or_else(|| Error::Invalid {
            what: "sync job",
            why: "a job with no destination".to_string(),
        })?;
        let transfer = match &job.sha1 {
            Some(sha1) => {
                if self.store.contains(sha1) {
                    Transfer {
                        resumed_from: 0,
                        bytes: 0,
                        attempts: 0,
                        already_present: true,
                    }
                } else {
                    let store_path = self.store.path(sha1)?;
                    download::download(
                        self.http,
                        &job.url,
                        &store_path,
                        Some(sha1),
                        job.size,
                        &self.options,
                    )?
                }
            }
            None => {
                download::download(self.http, &job.url, primary, None, job.size, &self.options)?
            }
        };
        for dest in &job.dests {
            match &job.sha1 {
                Some(sha1) => self.store.materialize(sha1, dest)?,
                None if dest != primary => {
                    std::fs::copy(primary, dest).map_err(|source| Error::Io {
                        path: dest.clone(),
                        source,
                    })?;
                }
                None => {}
            }
        }
        Ok((job.counted, transfer, files))
    }
}

fn required_url(url: &Option<String>, what: &str) -> Result<String> {
    url.clone().ok_or_else(|| Error::Invalid {
        what: "metadata download",
        why: format!("{what} names a file but no URL"),
    })
}
