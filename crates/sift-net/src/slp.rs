//! The Server List Ping a launcher asks a Minecraft server before it shows it.
//!
//! The protocol is four packets: a handshake that names the server, a status request, and the
//! server's JSON answer. This module builds the first two, reads the third, and parses the
//! fourth — everything a card needs (the MOTD, the version, the player counts) comes from that
//! JSON, and the same document is what the reference's card is drawn from.
//!
//! `shortcut: modern (1.7+) servers only; upgrade when a reader needs a beta-era server pinged.`
//! The legacy pre-1.7 handshake is a different byte sequence and nothing in the reference speaks
//! it either.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde_json::Value;

/// The protocol version sent in the handshake. Servers accept any value they understand; this
/// asks for the newest the launcher could ever launch and lets the server answer in its own
/// protocol, which is what makes one number work against every server.
pub const PROTOCOL_VERSION: i32 = 769;

/// What a server said about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// The message of the day, with its formatting removed: the card draws one colour of text.
    pub motd: String,
    /// The version the server names, e.g. `1.21.4`.
    pub version: String,
    /// Players online.
    pub online: u32,
    /// The server's player cap.
    pub capacity: u32,
}

/// A varint: seven bits per byte, little-endian, the high bit meaning "another byte follows".
pub fn varint(value: i32) -> Vec<u8> {
    let mut value = value as u32;
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return out;
        }
    }
}

/// Read a varint from the front of `bytes`, returning it and how many bytes it took.
///
/// Long encodings are refused: the protocol caps a varint at five bytes, and a sixth means the
/// stream is not what the caller thinks it is.
pub fn read_varint(bytes: &[u8]) -> Option<(i32, usize)> {
    let mut result: u32 = 0;
    for (index, byte) in bytes.iter().take(5).enumerate() {
        result |= ((byte & 0x7f) as u32) << (7 * index);
        if byte & 0x80 == 0 {
            return Some((result as i32, index + 1));
        }
    }
    None
}

/// A packet: its length as a varint, then its id and payload.
fn frame(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut body = varint(id);
    body.extend_from_slice(payload);
    let mut out = varint(body.len() as i32);
    out.extend_from_slice(&body);
    out
}

/// The handshake + status request: the whole of what a client sends.
pub fn request(host: &str, port: u16) -> Vec<u8> {
    let mut payload = varint(PROTOCOL_VERSION);
    let host_bytes = host.as_bytes();
    payload.extend_from_slice(&varint(host_bytes.len() as i32));
    payload.extend_from_slice(host_bytes);
    payload.extend_from_slice(&port.to_be_bytes());
    payload.extend_from_slice(&varint(1)); // next state: status

    let mut out = frame(0x00, &payload);
    out.extend_from_slice(&frame(0x00, &[]));
    out
}

/// The MOTD as one line of plain text: `§`-style colour codes dropped and newlines flattened.
pub fn plain_motd(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(character) = chars.next() {
        match character {
            // A section sign starts a two-character formatting code.
            '\u{a7}' => {
                chars.next();
            }
            '\n' => out.push(' '),
            other => out.push(other),
        }
    }
    out.trim().to_string()
}

