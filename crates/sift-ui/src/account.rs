//! Signing in to a Microsoft account, off the UI thread.
//!
//! The whole chain already exists in `sift_net::auth` — device code, polling, the Xbox and XSTS
//! exchanges, and the Minecraft profile fetch (`MicrosoftAuth::request_device_code`, `poll`,
//! `finish`). What was missing was a way for the window to drive it without blocking a frame:
//! this module owns one worker thread, one channel, and the four states a reader can see.
//!
//! `shortcut: the session is kept in memory only; upgrade when a refresh token is stored, which
//! is what makes a restart need no sign-in.` Nothing is written to disk here, so a restart asks
//! again.

use std::sync::mpsc::{channel, Receiver, Sender};

use sift_net::auth::{MicrosoftAuth, PollOutcome};

/// What the window knows about the account.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum State {
    /// Nobody has asked to sign in.
    #[default]
    Idle,
    /// A device code is being requested: one round trip, usually a fraction of a second.
    Starting,
    /// The reader has a code to enter at a URL. The code is the one Microsoft printed.
    Waiting {
        /// The short code the reader types, e.g. `K7QP2LXRZ`.
        code: String,
        /// Where they type it, e.g. `https://microsoft.com/link`.
        uri: String,
    },
    /// Signed in, with the profile's name.
    SignedIn {
        /// The Minecraft profile's name.
        name: String,
        /// Its id, which is what a launch passes as the player's uuid.
        uuid: String,
    },
    /// The chain failed, with the reason the transport or the protocol gave.
    Failed(String),
}

/// Where the account is kept, beside the instances folder.
fn store_path() -> std::path::PathBuf {
    crate::data::instances_dir()
        .parent()
        .map(|root| root.join("account.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("account.json"))
}

/// What the file holds: who is signed in and what renews them.
#[derive(Debug, Clone)]
struct Stored {
    name: String,
    uuid: String,
    refresh_token: String,
}

/// Read the stored account, if there is one. A file that will not parse is treated as none.
fn read_stored() -> Option<Stored> {
    read_at(&store_path())
}

fn read_at(path: &std::path::Path) -> Option<Stored> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(Stored {
        name: value.get("name")?.as_str()?.to_owned(),
        uuid: value.get("uuid")?.as_str()?.to_owned(),
        refresh_token: value.get("refresh_token")?.as_str()?.to_owned(),
    })
}

fn write_stored(stored: &Stored) {
    write_at(&store_path(), stored);
}

fn write_at(path: &std::path::Path, stored: &Stored) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let value = serde_json::json!({
        "name": stored.name,
        "uuid": stored.uuid,
        "refresh_token": stored.refresh_token,
    });
    if let Ok(text) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(path, text);
    }
}

fn clear_stored() {
    let _ = std::fs::remove_file(store_path());
}

/// What the worker sends back to the window.
#[derive(Debug)]
enum Message {
    Waiting { code: String, uri: String },
    SignedIn { name: String, uuid: String, token: String },
    Failed(String),
}

/// The account flow and its state.
#[derive(Debug, Default)]
pub struct Account {
    state: State,
    /// The game's access token, kept out of `State` so it cannot reach a `{:?}`.
    token: Option<String>,
    receiver: Option<Receiver<Message>>,
}

impl Account {
    /// What the window should draw.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// The signed-in profile, if there is one.
    pub fn session(&self) -> Option<(&str, &str)> {
        match &self.state {
            State::SignedIn { name, uuid } => Some((name.as_str(), uuid.as_str())),
            _ => None,
        }
    }

    /// What a launch needs to start as this account: name, uuid and access token.
    ///
    /// `None` when nobody is signed in, which is the offline player's case.
    pub fn launch_identity(&self) -> Option<(String, String, String)> {
        match (&self.state, &self.token) {
            (State::SignedIn { name, uuid }, Some(token)) => {
                Some((name.clone(), uuid.clone(), token.clone()))
            }
            _ => None,
        }
    }

    /// Whether a worker is still talking to Microsoft, which is what keeps the loop awake.
    pub fn busy(&self) -> bool {
        matches!(self.state, State::Starting | State::Waiting { .. })
    }

