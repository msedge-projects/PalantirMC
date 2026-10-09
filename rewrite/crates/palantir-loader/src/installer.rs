//! Forge-family installer documents: the install profile and the
//! processor pipeline it asks for.
//!
//! Forge and NeoForge ship their install as a jar carrying two documents
//! (fixtures from the vendors' own mavens, see `THIRD_PARTY_NOTICES.md`):
//!
//! - `version.json` -- an ordinary overlay document (`inheritsFrom` the
//!   game), which `install_document` already knows how to install;
//! - `install_profile.json` (`spec: 1`) -- how to *produce* the artifacts
//!   the overlay's libraries name: which toolchain libraries to place,
//!   and a list of **processors** to run as headless Java.
//!
//! A processor is `{sides?, jar, classpath, args, outputs?}`. Its args
//! carry three kinds of token, and every one appears in the real
//! documents:
//!
//! - `[group:artifact:version[:classifier][@ext]]` -- the absolute path of
//!   that artifact under the library root;
//! - `{NAME}` where `NAME` is a `data` key -- that side's value, itself
//!   either a bracketed artifact path or a `'quoted literal'`;
//! - the built-ins `{ROOT}`, `{INSTALLER}`, `{MINECRAFT_JAR}`, `{SIDE}`.
//!
//! Unknown tokens are an error naming the token: a processor run with a
//! literal `{PATCHED}` in it would write a file named `{PATCHED}` and the
//! install would fail much later, somewhere else.
//!
//! `outputs` is a skip receipt: `{marker: "{other marker}"}` pairs the
//! output artifact with its promised hash artifact -- when the file is
//! there and its hash matches, the processor has already run. Running
//! processors is the launch slice's work; this module is the plan.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use palantir_core::maven::MavenCoord;
use palantir_core::version::Library;
use serde::Deserialize;

use crate::error::{Error, Result};

/// One installer's `install_profile.json` (`spec: 1`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InstallProfile {
    pub spec: u32,
    /// The game version this installs over.
    pub minecraft: String,
    /// The id the installed version takes.
    pub version: String,
    /// Where the overlay document lives inside the installer jar.
    #[serde(default)]
    pub json: Option<String>,
    /// Toolchain libraries to place before running anything.
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub processors: Vec<Processor>,
    /// Marker name -> per-side value.
    #[serde(default)]
    pub data: BTreeMap<String, SideValue>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// A `data` value: one per side. Both vendors always give both, but a
/// missing side is "this marker does not exist for me", not a parse error.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SideValue {
    #[serde(default)]
    pub client: Option<String>,
    #[serde(default)]
    pub server: Option<String>,
}

impl SideValue {
    fn for_side(&self, side: Side) -> Option<&str> {
        match side {
            Side::Client => self.client.as_deref(),
            Side::Server => self.server.as_deref(),
        }
    }
}

/// One processor to run as headless Java.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Processor {
    /// Omitted means both sides.
    #[serde(default)]
    pub sides: Option<Vec<String>>,
    /// The tool to run: a Maven coordinate.
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// Skip receipt: output artifact -> promised hash artifact, both as
    /// tokens to expand.
    #[serde(default)]
    pub outputs: Option<BTreeMap<String, String>>,
}

/// Which side of the install is being made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Client,
    Server,
}

impl Side {
    fn name(self) -> &'static str {
        match self {
            Side::Client => "client",
            Side::Server => "server",
        }
    }
}

/// What the built-in tokens mean at this install: where everything is.
#[derive(Debug, Clone, Copy)]
pub struct ProcessorContext<'a> {
    /// `{ROOT}`: the install's root, where produced artifacts land.
    pub root: &'a Path,
    /// The library root, under which bracketed artifacts resolve.
    pub library_dir: &'a Path,
    /// `{MINECRAFT_JAR}`: the game jar the pipeline patches.
    pub minecraft_jar: &'a Path,
    /// `{INSTALLER}`: the installer jar the documents came out of.
    pub installer: &'a Path,
    pub side: Side,
}

/// One processor with its arguments fully expanded: what headless Java
/// will be asked to run. `jar` and `classpath` stay Maven coordinates --
/// they name files under the library root to assemble the classpath from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedProcessor {
    pub jar: String,
    pub classpath: Vec<String>,
    pub args: Vec<String>,
    /// Skip receipts: (produced artifact, its promised SHA-1).
    pub outputs: Vec<(PathBuf, String)>,
}

