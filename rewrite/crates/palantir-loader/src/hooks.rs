//! The launch pieces a player configures instead of a version document:
//! extra arguments, environment entries, and the three hooks (pre-launch,
//! wrapper, post-exit).
//!
//! One rule runs through all of it: a configured command is parsed with
//! shell *quoting* but executed directly, with no shell ever involved. The
//! reference's v1 split these on whitespace and had to migrate once paths
//! with spaces broke (`settings.rs:397-420`); starting with a real quoter
//! means there is nothing to migrate away from. A configured command is
//! never a script body: it is a program and its arguments.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Split one command line into words the way a shell would: unquoted
/// whitespace separates, `'...'` is literal, `"..."` groups with backslash
/// escapes for `"` and `\`, and a lone backslash escapes the next character.
///
/// An unterminated quote is an error rather than a silent truncation: a hook
/// that runs half configured is worse than a hook that refuses to run.
pub fn command_words(input: &str) -> Result<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let next = chars.next().ok_or_else(|| Error::Invalid {
                    what: "command",
                    why: "a trailing backslash has nothing to escape".to_string(),
                })?;
                word.push(next);
                started = true;
            }
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => word.push(c),
                        None => {
                            return Err(Error::Invalid {
                                what: "command",
                                why: "an unterminated single quote".to_string(),
                            });
                        }
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => {
                            let next = chars.next().ok_or_else(|| Error::Invalid {
                                what: "command",
                                why: "a trailing backslash inside double quotes".to_string(),
                            })?;
                            // Inside double quotes a backslash escapes only
                            // the quote and itself -- the shell's rule, kept.
                            match next {
                                '"' | '\\' => word.push(next),
                                other => {
                                    word.push('\\');
                                    word.push(other);
                                }
                            }
                        }
                        Some(c) => word.push(c),
                        None => {
                            return Err(Error::Invalid {
                                what: "command",
                                why: "an unterminated double quote".to_string(),
                            });
                        }
                    }
                }
            }
            c if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

