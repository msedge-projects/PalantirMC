//! A minimal HTTP/1.1 server for deterministic transfer tests.
//!
//! Behaviours the tests need from a real service and cannot schedule:
//! range requests answered with 206 (and ignored), transient failures
//! before success, and connections dropped mid-body. Request recording is
//! the receipt tests assert on -- that a resume actually *sent* a Range
//! header, that a cache hit sent nothing at all.
//!
//! It lives here rather than beside one crate's tests because several
//! crates now need to stand up a fake service; it is a dev-dependency and
//! ships in no build.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

pub struct Route {
    pub body: Vec<u8>,
    /// Answer `Range` requests with 206. When false the header is ignored
    /// and the whole body arrives -- the restart case.
    pub support_range: bool,
    /// Answer 500 for this many requests before serving.
    pub fail_first: u32,
    /// Declare the full length but close the connection after this many
    /// body bytes: an interrupted transfer.
    pub truncate_after: Option<u64>,
    /// Always answer this status (404, 429, ...) instead of serving.
    pub always_status: Option<u16>,
}

impl Route {
    pub fn new(body: impl Into<Vec<u8>>) -> Self {
        Self {
            body: body.into(),
            support_range: true,
            fail_first: 0,
            truncate_after: None,
            always_status: None,
        }
    }
}

struct RouteState {
    route: Route,
    fail_remaining: u32,
}

pub struct MockServer {
    pub base: String,
    hits: Arc<Mutex<Vec<String>>>,
    range_offsets: Arc<Mutex<Vec<u64>>>,
    state: Arc<Mutex<HashMap<String, RouteState>>>,
}

impl MockServer {
    pub fn start(routes: Vec<(&str, Route)>) -> Self {
        let state: HashMap<String, RouteState> = routes
            .into_iter()
            .map(|(path, route)| {
                let fail_remaining = route.fail_first;
                (
                    path.to_string(),
                    RouteState {
                        route,
                        fail_remaining,
                    },
                )
            })
            .collect();
        let state = Arc::new(Mutex::new(state));
        let hits = Arc::new(Mutex::new(Vec::new()));
        let range_offsets = Arc::new(Mutex::new(Vec::new()));

        let listener = TcpListener::bind("127.0.0.1:0").expect("test port");
        let base = format!(
            "http://127.0.0.1:{}",
            listener.local_addr().expect("test addr").port()
        );
        {
            let state = Arc::clone(&state);
            let hits = Arc::clone(&hits);
            let range_offsets = Arc::clone(&range_offsets);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    let state = Arc::clone(&state);
                    let hits = Arc::clone(&hits);
                    let range_offsets = Arc::clone(&range_offsets);
                    std::thread::spawn(move || {
                        serve(stream, &state, &hits, &range_offsets);
                    });
                }
            });
        }

        Self {
            base,
            hits,
            range_offsets,
            state,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// Serve one more route after startup. Documents that name URLs on
    /// this server (a manifest pointing at versions) can only be built
    /// once `base` exists; they come here.
    pub fn set_route(&self, path: &str, route: Route) {
        let fail_remaining = route.fail_first;
        if let Ok(mut table) = self.state.lock() {
            table.insert(
                path.to_string(),
                RouteState {
                    route,
                    fail_remaining,
                },
            );
        }
    }

    /// Request paths in arrival order.
    pub fn hits(&self) -> Vec<String> {
        self.hits.lock().map(|h| h.clone()).unwrap_or_default()
    }

    /// The `Range` offsets the server was asked to continue from.
    pub fn range_offsets(&self) -> Vec<u64> {
        self.range_offsets
            .lock()
            .map(|r| r.clone())
            .unwrap_or_default()
    }
}

fn serve(
    mut stream: TcpStream,
    state: &Mutex<HashMap<String, RouteState>>,
    hits: &Mutex<Vec<String>>,
    range_offsets: &Mutex<Vec<u64>>,
) {
    let Some(request) = read_request(&mut stream) else {
        return;
    };
    let (path, range) = request;
    if let Ok(mut hits) = hits.lock() {
        hits.push(path.clone());
    }
    if let Some(offset) = range {
        if let Ok(mut seen) = range_offsets.lock() {
            seen.push(offset);
        }
    }

    let mut table = match state.lock() {
        Ok(table) => table,
        Err(_) => return,
    };
    let Some(entry) = table.get_mut(&path) else {
        let _ = write_head(&mut stream, "404 Not Found", 0, None);
        return;
    };
    if let Some(status) = entry.route.always_status {
        let _ = write_head(
            &mut stream,
            if status == 429 {
                "429 Too Many Requests"
            } else {
                "404 Not Found"
            },
            0,
            None,
        );
        return;
    }
    if entry.fail_remaining > 0 {
        entry.fail_remaining -= 1;
        let _ = write_head(&mut stream, "500 Internal Server Error", 0, None);
        return;
    }

    let body = entry.route.body.clone();
    let support_range = entry.route.support_range;
    // One-shot: the interruption happens once, like a dropped connection.
    // Left armed it would truncate the resumed transfer too, forever.
    let truncate_after = entry.route.truncate_after.take();
    drop(table);

    match range.filter(|_| support_range) {
        Some(from) if from as usize >= body.len() => {
            let _ = write_head(&mut stream, "416 Range Not Satisfiable", 0, None);
        }
        Some(from) => {
            let slice = &body[from as usize..];
            if write_head(
                &mut stream,
                "206 Partial Content",
                slice.len() as u64,
                Some((from, body.len() as u64)),
            )
            .is_ok()
            {
                let limited = truncate_after.map(|n| n as usize).unwrap_or(slice.len());
                let _ = stream.write_all(&slice[..limited.min(slice.len())]);
            }
        }
        None => {
            if write_head(&mut stream, "200 OK", body.len() as u64, None).is_ok() {
                let limited = truncate_after.map(|n| n as usize).unwrap_or(body.len());
                let _ = stream.write_all(&body[..limited.min(body.len())]);
            }
        }
    }
    // Connection: close semantics: the socket drops at the end of this
    // handler, mid-body when `truncate_after` said so.
}

fn read_request(stream: &mut TcpStream) -> Option<(String, Option<u64>)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let read = stream.read(&mut chunk).ok()?;
        buf.extend_from_slice(&chunk[..read]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.is_empty() {
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.lines();
    let first = lines.next()?;
    let path = first.split_whitespace().nth(1)?.to_string();
    let mut range = None;
    for line in lines {
        // Header names are case-insensitive and arrive lowercase on the
        // wire; a case-sensitive match here once cost an afternoon.
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("range") {
            // "bytes=N-" (and "bytes=N-M" parses the same way here)
            let Ok(span) = value.trim().strip_prefix("bytes=").ok_or(()) else {
                continue;
            };
            if let Some(from) = span.split('-').next().and_then(|n| n.parse::<u64>().ok()) {
                range = Some(from);
            }
        }
    }
    Some((path, range))
}

fn write_head(
    stream: &mut TcpStream,
    status: &str,
    content_length: u64,
    content_range: Option<(u64, u64)>,
) -> std::io::Result<()> {
    let mut head =
        format!("HTTP/1.1 {status}\r\nContent-Length: {content_length}\r\nConnection: close\r\n");
    if let Some((from, total)) = content_range {
        let to = from + content_length.saturating_sub(1);
        head.push_str(&format!("Content-Range: bytes {from}-{to}/{total}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.flush()
}
