//! Required content dependencies: the projects a version needs installed
//! beside it.
//!
//! A catalogue describes a version's dependencies as ids plus a
//! `dependency_type`. Only `required` entries must land, and they recurse
//! -- a library's own library is still needed -- so the work is a graph
//! walk, and the walk is where this launcher can go quietly wrong. It is
//! pure on purpose: the caller injects one lookup closure, the tests inject
//! a map, and nothing here touches a socket.
//!
//! Two rules run through it, both aimed at the failure this tree keeps
//! refusing, a *silent wrong install*:
//!
//! - a required dependency whose catalogue lists no build for this game and
//!   loader is an error naming it, never a quiet skip;
//! - a required dependency that names neither a project nor a version is an
//!   error too -- nothing can be installed for it, and an install missing
//!   what it asked for fails much later, somewhere else entirely.

use std::collections::{BTreeSet, VecDeque};

use serde_json::Value;

use crate::error::{Error, Result};

/// How deep the walk may go before it is called a loop.
///
/// Real chains nest three or four deep (a mod, its library, that library's
/// config library). Past this, a longer chain is far more likely to be
/// metadata that does not terminate than a real requirement, and the cost of
/// being wrong is a hang in a downloader rather than an error someone reads.
pub const MAX_DEPTH: usize = 8;

/// One entry of a version's `dependencies` array.
///
/// The fields mirror the catalogue's own names so a document can be read
/// straight into it; `dependency_type` is the only one always present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub dependency_type: String,
    /// The project the dependency names, when it names one. A dependency
    /// may instead name only a version.
    pub project_id: Option<String>,
    /// The exact version it pins, when it pins one.
    pub version_id: Option<String>,
    /// The author's file name, when they supplied one.
    pub file_name: Option<String>,
}

impl Dependency {
    /// Read one entry tolerantly. Catalogues omit keys they have no value
    /// for, so a missing id is data, not damage -- but an entry with no
    /// `dependency_type` states no requirement and is not a dependency at
    /// all.
    pub fn from_value(value: &Value) -> Option<Self> {
        let dependency_type = value.get("dependency_type")?.as_str()?.to_string();
        let text = |key: &str| {
            value
                .get(key)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        };
        Some(Self {
            dependency_type,
            project_id: text("project_id"),
            version_id: text("version_id"),
            file_name: text("file_name"),
        })
    }
}

/// A dependency that must be installed, reduced to what resolving needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// `None` when the dependency named only a version.
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

impl Requirement {
    /// The id the caller's lookup is asked for: the project when the
    /// dependency names one, otherwise the version it pinned (a catalogue
    /// answers that with the one version document it names).
    pub fn lookup_key(&self) -> Option<&str> {
        self.project_id.as_deref().or(self.version_id.as_deref())
    }

    /// What this requirement is called in an error.
    fn label(&self) -> String {
        self.project_id
            .clone()
            .or_else(|| self.version_id.clone())
            .unwrap_or_else(|| "an unnamed dependency".to_string())
    }
}

/// One file the plan will fetch, flattened out of the catalogue's version
/// document so a caller needs no second parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFile {
    pub project_id: String,
    pub version_id: String,
    pub file_name: String,
    /// The first download URL, or `None` when the record names none.
    pub url: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// The `required` dependencies of one version document, in the order the
/// document lists them.
///
/// Optional and incompatible entries are deliberately not here: what a pack
/// *may* have is the user's choice and belongs to the importer's selection,
/// and what it must *not* have is a conflict to report, not a file to fetch.
///
/// `project_id` is the project this document belongs to. A dependency
/// naming that same project is not a requirement -- the version is already
/// being installed -- and dropping it here is the same deduplication the
/// walk applies to every other already-planned project.
pub fn required(project_id: &str, version: &Value) -> Vec<Requirement> {
    version
        .get("dependencies")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Dependency::from_value)
                .filter(|dependency| dependency.dependency_type == "required")
                .filter(|dependency| dependency.project_id.as_deref() != Some(project_id))
                .map(|dependency| Requirement {
                    project_id: dependency.project_id,
                    version_id: dependency.version_id,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Pick the build to install for one requirement.
///
/// A pinned `version_id` wins outright -- the author pinned it for a reason,
/// and re-deciding that here would be substituting our judgement for theirs.
/// Otherwise the **first** candidate the catalogue returned that lists both
/// the game version and the loader is chosen: the catalogue's order is its
/// own relevance order, so re-sorting it would do the same substitution.
///
/// No compatible candidate is an error naming the project, which is the
/// whole point of resolving rather than guessing.
pub fn choose_version(
    candidates: &[Value],
    requirement: &Requirement,
    game_version: &str,
    loader: &str,
) -> Result<Value> {
    let lists = |candidate: &Value, key: &str, wanted: &str| {
        candidate
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(wanted)))
    };
    let chosen = match &requirement.version_id {
        Some(version_id) => candidates
            .iter()
            .find(|candidate| {
                candidate.get("id").and_then(Value::as_str) == Some(version_id.as_str())
            })
            .ok_or_else(|| Error::Invalid {
                what: "dependency",
                why: format!(
                    "{} pins {version_id}, which the catalogue did not list",
                    requirement.label()
                ),
            })?,
        None => candidates
            .iter()
            .find(|candidate| {
                lists(candidate, "game_versions", game_version)
                    && lists(candidate, "loaders", loader)
            })
            .ok_or_else(|| Error::Invalid {
                what: "dependency",
                why: format!(
                    "{} has no build for Minecraft {game_version} on {loader}",
                    requirement.label()
                ),
            })?,
    };
    Ok(chosen.clone())
}

