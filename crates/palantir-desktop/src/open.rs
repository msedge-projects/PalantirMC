//! Opening a link in the browser this machine already has.
//!
//! The reference has an `openUrl` helper and uses it for every link it draws: a
//! project's page, a changelog, the news. This launcher had none until the
//! panel's news section, where an article card and its *View all news* button are
//! both links -- and a control that does nothing is the one thing this shell's
//! conventions refuse.
//!
//! It is also the only place this launcher starts a program that is not Minecraft
//! or Java, so the rules are stated here rather than at each call site:
//!
//! * **Two schemes, not every scheme.** `http` and `https` are opened. The feed is
//!   a stranger's JSON, and `file:///...` would hand the operating system a local
//!   file while a Windows `cmd:`-style scheme would hand it a command; neither is
//!   a link, and a scheme this module does not know is refused with a sentence.
//! * **The command is a value**, built per platform by [`command_for`], so the
//!   tests assert what *would* be run. A command that is only ever built is
//!   exactly the kind of thing that is wrong until somebody runs it, and only one
//!   of the three platforms is the one this is written on.
//! * **A failure is a sentence**, because a link that does not open is a thing a
//!   reader reports.
//!
//! Nothing here waits for the program: a browser that takes two seconds to start
//! must not hold the frame, and this launcher has no use for its exit status.

use std::process::Command;

/// Whether this launcher will hand `url` to the operating system.
///
/// The scheme has to be one of two, and it has to be followed by `//`: a link
/// names a host, and `https:something` is not a URL a browser would resolve. The
/// scheme is lower-cased first, because `HTTPS://` is the same scheme.
pub fn is_openable(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    rest.starts_with("//") && matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
}

/// The program and arguments that open `url` on `platform`, or `None` when this
/// launcher has no opener for it.
///
/// `platform` is a parameter rather than a `cfg!` inside, one of
/// [`std::env::consts::OS`]'s names, so the tests can assert all three commands
/// on whichever machine runs them.
pub fn command_for(platform: &str, url: &str) -> Option<(&'static str, Vec<String>)> {
    match platform {
        "windows" => {
            // `start` is a `cmd` builtin and its first quoted argument is the
            // window *title*, so the empty argument is what keeps the URL from
            // being read as one: without it, a URL with a space in it opens an
            // empty window titled by the first half of the address.
            Some((
                "cmd",
                vec![
                    "/C".to_string(),
                    "start".to_string(),
                    String::new(),
                    url.to_string(),
                ],
            ))
        }
        "macos" => Some(("open", vec![url.to_string()])),
        // Linux and the BSDs: `xdg-open` is the desktop's own opener, and it is
        // the one the reference's own environment would use too.
        "linux" | "freebsd" | "netbsd" | "openbsd" | "dragonfly" => {
            Some(("xdg-open", vec![url.to_string()]))
        }
        _ => None,
    }
}

/// Open `url` with the machine's own opener.
///
/// The program is spawned and not waited for; its output goes nowhere, because a
/// browser's chatter is not this launcher's to draw and a reader cannot act on it.
pub fn url(url: &str) -> Result<(), String> {
    if !is_openable(url) {
        return Err(format!("'{url}' is not a link this launcher will open"));
    }
    let (program, args) = command_for(std::env::consts::OS, url)
        .ok_or_else(|| format!("there is no way to open '{url}' on this system"))?;
    Command::new(program)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("opening '{url}' failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_web_schemes_are_links() {
        assert!(is_openable("https://modrinth.com/news"));
        assert!(is_openable("http://localhost:8080/x"));
        assert!(is_openable("HTTPS://MODRINTH.COM/news"), "a scheme is case-insensitive");
        // The feed is a stranger's JSON: a local file, a Windows command handler
        // and a `javascript:` URL all arrive as strings and none of them is a link.
        assert!(!is_openable("file:///C:/Windows/System32/cmd.exe"));
        assert!(!is_openable("cmd://whatever"));
        assert!(!is_openable("javascript://alert(1)"));
        assert!(!is_openable("ms-settings://privacy"));
        // And neither is something that only looks like one.
        assert!(!is_openable("https:modrinth.com/news"), "a link names a host");
        assert!(!is_openable("modrinth.com/news"));
        assert!(!is_openable(""));
    }

    #[test]
    fn each_platform_opens_with_its_own_command() {
        let url = "https://modrinth.com/news/article/sync-settings";
        // Windows: `cmd /C start "" <url>`, the empty title included -- without it
        // a URL with a space in it is read as a window title.
        let (program, args) = command_for("windows", url).expect("a windows command");
        assert_eq!(program, "cmd");
        assert_eq!(args, vec!["/C".to_string(), "start".to_string(), String::new(), url.to_string()]);
        // macOS and Linux: the opaque opener, with the URL as the only argument.
        let (program, args) = command_for("macos", url).expect("a macos command");
        assert_eq!((program, args), ("open", vec![url.to_string()]));
        let (program, args) = command_for("linux", url).expect("a linux command");
        assert_eq!((program, args), ("xdg-open", vec![url.to_string()]));
        // A platform nobody has written a command for says so rather than
        // guessing at one.
        assert!(command_for("plan9", url).is_none());
    }

    #[test]
    fn a_refused_link_says_so_instead_of_starting_something() {
        // The failure path is the one a test can run without a browser, and these
        // tests stop there: [`command_for`] is asserted as a value and [`url`] is
        // only ever called with a URL it refuses. A test that *does* run the
        // opener starts a real window on whoever ran `cargo test` -- `launch.rs`
        // had a `#[cfg(test)]` shim that did exactly that with `not-a-url`, and
        // every run of the suite popped Windows' "cannot find 'not-a-url'" dialog
        // over whatever else the machine was doing.
        let error = url("file:///etc/passwd").expect_err("a local file is not a link");
        assert!(error.contains("is not a link this launcher will open"), "{error}");
        assert!(error.contains("file:///etc/passwd"), "the refusal quotes it: {error}");
    }
}
