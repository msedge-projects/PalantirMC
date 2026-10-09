//! Turning a resolved version document into a runnable process description.
//!
//! The loader crate owns the zip extraction and the final `LaunchPlan`
//! assembly; this module owns the metadata-shaped pieces: which argument list
//! survived a rules filter, and which download is the client jar on this
//! platform.

use super::rules::Platform;
use super::version::{Arguments, Download, Version};

/// The argument lists a version actually uses on this platform: any entry
/// gated by a rule that does not match this platform is dropped, and the
/// `default-user-jvm` list is folded into `jvm` (it is advertised as a
/// default the launcher may override, and on this platform it is already
/// applicable).
pub fn resolve_args(version: &Version, platform: &Platform) -> Arguments {
    let arguments = match &version.arguments {
        Some(args) => args,
        None => return Arguments::default(),
    };

    let default_user_jvm = arguments
        .default_user_jvm
        .as_ref()
        .map(|list| resolve_list(list, platform))
        .filter(|list| !list.is_empty());

    let jvm = arguments
        .jvm
        .as_ref()
        .map(|list| resolve_list(list, platform))
        .filter(|list| !list.is_empty());

    let game = arguments
        .game
        .as_ref()
        .map(|list| resolve_list(list, platform))
        .filter(|list| !list.is_empty());

    let mut merged = Arguments {
        default_user_jvm: default_user_jvm.clone(),
        jvm: jvm.clone(),
        game: game.clone(),
        extra: arguments.extra.clone(),
    };

    // Fold the applicable default-user-jvm entries into `jvm` so a caller
    // sees one JVM list. The format's `default-user-jvm` is advice a launcher
    // may override; here, what applies to this platform is already in.
    if let Some(defaults) = default_user_jvm {
        let mut jvm = jvm.unwrap_or_default();
        jvm.extend(defaults);
        merged.jvm = if jvm.is_empty() { None } else { Some(jvm) };
    }

    merged
}

/// One argument list after the rules filter: a rule that does not match this
/// platform drops its entry; a list with no rules passes through.
pub(crate) fn resolve_list(
    list: &[super::version::Argument],
    platform: &Platform,
) -> Vec<super::version::Argument> {
    list.iter()
        .filter(|arg| arg.applies_to(platform))
        .cloned()
        .collect()
}

impl super::version::Argument {
    pub fn applies_to(&self, platform: &Platform) -> bool {
        match self {
            super::version::Argument::Plain(_) => true,
            super::version::Argument::Conditional { rules, .. } => {
                rules.is_empty() || super::rules::rules_allow(rules, platform).unwrap_or(false)
            }
        }
    }
}

/// The client jar this version wants on this platform. A version either
/// names per-platform downloads or (older) names only a `client` download;
/// in either case there is always a client jar for a runnable version.
pub fn client_download(version: &Version, platform: &Platform) -> Option<Download> {
    version
        .downloads
        .get(platform.os.mojang_name()?)
        .cloned()
        .or_else(|| version.downloads.get("client").cloned())
}