impl InstallProfile {
    /// Also BOM-tolerant: these documents ship inside vendor jars.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        serde_json::from_str(text).map_err(|source| Error::Invalid {
            what: "install profile",
            why: source.to_string(),
        })
    }

    /// Does this processor run for `side`? Omitted sides mean both.
    pub fn runs_on(processor: &Processor, side: Side) -> bool {
        match &processor.sides {
            None => true,
            Some(sides) => sides.iter().any(|s| s == side.name()),
        }
    }
}

/// Plan every processor that runs for `side`, in document order, with all
/// tokens expanded.
pub fn plan_processors(
    profile: &InstallProfile,
    context: &ProcessorContext<'_>,
) -> Result<Vec<PlannedProcessor>> {
    let mut plan = Vec::new();
    for processor in &profile.processors {
        if !InstallProfile::runs_on(processor, context.side) {
            continue;
        }
        let args = processor
            .args
            .iter()
            .map(|arg| expand(arg, profile, context))
            .collect::<Result<Vec<_>>>()?;
        let outputs = match &processor.outputs {
            None => Vec::new(),
            Some(outputs) => outputs
                .iter()
                .map(|(file, hash)| {
                    Ok((
                        PathBuf::from(expand(file, profile, context)?),
                        expand(hash, profile, context)?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?,
        };
        plan.push(PlannedProcessor {
            jar: processor.jar.clone(),
            classpath: processor.classpath.clone(),
            args,
            outputs,
        });
    }
    Ok(plan)
}

/// Expand one argument string: brackets are artifact paths, braces are
/// data markers or built-ins. Tokens are never nested in the real
/// documents; anything unrecognized is an error naming it.
fn expand(text: &str, profile: &InstallProfile, context: &ProcessorContext<'_>) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find(['{', '[']) {
        let token = rest.as_bytes()[open];
        let close = if token == b'{' { '}' } else { ']' };
        let Some(end) = rest[open..].find(close) else {
            return Err(Error::Invalid {
                what: "install profile",
                why: format!("{text:?} has an unterminated token"),
            });
        };
        let inner = &rest[open + 1..open + end];
        out.push_str(&rest[..open]);
        out.push_str(&expand_token(inner, token == b'[', profile, context)?);
        rest = &rest[open + end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// One token's value. `bracketed` selects the artifact-path reading.
fn expand_token(
    inner: &str,
    bracketed: bool,
    profile: &InstallProfile,
    context: &ProcessorContext<'_>,
) -> Result<String> {
    if bracketed {
        return artifact_path(inner, context);
    }
    let value = match inner {
        "ROOT" => return Ok(context.root.display().to_string()),
        "INSTALLER" => return Ok(context.installer.display().to_string()),
        "MINECRAFT_JAR" => return Ok(context.minecraft_jar.display().to_string()),
        "SIDE" => return Ok(context.side.name().to_string()),
        _ => {
            let marker = profile.data.get(inner).ok_or_else(|| Error::Invalid {
                what: "install profile",
                why: format!("{{{inner}}} is not a marker this document defines"),
            })?;
            marker
                .for_side(context.side)
                .ok_or_else(|| Error::Invalid {
                    what: "install profile",
                    why: format!("{{{inner}}} has no {} value", context.side.name()),
                })?
        }
    };
    data_value(value, context)
}

/// A `data` value: a bracketed artifact path, a `'quoted literal'`, or a
/// plain literal.
fn data_value(value: &str, context: &ProcessorContext<'_>) -> Result<String> {
    if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        return artifact_path(inner, context);
    }
    if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        return Ok(inner.to_string());
    }
    Ok(value.to_string())
}

/// `group:artifact:version[:classifier][@ext]` as the absolute path of the
/// artifact under the library root.
fn artifact_path(coord: &str, context: &ProcessorContext<'_>) -> Result<String> {
    let parsed = MavenCoord::parse(coord)?;
    // Join each segment rather than the whole `rel_path` at once: that
    // string is URL-shaped (`/`-separated, because the same layout
    // builds download URLs), and Windows joins it with one native
    // separator only at the boundary -- the argument would come out
    // mixed (`libraries\net/minecraft/...`) and any consumer comparing
    // path text would see two different files.
    let path = parsed
        .rel_path()
        .split('/')
        .fold(context.library_dir.to_path_buf(), |dir, segment| {
            dir.join(segment)
        });
    Ok(path.display().to_string())
}