/// Parse the JSON a server answers with.
pub fn parse_status(json: &str) -> Option<Status> {
    let value: Value = serde_json::from_str(json).ok()?;
    let description = value.get("description")?;
    // `description` is a string on modern servers and a chat component on most others.
    let motd = match description {
        Value::String(text) => plain_motd(text),
        other => plain_motd(&chat_text(other)),
    };
    let version = value
        .get("version")
        .and_then(|version| version.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let players = value.get("players");
    let online = players
        .and_then(|players| players.get("online"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let capacity = players
        .and_then(|players| players.get("max"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    Some(Status {
        motd,
        version,
        online,
        capacity,
    })
}

/// Flatten a chat component into its text: a component is `{ text }` or `{ extra: [...] }`.
fn chat_text(value: &Value) -> String {
    let mut out = String::new();
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        out.push_str(text);
    }
    if let Some(extra) = value.get("extra").and_then(Value::as_array) {
        for part in extra {
            out.push_str(&chat_text(part));
        }
    }
    out
}

/// Ask a server what it is. Blocking, with a timeout, for the caller's worker thread.
///
/// `host` is the name a player types; a `host:port` spelling is accepted and the port wins over
/// the one passed here, because that is how the reference reads its own inputs.
pub fn ping(host: &str, port: u16, timeout: Duration) -> Result<Status, String> {
    let (host, port) = split_address(host, port);
    let address = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|error| format!("{host}:{port} could not be resolved: {error}"))?
        .next()
        .ok_or_else(|| format!("{host}:{port} resolved to no address"))?;
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| format!("{host}:{port} did not answer: {error}"))?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    stream
        .write_all(&request(&host, port))
        .map_err(|error| format!("{host}:{port} closed while asking: {error}"))?;

    // The answer: a length, then a packet id, then a string length, then the JSON.
    let body = read_framed(&mut stream).map_err(|error| format!("{host}:{port}: {error}"))?;
    let (_, id_used) = read_varint(&body).ok_or("the answer had no packet id")?;
    let (text_len, len_used) =
        read_varint(&body[id_used..]).ok_or("the answer had no string length")?;
    let start = id_used + len_used;
    let end = start
        .checked_add(text_len.max(0) as usize)
        .ok_or("the answer's string length overflowed")?;
    let json = body.get(start..end).ok_or("the answer was shorter than it claimed")?;
    let json = String::from_utf8(json.to_vec())
        .map_err(|_| "the answer was not UTF-8".to_string())?;
    parse_status(&json).ok_or_else(|| format!("{host}:{port} answered with something else"))
}

/// Read one length-prefixed packet: the varint length, then that many bytes.
fn read_framed(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut head = [0u8; 5];
    let mut filled = 0;
    let packet_len = loop {
        let read = stream
            .read(&mut head[filled..filled + 1])
            .map_err(|error| format!("nothing came back: {error}"))?;
        if read == 0 {
            return Err("the server closed without answering".to_owned());
        }
        filled += 1;
        if let Some((length, _)) = read_varint(&head[..filled]) {
            break length;
        }
        if filled == head.len() {
            return Err("the answer's length was not a varint".to_owned());
        }
    };
    let mut body = vec![0u8; packet_len.max(0) as usize];
    stream
        .read_exact(&mut body)
        .map_err(|error| format!("the answer stopped early: {error}"))?;
    Ok(body)
}

/// Split `host:port` when it is written that way.
fn split_address(host: &str, port: u16) -> (String, u16) {
    match host.rsplit_once(':') {
        Some((name, tail)) if !name.is_empty() => match tail.parse::<u16>() {
            Ok(parsed) => (name.to_owned(), parsed),
            Err(_) => (host.to_owned(), port),
        },
        _ => (host.to_owned(), port),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip_at_the_sizes_the_protocol_uses() {
        for value in [0, 1, 127, 128, 255, 769, 25565, 2_097_151] {
            let encoded = varint(value);
            let (decoded, used) = read_varint(&encoded).expect("a readable varint");
            assert_eq!(decoded, value, "{value} did not survive a round trip");
            assert_eq!(used, encoded.len(), "the length read is the length written");
        }
        // The protocol's own example: 300 is two bytes.
        assert_eq!(varint(300), vec![0xac, 0x02]);
    }

    #[test]
    fn a_varint_stops_at_five_bytes() {
        assert_eq!(read_varint(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x01]), None);
        assert_eq!(read_varint(&[]), None);
    }

    #[test]
    fn the_request_names_the_server_and_asks_for_status() {
        let bytes = request("mc.example.com", 25565);
        // Frame one: the handshake.
        let (length, used) = read_varint(&bytes).expect("a length");
        let first = &bytes[used..used + length as usize];
        let (id, id_len) = read_varint(first).expect("a packet id");
        assert_eq!(id, 0, "the handshake is packet 0");
        let text = String::from_utf8_lossy(&first[id_len..]);
        assert!(text.contains("mc.example.com"), "the handshake names the server");
        assert!(text.contains("\u{1}"), "the next state is status");

        // Frame two: the status request, which is an id and nothing else.
        let rest = &bytes[used + length as usize..];
        assert_eq!(rest, &[0x01, 0x00], "an empty packet 0: length 1, id 0");
    }

    #[test]
    fn a_motd_loses_its_formatting_and_its_newlines() {
        assert_eq!(plain_motd("\u{a7}6Hello \u{a7}lworld"), "Hello world");
        assert_eq!(plain_motd("line one\nline two"), "line one line two");
    }

    #[test]
    fn the_answer_parses_from_either_shape_of_description() {
        let modern = r#"{"version":{"name":"1.21.4","protocol":769},
            "players":{"online":12,"max":60},"description":"A \u00a7aserver"}"#;
        let parsed = parse_status(modern).expect("a status");
        assert_eq!(parsed.motd, "A server");
        assert_eq!(parsed.version, "1.21.4");
        assert_eq!((parsed.online, parsed.capacity), (12, 60));

        let component = r#"{"version":{"name":"1.20"},"players":{"online":1,"max":2},
            "description":{"text":"Hi ","extra":[{"text":"there"}]}}"#;
        let parsed = parse_status(component).expect("a status");
        assert_eq!(parsed.motd, "Hi there");
    }

    #[test]
    fn something_that_is_not_a_status_is_not_a_status() {
        assert!(parse_status("not json").is_none());
        assert!(parse_status(r#"{"version":{"name":"1.20"}}"#).is_none());
    }

    #[test]
    fn an_address_may_carry_its_own_port() {
        assert_eq!(split_address("mc.example.com:25566", 25565), ("mc.example.com".to_owned(), 25566));
        assert_eq!(split_address("mc.example.com", 25565), ("mc.example.com".to_owned(), 25565));
        assert_eq!(split_address("mc.example.com:notaport", 25565), ("mc.example.com:notaport".to_owned(), 25565));
    }
}
