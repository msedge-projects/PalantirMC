//! The game engine's shim: re-exports [`sift_game::launch`] and supplies the two
//! facts a launch needs from this shell.
//!
//! `launch.rs` used to hold the whole engine -- the install plan, the auth
//! renewal, the worker, the run log -- because the iced shell was the only
//! caller. It now lives in `sift-game` so the egui shell can reach it too, and
//! this module exists to keep `crate::launch::…` working unchanged for
//! `shell.rs`, `store.rs` and the instance-settings page.
//!
//! Two things could not move, and both are supplied here:
//!
//! * **The preferences conversion.** `LaunchDefaults::from_prefs` and
//!   `JavaPrefs::from_prefs` were methods in the engine. `Prefs` is this
//!   launcher's settings file (theme, language, and the Java tab), and the
//!   engine crate deliberately has no UI dependencies, so the conversion is
//!   [`defaults_from_prefs`] and [`java_prefs`] here. The engine's
//!   [`LaunchDefaults::new`] takes the three plain values. Behaviour is
//!   unchanged: the values are still read at the moment a run is requested.
//! * **The monitor query.** A maximized instance is passed the primary monitor's
//!   work area, which is a Win32 call (`native::primary_work_area`). The engine
//!   takes it as a `WorkAreaProbe` function pointer; [`run_launch_worker`] here
//!   passes [`primary_work_area`] so `shell.rs` keeps calling the same
//!   three-argument worker it always did.
//!
//! The machine-awake guard for a launch is not here either: the engine is safe
//! Rust, so `shell.rs` holds it around the thread that calls this worker. See
//! `awake.rs`.

use futures::channel::mpsc::Sender;
use sift_game::java_runtime::JavaPrefs;

pub use sift_game::launch::*;

/// The launch defaults this shell's preferences hold.
///
/// Read at the moment a run is requested, because the worker has no business
/// going back to a preferences file -- or to another launcher's -- to find out
/// what it was told to use.
pub fn defaults_from_prefs(prefs: &crate::prefs::Prefs) -> LaunchDefaults {
    LaunchDefaults::new(
        java_prefs(prefs),
        i64::from(prefs.min_mem_mib()),
        i64::from(prefs.max_mem_mib()),
    )
}

/// The Java tab as the engine's [`JavaPrefs`].
///
/// A key that is not a number and a path that is blank are both dropped: the
/// first is a file from a newer build or a hand-edit, and the second is how the
/// tab records "nothing chosen" -- neither is a Java to run with.
pub fn java_prefs(prefs: &crate::prefs::Prefs) -> JavaPrefs {
    let by_major = prefs
        .java_paths
        .iter()
        .filter_map(|(major, path)| {
            let major = major.trim().parse::<i64>().ok()?;
            let path = path.trim();
            (!path.is_empty()).then(|| (major, path.to_string()))
        })
        .collect();
    JavaPrefs {
        default_path: prefs
            .default_java_path
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_string(),
        by_major,
    }
}

/// The primary monitor's work area in physical pixels, for the engine's
/// maximized-window translation.
///
/// The Win32 call itself stays in [`crate::native`]; this is only the shape the
/// engine asks for. It is passed as a function pointer and therefore queried
/// only when an instance actually asks to maximize.
fn primary_work_area() -> Option<(i64, i64)> {
    crate::native::primary_work_area().map(|area| (area.width as i64, area.height as i64))
}

/// Run one launch in a background thread, with this shell's monitor query.
///
/// The engine's worker takes the probe as a fourth argument; this wrapper is the
/// three-argument form `shell.rs` has always called, so the shell does not have
/// to know where the engine keeps its window-system seam.
pub fn run_launch_worker(params: LaunchParams, slot: ChildSlot, sender: Sender<LaunchEvent>) {
    sift_game::launch::run_launch_worker(params, slot, sender, Some(primary_work_area));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Synced settings pane is the answer for the heap, not Prism's file.
    ///
    /// A machine where both exist is the case that matters: the pane shows one
    /// number, so the JVM has to get that number, or the pane is wrong about the
    /// machine it describes.
    #[test]
    fn the_defaults_are_read_from_the_preferences_rather_than_left_at_zero() {
        let mut prefs = crate::prefs::Prefs::default();
        prefs.default_min_mem_mib = Some(1024);
        prefs.default_max_mem_mib = Some(8192);
        prefs.default_java_path = Some("C:/jdk21/bin/javaw.exe".to_string());
        let defaults = defaults_from_prefs(&prefs);
        assert_eq!((defaults.min_mem_mib, defaults.max_mem_mib), (1024, 8192));
        assert_eq!(
            defaults.java.default_binary(),
            Some("C:/jdk21/bin/javaw.exe")
        );

        // Nothing set anywhere: the shipped numbers, because a heap of zero
        // megabytes is a launch that dies in the JVM rather than a default.
        let shipping = defaults_from_prefs(&crate::prefs::Prefs::default());
        assert_eq!(
            (shipping.min_mem_mib, shipping.max_mem_mib),
            (
                i64::from(crate::prefs::DEFAULT_MIN_MEM_MIB),
                i64::from(crate::prefs::DEFAULT_MAX_MEM_MIB)
            )
        );
    }

    #[test]
    fn the_java_tab_reaches_the_launch_path() {
        let mut prefs = crate::prefs::Prefs::default();
        prefs.default_java_path = Some("  C:/jdk/bin/javaw.exe ".to_string());
        prefs
            .java_paths
            .insert("8".to_string(), "C:/jdk8/bin/java.exe".to_string());
        prefs
            .java_paths
            .insert("21".to_string(), "C:/jdk21/bin/java.exe".to_string());
        // Neither of these is a Java to run with.
        prefs
            .java_paths
            .insert("not-a-major".to_string(), "C:/nope/java.exe".to_string());
        prefs.java_paths.insert("17".to_string(), "   ".to_string());

        let java = java_prefs(&prefs);
        assert_eq!(java.default_binary(), Some("C:/jdk/bin/javaw.exe"));
        assert_eq!(
            java.by_major.len(),
            2,
            "a bad key and a blank path are not choices"
        );
        // The version's own order decides, not the map's. This is the case the
        // tab exists for: 1.12.2 accepts 8 and would otherwise be run on 21.
        assert_eq!(
            java.for_majors(&[8, 21]),
            vec![(8, "C:/jdk8/bin/java.exe"), (21, "C:/jdk21/bin/java.exe")]
        );
        assert_eq!(java.for_majors(&[21, 8])[0].0, 21);
        assert!(java.for_majors(&[17]).is_empty());
        assert!(JavaPrefs::default().default_binary().is_none());
    }
}
