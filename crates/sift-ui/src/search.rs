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
    /// The project's display categories (`client`, `optimization`, ...), which the API sends in
    /// the same array as the loaders and the reference draws ahead of them.
    pub categories: Vec<String>,
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

    /// Ask for `query` within `project_type`, unless that is what is already answered or out.
    ///
    /// The empty query is not asked: Discover with no query shows the reference's own default
    /// feed, which is a different request with its own facets and does not belong here yet.
    pub fn request(
        &mut self,
        query: &str,
        project_type: &str,
        index: &str,
        limit: u32,
        offset: u32,
        facets: &[String],
    ) {
        let query = query.trim();
        // The page and the filters are part of the question: without them a second page or a
        // filtered search is the unfiltered first page's answer.
        let asked = format!(
            "{project_type}|{index}|{limit}|{offset}|{}|{query}",
            facets.join(",")
        );
        // An empty box is the reference's default feed and is not asked -- unless a facet is
        // chosen, which is a question of its own and the one a category press asks.
        if (query.is_empty() && facets.is_empty())
            || self.current.as_deref() == Some(asked.as_str())
        {
            return;
        }
        self.current = Some(asked);
        self.state = State::Loading;
        let filters = facets.to_vec();

        let owned = query.to_owned();
        let kind = project_type.to_owned();
        let order = index.to_owned();
        let reply = self.current.clone().unwrap_or_default();
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("sift-search".to_owned())
            .spawn(move || {
                let _ = sender.send((reply, run(&owned, &kind, &order, limit, offset, &filters)));
            });
        if spawned.is_err() {
            self.state = State::Failed {
                query: query.to_owned(),
                reason: "could not start a search worker".to_owned(),
            };
        }
    }

    /// Forget the answered query, so the next `request` asks again.
    ///
    /// Used when the *kind* changes and the text does not: the query is the same string but the
    /// question is different, and without this the second question would never be asked.
    pub fn forget(&mut self) {
        self.current = None;
        self.state = State::Idle;
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
fn run(
    query: &str,
    project_type: &str,
    index: &str,
    limit: u32,
    offset: u32,
    facets: &[String],
) -> Result<(Vec<Hit>, u64), String> {
    // The tab the reader is on is a facet, not a filter applied afterwards: the API answers
    // the right kind of project and counts the right total, which a local filter could not.
    // The sort and the page size are the API's own parameters, not a re-sort afterwards: the
    // total has to match the order the reader asked for, and only the API can count that.
    let url = sift_net::modrinth::search_url_parts_with_facets(
        query,
        Some(project_type),
        Some(index),
        limit,
        offset,
        facets,
    );
    // The launcher's own client, streaming to a temporary file: the same timeout and user
    // agent as every other request, and a body that never has to fit on this thread's stack.
    let path = std::env::temp_dir().join(format!(
        "sift-search-{:016x}.json",
        hash(&format!("{project_type}|{index}|{limit}|{offset}|{}|{query}", facets.join(",")))
    ));
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

/// Whether a name in the API's `categories` array is a loader rather than a display category.
fn is_loader(category: &str) -> bool {
    matches!(
        category,
        "fabric" | "forge" | "neoforge" | "quilt" | "liteloader" | "risugami"
    )
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
        followers: thousands(source.follows),
        categories: source
            .categories
            .iter()
            .filter(|category| !is_loader(category))
            .map(|category| {
                // The API's ids are lowercase and hyphenated; a card shows them capitalised.
                let spaced = category.replace('-', " ");
                let mut letters = spaced.chars();
                match letters.next() {
                    Some(first) => format!("{}{}", first.to_uppercase(), letters.as_str()),
                    None => String::new(),
                }
            })
            .filter(|category| !category.is_empty())
            .collect(),
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
/// A follower count, printed in full with thin thousands separators -- the reference shows
/// `5,023` beside a download count it writes as `19.62M`, and `662` and `2,744` on the cards
/// below it.
pub fn thousands(count: u64) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A download count, abbreviated where the reference abbreviates.
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

    /// A facet is part of the question: the same query with a filter is a different request, and
    /// an empty box with a filter is still a request.
    #[test]
    fn a_facet_is_part_of_the_question() {
        let mut search = Search::default();
        search.request("", "modpack", "relevance", 20, 0, &[]);
        assert!(
            matches!(search.state(), State::Idle),
            "an empty box with no filter is the default feed, not a request"
        );

        let facet = vec!["categories:adventure".to_owned()];
        search.request("", "modpack", "relevance", 20, 0, &facet);
        assert!(
            matches!(search.state(), State::Loading),
            "an empty box with a filter is a question"
        );

        // The same query with a second filter is a different question, not a cached answer.
        let other = vec!["categories:magic".to_owned()];
        let before = search.current.clone();
        search.request("", "modpack", "relevance", 20, 0, &other);
        assert_ne!(search.current, before, "the filter is part of the key");
    }

    /// The abbreviations the reference's cards show, at each threshold.
    #[test]
    fn counts_abbreviate_the_way_the_reference_shows_them() {
        assert_eq!(abbreviate(0), "0");
        assert_eq!(abbreviate(999), "999");
        assert_eq!(abbreviate(1_000), "1.0K");
        assert_eq!(abbreviate(15_360_000), "15.36M");
        assert_eq!(abbreviate(19_380_000), "19.38M");
        // Downloads are a size rule: one decimal in thousands, two in millions.
        assert_eq!(abbreviate(2_732), "2.7K");
    }

    /// Followers are not abbreviated at all: the reference prints them in full with separators,
    /// which is how a card can show `5,023` next to `19.62M`.
    #[test]
    fn follower_counts_keep_their_digits() {
        assert_eq!(thousands(662), "662");
        assert_eq!(thousands(2_744), "2,744");
        assert_eq!(thousands(5_023), "5,023");
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(0), "0");
    }

    /// A query that is empty or unchanged is not asked again: the second request would be the
    /// same answer bought twice.
    #[test]
    fn the_same_query_is_not_asked_twice() {
        let mut search = Search::default();
        search.request("   ", "modpack", "relevance", 20, 0, &[]);
        assert!(matches!(search.state(), State::Idle), "whitespace is not a query");
        search.request("sodium", "modpack", "relevance", 20, 0, &[]);
        assert!(matches!(search.state(), State::Loading));
        search.request("sodium", "modpack", "relevance", 20, 0, &[]);
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
