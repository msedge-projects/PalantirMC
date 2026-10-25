//! Discover's real search: the query goes to Modrinth through the launcher's own client.
//!
//! **Off the frame thread, like the icons.** A search is a network round trip and a JSON
//! decode, so it is a worker, a channel and a state machine -- `Idle`, `Loading`,
//! `Ready(hits)`, `Failed(reason)` -- and the page draws whichever of those it is in. The
//! frame that finds a reply replaces the list; no frame waits for one.
//!
//! **The URL comes from the client crate, not from string surgery here.**
//! `sift_net::modrinth::search_url_parts` is the one place the parameter spelling lives, and
//! `ModrinthSearchResponse` is the one place the response shape lives. This module is the
//! glue: a thread, two channels and a cache keyed by the query it answered.
//!
//! **One request in flight at a time, and the last one wins.** Typing produces a query per
//! keystroke; a client that fired a request per keystroke would hammer the API and deliver
//! replies out of order. A request is only made for a query that has not been asked for, and
//! a reply for a query that is no longer current is dropped rather than shown.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

/// What a card needs from a search hit, decoded on the worker.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The project's stable id.
    pub id: String,
    /// Its name.
    pub title: String,
    /// The publishing team's name.
    pub author: String,
    /// The one-line summary.
    pub summary: String,
    /// The icon's URL, empty when the project has none.
    pub icon: String,
    /// Downloads, already abbreviated the way the reference shows them.
    pub downloads: String,
    /// Followers, abbreviated the same way.
    pub followers: String,
    /// The loader or loaders the project supports, as the API lists them.
    pub loaders: Vec<String>,
}

/// The search's state.
#[derive(Debug, Default)]
pub enum State {
    /// Nothing asked yet.
    #[default]
    Idle,
    /// A query is out.
    Loading,
    /// Results, for the query named, and how many the API says exist in total.
    ///
    /// The total is not `hits.len()`: one page is twenty of a search that may match thousands,
    /// and the toolbar's `1 - 946` is the reference printing exactly that distinction.
    Ready {
        query: String,
        hits: Vec<Hit>,
        total: u64,
    },
    /// The query failed, with a reason a page can show.
    Failed { query: String, reason: String },
}

/// The search: its state, the query it last answered, and the channel its workers answer on.
pub struct Search {
    state: State,
    /// The query the current state belongs to, so a stale reply can be dropped.
    current: Option<String>,
    sender: Sender<(String, Result<(Vec<Hit>, u64), String>)>,
    receiver: Receiver<(String, Result<(Vec<Hit>, u64), String>)>,
}

impl Default for Search {
    fn default() -> Search {
        let (sender, receiver) = channel();
        Search {
            state: State::Idle,
            current: None,
            sender,
            receiver,
        }
    }
}

impl Search {
    /// The state the page draws.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Whether a query is out: the caller keeps drawing while this is true.
    pub fn loading(&self) -> bool {
        matches!(self.state, State::Loading)
    }

    /// Ask for `query`, unless it is the one already answered or already out.
    ///
    /// The empty query is not asked: Discover with no query shows the reference's own default
    /// feed, which is a different request with its own facets and does not belong here yet.
    pub fn request(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() || self.current.as_deref() == Some(query) {
            return;
        }
        self.current = Some(query.to_owned());
        self.state = State::Loading;

        let owned = query.to_owned();
        let reply = owned.clone();
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("sift-search".to_owned())
            .spawn(move || {
                let _ = sender.send((reply, run(&owned)));
            });
        if spawned.is_err() {
            self.state = State::Failed {
                query: query.to_owned(),
                reason: "could not start a search worker".to_owned(),
            };
        }
    }

    /// Take any reply that has arrived. Called once per frame by the page.
    pub fn poll(&mut self) {
        while let Ok((query, result)) = self.receiver.try_recv() {
            // A reply for a query the reader has already moved past is dropped: showing it
            // would put results under the wrong search box.
            if self.current.as_deref() != Some(query.as_str()) {
                continue;
            }
            self.state = match result {
                Ok((hits, total)) => State::Ready { query, hits, total },
                Err(reason) => State::Failed { query, reason },
            };
        }
    }
}