    /// Pick up the account a previous run left, and renew its token.
    ///
    /// Returns whether there was one. The profile shows immediately -- a reader who signed in
    /// yesterday should not see `Sign in to Minecraft` for the two seconds a refresh takes --
    /// and the token arrives on the same channel a fresh sign-in uses.
    pub fn restore(&mut self) -> bool {
        let Some(stored) = read_stored() else {
            return false;
        };
        self.state = State::SignedIn {
            name: stored.name.clone(),
            uuid: stored.uuid.clone(),
        };
        let (sender, receiver) = channel();
        self.receiver = Some(receiver);
        let for_thread = stored.clone();
        let name = stored.name;
        let spawned = std::thread::Builder::new()
            .name("sift-refresh".to_owned())
            .spawn(move || refresh(sender, for_thread));
        if let Err(error) = spawned {
            // The profile is still shown; only the game token is missing, and a launch without
            // one is the engine's to refuse with its own sentence.
            self.state = State::Failed(format!(
                "signed in as {name} but the token could not be renewed: {error}"
            ));
            self.receiver = None;
        }
        true
    }

    /// Begin a sign-in. A second press while one is running is refused rather than starting a
    /// second worker: two device codes at once would put two codes on screen and only one of
    /// them live.
    pub fn start(&mut self) {
        if self.busy() {
            return;
        }
        let (sender, receiver) = channel();
        self.state = State::Starting;
        self.receiver = Some(receiver);
        let spawned = std::thread::Builder::new()
            .name("sift-sign-in".to_owned())
            .spawn(move || run(sender));
        if let Err(error) = spawned {
            self.state = State::Failed(format!("the sign-in thread could not start: {error}"));
            self.receiver = None;
        }
    }

    /// Take whatever the worker has said since the last frame.
    pub fn poll(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        // Drained, not taken once: a fast failure followed by nothing must still be seen on
        // the frame it arrives.
        while let Ok(message) = receiver.try_recv() {
            self.state = match message {
                Message::Waiting { code, uri } => State::Waiting { code, uri },
                Message::SignedIn { name, uuid, token } => {
                    self.token = Some(token);
                    State::SignedIn { name, uuid }
                }
                Message::Failed(reason) => State::Failed(reason),
            };
        }
    }

    /// Forget the session, and the file that would bring it back.
    pub fn sign_out(&mut self) {
        self.state = State::Idle;
        self.token = None;
        self.receiver = None;
        clear_stored();
    }
}

/// The refresh worker: one exchange, then the profile's own game token.
fn refresh(sender: Sender<Message>, stored: Stored) {
    let auth = MicrosoftAuth::with_public_client_id();
    let token = match auth.refresh(&stored.refresh_token) {
        Ok(token) => token,
        Err(error) => {
            // A refresh that fails is a session that cannot be renewed: the file goes, so the
            // next start asks for a sign-in rather than pretending.
            clear_stored();
            let _ = sender.send(Message::Failed(format!(
                "the saved sign-in expired ({error}); sign in again"
            )));
            return;
        }
    };
    let refresh_token = token.refresh_token.clone();
    match auth.finish(&token) {
        Ok(session) => {
            write_stored(&Stored {
                name: session.name.clone(),
                uuid: session.uuid.clone(),
                refresh_token,
            });
            let _ = sender.send(Message::SignedIn {
                name: session.name,
                uuid: session.uuid,
                token: session.access_token,
            });
        }
        Err(error) => {
            let _ = sender.send(Message::Failed(format!("{error}")));
        }
    }
}