/// The full plan of files a version's required dependencies need.
///
/// `lookup` is the only I/O and is injected: it answers an id -- a project
/// id, or a version id when that is all a dependency named -- with the
/// candidate version documents for that id. The walk is breadth-first in
/// document order, so the result is deterministic; a project reached twice
/// is planned once (first choice wins), while a project reached *again
/// along its own path* is refused as a cycle rather than walked forever.
pub fn resolve_plan(
    root: &Value,
    lookup: &mut dyn FnMut(&str) -> Result<Vec<Value>>,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ResolvedFile>> {
    let root_project = root
        .get("project_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let root_label = if root_project.is_empty() {
        "the root version".to_string()
    } else {
        root_project.clone()
    };

    let mut planned: Vec<ResolvedFile> = Vec::new();
    // Projects already planned: a second path to one of them is a diamond,
    // not a problem, and adding it twice would collide on its file.
    let mut planned_projects: BTreeSet<String> = BTreeSet::new();
    if !root_project.is_empty() {
        planned_projects.insert(root_project.clone());
    }

    let mut queue: VecDeque<(usize, Requirement, Vec<String>)> = VecDeque::new();
    for requirement in required(&root_project, root) {
        queue.push_back((1, requirement, vec![root_label.clone()]));
    }

    while let Some((depth, requirement, path)) = queue.pop_front() {
        let Some(key) = requirement.lookup_key().map(str::to_string) else {
            return Err(Error::Invalid {
                what: "dependency",
                why: format!(
                    "{} requires a dependency that names neither a project nor a version",
                    path.join(" -> ")
                ),
            });
        };
        let identity = requirement
            .project_id
            .clone()
            .unwrap_or_else(|| key.clone());

        // The cycle check must precede the deduplication: a project that is
        // both already planned and on this path is a loop, and reporting it
        // as a tidy skip would hide metadata that cannot be satisfied.
        if path.contains(&identity) {
            let mut cycle = path.clone();
            cycle.push(identity);
            return Err(Error::Invalid {
                what: "dependency",
                why: format!("dependency cycle: {}", cycle.join(" -> ")),
            });
        }
        if depth > MAX_DEPTH {
            return Err(Error::Invalid {
                what: "dependency",
                why: format!(
                    "{identity} is {depth} dependencies deep, past the {MAX_DEPTH} this walk allows"
                ),
            });
        }
        if planned_projects.contains(&identity) {
            continue;
        }

        let candidates = lookup(&key)?;
        let chosen = choose_version(&candidates, &requirement, game_version, loader)?;
        let file = resolved_from(&identity, &requirement, &chosen)?;
        planned_projects.insert(identity.clone());
        let next_path = {
            let mut next = path.clone();
            next.push(identity);
            next
        };
        for child in required(&file.project_id, &chosen) {
            queue.push_back((depth + 1, child, next_path.clone()));
        }
        planned.push(file);
    }
    Ok(planned)
}

/// Flatten a chosen version document into the one file it contributes.
fn resolved_from(
    identity: &str,
    requirement: &Requirement,
    chosen: &Value,
) -> Result<ResolvedFile> {
    // A pinned version is the author's decision, so it outranks a document
    // that omits its own id rather than being silently replaced by a blank.
    let version_id = chosen
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| requirement.version_id.clone())
        .unwrap_or_default();
    let files = chosen.get("files").and_then(Value::as_array);
    // The primary file is the one the author points at; when the record
    // marks none, the first is what the catalogue itself would offer.
    let primary = files.and_then(|files| {
        files
            .iter()
            .find(|file| file.get("primary").and_then(Value::as_bool) == Some(true))
            .or_else(|| files.first())
    });
    let Some(file) = primary else {
        return Err(Error::Invalid {
            what: "dependency",
            why: format!("{identity} {version_id} lists no file to install"),
        });
    };
    let file_name = file
        .get("filename")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::Invalid {
            what: "dependency",
            why: format!("{identity} {version_id} names no file name"),
        })?;
    Ok(ResolvedFile {
        project_id: chosen
            .get("project_id")
            .and_then(Value::as_str)
            .unwrap_or(identity)
            .to_string(),
        version_id,
        file_name,
        url: file.get("url").and_then(Value::as_str).map(str::to_string),
        sha1: file
            .get("hashes")
            .and_then(|hashes| hashes.get("sha1"))
            .and_then(Value::as_str)
            .map(str::to_string),
        size: file.get("size").and_then(Value::as_u64),
    })
}