/// Fetch and decode one search. Runs on a worker thread.
fn run(query: &str) -> Result<(Vec<Hit>, u64), String> {
    let url = sift_net::modrinth::search_url_parts(
        query,
        Some("modpack"),
        Some("relevance"),
        20,
        0,
    );
    // The launcher's own client, streaming to a temporary file: the same timeout and user
    // agent as every other request, and a body that never has to fit on this thread's stack.
    let path = std::env::temp_dir().join(format!("sift-search-{:016x}.json", hash(query)));
    let fetched = sift_net::download_file(&url, &path, Duration::from_secs(20))
        .map_err(|error| error.to_string());
    let body = match fetched {
        Ok(_bytes) => std::fs::read(&path).map_err(|error| error.to_string()),
        Err(error) => Err(error),
    };
    let _ = std::fs::remove_file(&path);

    let body = body?;
    let page: sift_net::modrinth::ModrinthSearchResponse =
        serde_json::from_slice(&body).map_err(|error| error.to_string())?;
    // The total travels with the page: twenty hits out of nine hundred is the number the
    // reference's toolbar shows, and `hits.len()` cannot produce it.
    let total = u64::from(page.total_hits);
    Ok((page.hits.into_iter().map(hit).collect(), total))
}

/// One API hit, as a card needs it.
fn hit(source: sift_net::modrinth::ModrinthSearchHit) -> Hit {
    Hit {
        id: source.project_ref().to_owned(),
        title: source.title,
        author: source.author,
        summary: source.description,
        icon: source.icon_url,
        downloads: abbreviate(source.downloads),
        followers: abbreviate(source.follows),
        loaders: source
            .categories
            .into_iter()
            .filter(|category| {
                matches!(
                    category.as_str(),
                    "fabric" | "forge" | "neoforge" | "quilt" | "liteloader" | "risugami"
                )
            })
            .map(|category| match category.as_str() {
                "neoforge" => "NeoForge".to_owned(),
                "liteloader" => "LiteLoader".to_owned(),
                "risugami" => "Risugami".to_owned(),
                other => {
                    let mut chars = other.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                }
            })
            .collect(),
    }
}

/// `19_380_000` -> `19.38M`, the way the reference's cards print a count.
///
/// Two decimals on millions and one on thousands, both read off the reference's own cards
/// (`19.38M`, `15.36M`, `2.7K`). A number under a thousand is shown as itself. The project
/// page prints downloads with one decimal, so a page that wants that formatting rounds this
/// result rather than growing a second rule here.
pub fn abbreviate(count: u64) -> String {
    match count {
        0..=999 => count.to_string(),
        1_000..=999_999 => format!("{:.1}K", count as f64 / 1_000.0),
        _ => format!("{:.2}M", count as f64 / 1_000_000.0),
    }
}

/// A stable name for a query, for the temporary file.
fn hash(query: &str) -> u64 {
    let mut value: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in query.as_bytes() {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The abbreviations the reference's cards show, at each threshold.
    #[test]
    fn counts_abbreviate_the_way_the_reference_shows_them() {
        assert_eq!(abbreviate(0), "0");
        assert_eq!(abbreviate(999), "999");
        assert_eq!(abbreviate(1_000), "1.0K");
        assert_eq!(abbreviate(15_360_000), "15.36M");
        assert_eq!(abbreviate(19_380_000), "19.38M");
        // The reference prints 2.7K for 2,732 followers, and the project page prints 19.4M
        // by rounding this result.
        assert_eq!(abbreviate(2_732), "2.7K");
    }

    /// A query that is empty or unchanged is not asked again: the second request would be the
    /// same answer bought twice.
    #[test]
    fn the_same_query_is_not_asked_twice() {
        let mut search = Search::default();
        search.request("   ");
        assert!(matches!(search.state(), State::Idle), "whitespace is not a query");
        search.request("sodium");
        assert!(matches!(search.state(), State::Loading));
        search.request("sodium");
        assert!(matches!(search.state(), State::Loading), "still the first request");
    }

    /// A search starts idle and owes no frame: nothing is out until something asks.
    #[test]
    fn a_fresh_search_is_idle() {
        let search = Search::default();
        assert!(!search.loading());
        assert!(matches!(search.state(), State::Idle));
    }
}