/// The worker: request a code, print it to the window, then poll until Microsoft answers.
///
/// `MicrosoftAuth::poll` makes a single request and says what it saw, so the waiting is here:
/// sleep the interval, ask again, and keep asking until the reader finishes, the code expires,
/// or the transport fails. A transport failure ends it, because a code that never reaches
/// Microsoft is not something more waiting fixes.
fn run(sender: Sender<Message>) {
    let auth = MicrosoftAuth::with_public_client_id();
    let device = match auth.request_device_code() {
        Ok(device) => device,
        Err(error) => {
            let _ = sender.send(Message::Failed(format!("{error}")));
            return;
        }
    };
    if sender
        .send(Message::Waiting {
            code: device.user_code.clone(),
            uri: device.verification_uri.clone(),
        })
        .is_err()
    {
        // The window went away; there is nobody to sign in for.
        return;
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(device.expires_in);
    let mut interval = device.interval.max(1);
    let token = loop {
        if std::time::Instant::now() >= deadline {
            let _ = sender.send(Message::Failed(
                "the sign-in code expired before it was used".to_owned(),
            ));
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(interval));
        match auth.poll(&device.device_code, interval) {
            Ok(PollOutcome::Retry { interval: next }) => interval = next.max(1),
            Ok(PollOutcome::Authorized(token)) => break token,
            Ok(PollOutcome::Failed(reason)) => {
                let _ = sender.send(Message::Failed(reason));
                return;
            }
            Err(error) => {
                let _ = sender.send(Message::Failed(format!("{error}")));
                return;
            }
        }
    };

    let refresh_token = token.refresh_token.clone();
    match auth.finish(&token) {
        Ok(session) => {
            // The profile is worth keeping; the token it must be renewed with is kept with it.
            write_stored(&Stored {
                name: session.name.clone(),
                uuid: session.uuid.clone(),
                refresh_token,
            });
            let _ = sender.send(Message::SignedIn {
                name: session.name,
                uuid: session.uuid,
                token: session.access_token,
            });
        }
        Err(error) => {
            let _ = sender.send(Message::Failed(format!("{error}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file the next run reads: a round trip, and a file that is not one is no account.
    #[test]
    fn a_stored_account_round_trips_and_nonsense_is_ignored() {
        let dir = std::env::temp_dir().join(format!("sift-account-{}", std::process::id()));
        let path = dir.join("account.json");
        let stored = Stored {
            name: "Steve".into(),
            uuid: "0123456789abcdef".into(),
            refresh_token: "M.R3_BAY.token".into(),
        };
        write_at(&path, &stored);
        let back = read_at(&path).expect("the file just written");
        assert_eq!(back.name, "Steve");
        assert_eq!(back.uuid, "0123456789abcdef");
        assert_eq!(back.refresh_token, "M.R3_BAY.token");

        std::fs::write(&path, "not json at all").expect("a broken file");
        assert!(read_at(&path).is_none(), "a broken file is no account");
        std::fs::write(&path, r#"{"name":"Steve"}"#).expect("a partial file");
        assert!(read_at(&path).is_none(), "a file with no token is no account");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_fresh_account_is_idle_and_not_busy() {
        let account = Account::default();
        assert_eq!(account.state(), &State::Idle);
        assert!(!account.busy());
        assert!(account.session().is_none());
    }

    #[test]
    fn waiting_is_busy_and_signing_in_is_not() {
        let mut account = Account::default();
        account.state = State::Waiting {
            code: "ABC".into(),
            uri: "https://microsoft.com/link".into(),
        };
        assert!(account.busy(), "a code on screen is still a live worker");
        account.state = State::SignedIn {
            name: "Steve".into(),
            uuid: "0".into(),
        };
        assert!(!account.busy());
        assert_eq!(account.session(), Some(("Steve", "0")));
    }

    /// A signed-in account hands a launch its name, uuid and token; anything else hands it
    /// nothing, which is what keeps the offline player for a reader who has not signed in.
    #[test]
    fn a_launch_identity_needs_both_the_state_and_the_token() {
        let mut account = Account {
            state: State::SignedIn {
                name: "Steve".into(),
                uuid: "abc".into(),
            },
            token: None,
            receiver: None,
        };
        assert_eq!(account.launch_identity(), None, "no token, no launch identity");

        account.token = Some("token-value".into());
        assert_eq!(
            account.launch_identity(),
            Some(("Steve".into(), "abc".into(), "token-value".into()))
        );

        account.sign_out();
        assert_eq!(account.launch_identity(), None, "signing out forgets both");
    }

    #[test]
    fn a_failure_is_neither_busy_nor_signed_in() {
        let account = Account {
            state: State::Failed("no network".into()),
            token: None,
            receiver: None,
        };
        assert!(!account.busy());
        assert!(account.session().is_none());
    }
}
