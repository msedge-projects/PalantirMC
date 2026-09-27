//! The engine: one client, one ceiling, one policy for what to do when a
//! request fails.
//!
//! Stage 4 of the rewrite spec. Everything that touches the network goes through
//! here, and the reason it is a module rather than a habit is that the launcher
//! has had the alternative: a `Client` built per call, a thread count that meant
//! something different in each phase, a download that started again from zero,
//! retries scattered across call sites that each decided differently, and no way
//! at all to stop a transfer that was already running.
//!
//! The pieces, and what each of them is for:
//!
//! | Module | What it decides |
//! | --- | --- |
//! | [`limit`] | How many requests may be in flight at once, for the whole process |
//! | [`retry`] | Which failures get another attempt, and how long before it |
//! | [`cancel`] | How a running transfer is stopped, mid-body, in milliseconds |
//! | [`request`] | What a request is, and the seam every rule above is tested through |
//! | [`cache`] | A TTL on every metadata answer, and a revalidation instead of a re-download |
//! | [`content`] | Files named by their own digest: what is already here is never fetched twice |
//! | [`http`] | The one `reqwest` client, the `Range` header, and the honest answer when a range is ignored |
//! | [`download`] | One file: resumed if it can be, restarted if it must be, verified before it is done |
//! | [`schedule`] | A queue of files over a few workers, where every job reports and any one can be stopped |
//! | [`forge`] | Forge's and NeoForge's installer jars: the launch profile inside them, and the processors that install them |
//! | [`piston`] | Mojang's own version manifest and version files, checked against the digests it publishes |
//! | [`modrinth`] | Modrinth's API over the same cache, with a TTL short enough for a search |
//! | [`loaders`] | What Fabric, Quilt, NeoForge and Forge each publish as their own build list, read from the loader that published it rather than from the other launcher's mirror of it |
//!
//! ## Why the rules are here and not at the call sites
//!
//! A retry policy applied per call site is a policy that differs per call site,
//! and the differences are invisible until a service has a bad afternoon. The
//! same argument holds for the ceiling: the only number a service can be fair
//! about is a global one. So the engine owns both, and the call sites pass the
//! things they know (a URL, a destination, a digest) rather than the things the
//! engine decides (how many tries, how long to wait, how many at once).
//!
//! ## Testing without a network
//!
//! `reqwest` cannot be reached from a unit test, so every rule that has to hold
//! is stated against [`request::Fetch`] and exercised with
//! [`request::MapFetch`], which is a server that can be told to honour an offset
//! or ignore it, to fail twice with a 503, and to cancel a transfer three chunks
//! in. That last one is the reason the double exists at all: a cancellation that
//! lands *inside* a body is the case a pre-set flag cannot reach, and it is the
//! case the loop's check is for.
//!
//! What the double cannot say is that `reqwest` behaves the way it is assumed
//! to -- that a `Range` request really comes back 206, that `read` really
//! returns zero at the end. That is `tests/live.rs`'s job, and it is a
//! `#[ignore]`d test by design: a network that is down is not a passing test.

pub mod cache;
pub mod cancel;
pub mod content;
pub mod download;
pub mod forge;
pub mod http;
pub mod limit;
pub mod loaders;
pub mod modrinth;
pub mod piston;
pub mod request;
pub mod retry;
pub mod schedule;

pub use cache::{Cached, MetadataCache, DEFAULT_TTL, IMMUTABLE_TTL};
pub use cancel::Cancel;
pub use content::{ContentStore, Digest, Stored};
pub use download::{fetch_to_file, Download, Downloaded};
pub use forge::{
    artifact_path, component_uid, find_java, install, installer_url, maven_roots, maven_sha1,
    parse_installer, translate_profile, DataValue, InstallCtx, InstallSpec, InstalledProcessor,
    InstallerMeta, ParsedInstaller, Processor, CENTRAL_MAVEN, CLIENT_SIDE, FORGE_MAVEN,
    MOJANG_LIBRARIES, NEOFORGE_MAVEN,
};
pub use http::{HttpPool, DEFAULT_LIMIT, DEFAULT_TIMEOUT, USER_AGENT};
pub use limit::{Limit, Permit};
pub use loaders::{default_build, Build, Loader, LoaderMeta, MAX_BUILDS};
pub use modrinth::{ModrinthApi, Search, SEARCH_TTL};
pub use piston::{Manifest, ManifestVersion, PistonMeta, PISTON_MANIFEST_URL};
pub use request::{Fetch, Outcome, Request, Response};
pub use retry::{is_retryable, Backoff};
pub use schedule::{next_event, Event, Job, JobId, Scheduler, DEFAULT_WORKERS};