/// Quote words so `command_words` reads them back exactly: the inverse of
/// the parser, used to show a stored argument list or hook in an edit field.
pub fn quote_words(words: &[String]) -> String {
    words
        .iter()
        .map(|word| {
            let safe = !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_./:=+-".contains(c));
            if safe {
                word.clone()
            } else {
                format!("'{}'", word.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Turn parsed words into environment entries: `KEY=VALUE`, split at the
/// *first* `=`, so a value containing `=` survives intact. The order of the
/// input is the order of the result -- launches depend on it.
pub fn env_pairs(words: &[String]) -> Result<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    for word in words {
        let (key, value) = word.split_once('=').ok_or_else(|| Error::Invalid {
            what: "environment entry",
            why: format!("{word:?} is not a KEY=VALUE pair"),
        })?;
        if key.is_empty() || key.contains(char::is_control) {
            return Err(Error::Invalid {
                what: "environment entry",
                why: format!("{key:?} is not a usable variable name"),
            });
        }
        pairs.push((key.to_string(), value.to_string()));
    }
    Ok(pairs)
}

/// The three hooks a launch can carry. `None` means the hook does not exist,
/// which is different from an empty command (which refuses to run).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hooks {
    /// Runs to completion before the game spawns. A failure stops the launch.
    pub pre_launch: Option<String>,
    /// Wraps the game command: its words become the program and prefix
    /// arguments (`gamemoderun java ...`).
    pub wrapper: Option<String>,
    /// Runs after the game's process ends, however it ended.
    pub post_exit: Option<String>,
}

/// Everything about a launch that the game's own metadata does not decide.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchCustom {
    /// Words appended to the game's own arguments, in the order stored.
    pub extra_args: Vec<String>,
    /// Environment entries applied over the inherited environment, in the
    /// order stored.
    pub env: Vec<(String, String)>,
    pub hooks: Hooks,
}

impl LaunchCustom {
    /// Is there nothing here to apply?
    pub fn is_empty(&self) -> bool {
        self.extra_args.is_empty()
            && self.env.is_empty()
            && self.hooks.pre_launch.is_none()
            && self.hooks.wrapper.is_none()
            && self.hooks.post_exit.is_none()
    }
    /// Reject a configured hook that would not run: each present hook must
    /// parse to at least one word. Checked at save time so the edit field can
    /// say what is wrong instead of the launch failing later.
    pub fn validate(&self) -> Result<()> {
        for (name, hook) in [
            ("pre-launch", self.hooks.pre_launch.as_deref()),
            ("wrapper", self.hooks.wrapper.as_deref()),
            ("post-exit", self.hooks.post_exit.as_deref()),
        ] {
            if let Some(command) = hook {
                if command_words(command)?.is_empty() {
                    return Err(Error::Invalid {
                        what: "hook",
                        why: format!("the {name} hook names no program"),
                    });
                }
            }
        }
        Ok(())
    }
    /// Apply the environment entries and append the extra arguments to a
    /// command that already carries the plan's own arguments.
    pub fn apply(&self, command: &mut std::process::Command) {
        for (key, value) in &self.env {
            command.env(key, value);
        }
        command.args(&self.extra_args);
    }
}

/// Run one hook: parsed like a shell command line, executed directly. The
/// custom environment entries apply over the inherited one. The caller owns
/// what a failure means (a pre-launch failure stops the launch; a post-exit
/// failure is reported and the launcher carries on).
pub fn run(command: &str, dir: &Path, env: &[(String, String)]) -> Result<std::process::Output> {
    let words = command_words(command)?;
    let (program, args) = words.split_first().ok_or_else(|| Error::Invalid {
        what: "hook",
        why: "the command names no program".to_string(),
    })?;
    let mut process = std::process::Command::new(program);
    process.args(args).current_dir(dir);
    for (key, value) in env {
        process.env(key, value);
    }
    process.output().map_err(|source| Error::Io {
        path: PathBuf::from(program),
        source,
    })
}

/// Fold the wrapper hook around the game command: the wrapper's words become
/// the program and its prefix arguments, then the java path and the plan's
/// own arguments follow, exactly what wrappers like `gamemoderun` expect.
pub fn wrap(wrapper: &str, java: &Path, rest: &[String]) -> Result<(PathBuf, Vec<String>)> {
    let words = command_words(wrapper)?;
    let mut iter = words.into_iter();
    let program = iter.next().ok_or_else(|| Error::Invalid {
        what: "wrapper hook",
        why: "the command names no program".to_string(),
    })?;
    let mut args: Vec<String> = iter.collect();
    args.push(java.to_string_lossy().into_owned());
    args.extend(rest.iter().cloned());
    Ok((PathBuf::from(program), args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_round_trips_and_refuses_to_guess() {
        assert_eq!(
            command_words(r#"one "two words" three"#).unwrap(),
            vec!["one", "two words", "three"]
        );
        assert_eq!(
            command_words(r#"'/path with space/run.sh' --flag"#).unwrap(),
            vec!["/path with space/run.sh", "--flag"]
        );
        assert_eq!(command_words(r#"a\ b"#).unwrap(), vec!["a b"]);
        assert_eq!(
            command_words(r#"say "a \"quoted\" word""#).unwrap(),
            vec!["say", r#"a "quoted" word"#]
        );
        // Unterminated quotes refuse; they never run half a command.
        assert!(command_words("\"open").is_err());
        assert!(command_words("'open").is_err());
        assert!(command_words("trail\\").is_err());
        // The inverse: quoting a list reads back exactly.
        let words = vec![
            "/path with space/run.sh".to_string(),
            "a'b".to_string(),
            "".to_string(),
        ];
        assert_eq!(command_words(&quote_words(&words)).unwrap(), words);
    }

    #[test]
    fn env_pairs_split_at_the_first_equals_and_keep_order() {
        let words = command_words("FOO=bar BAR=a=b EMPTY=").unwrap();
        let pairs = env_pairs(&words).unwrap();
        assert_eq!(
            pairs,
            vec![
                ("FOO".to_string(), "bar".to_string()),
                ("BAR".to_string(), "a=b".to_string()),
                ("EMPTY".to_string(), String::new()),
            ]
        );
        assert!(env_pairs(&command_words("NO_EQUALS").unwrap()).is_err());
        assert!(env_pairs(&command_words("=value").unwrap()).is_err());
    }
}
