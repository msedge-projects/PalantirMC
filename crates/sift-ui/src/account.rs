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

/// What the worker sends back to the window.
#[derive(Debug)]
enum Message {
    Waiting { code: String, uri: String },
    SignedIn { name: String, uuid: String },
    Failed(String),
}

/// The account flow and its state.
#[derive(Debug, Default)]
pub struct Account {
    state: State,
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

    /// Whether a worker is still talking to Microsoft, which is what keeps the loop awake.
    pub fn busy(&self) -> bool {
        matches!(self.state, State::Starting | State::Waiting { .. })
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
                Message::SignedIn { name, uuid } => State::SignedIn { name, uuid },
                Message::Failed(reason) => State::Failed(reason),
            };
        }
    }

    /// Forget the session. Nothing is stored, so this is the whole of signing out.
    pub fn sign_out(&mut self) {
        self.state = State::Idle;
        self.receiver = None;
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

    match auth.finish(&token) {
        Ok(session) => {
            let _ = sender.send(Message::SignedIn {
                name: session.name,
                uuid: session.uuid,
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

    #[test]
    fn a_failure_is_neither_busy_nor_signed_in() {
        let account = Account {
            state: State::Failed("no network".into()),
            receiver: None,
        };
        assert!(!account.busy());
        assert!(account.session().is_none());
    }
}
