//! JSON serialization compatible with `QJsonDocument::toJson(Indented)`.
//!
//! QJsonDocument emits 4-space indentation, object keys in alphabetical
//! order (QJsonObject keeps its keys sorted) and a trailing newline. The
//! default `serde_json` map is a `BTreeMap`, so building values with the
//! default crate features already yields alphabetical keys; this module
//! contributes the exact indentation and trailing newline.

use crate::error::{Error, Result};
use serde_json::ser::Formatter;
use serde_json::Value;
use std::io::Write;

/// Serialize a JSON value exactly like `QJsonDocument::toJson(QJsonDocument::Indented)`:
/// 4-space indent, sorted keys (guaranteed by `BTreeMap`), one array element
/// per line, trailing newline.
pub fn to_document_string(value: &Value) -> Result<String> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, QJsonFormatter::default());
        serde::Serialize::serialize(value, &mut ser)
            .map_err(|e| Error::json("<memory>", e.to_string()))?;
    }
    buf.push(b'\n');
    String::from_utf8(buf).map_err(|e| Error::json("<memory>", e.to_string()))
}

/// Parse JSON text into a [`Value`].
pub fn parse(text: &str) -> Result<Value> {
    serde_json::from_str(text).map_err(|e| Error::json("<memory>", e.to_string()))
}

#[derive(Default)]
struct QJsonFormatter {
    depth: usize,
    has_value: Vec<bool>,
}

fn write_indent<W: ?Sized + Write>(w: &mut W, depth: usize) -> std::io::Result<()> {
    for _ in 0..depth {
        w.write_all(b"    ")?;
    }
    Ok(())
}

impl Formatter for QJsonFormatter {
    fn begin_object<W: ?Sized + Write>(&mut self, w: &mut W) -> std::io::Result<()> {
        self.depth += 1;
        self.has_value.push(false);
        w.write_all(b"{")
    }

    fn end_object<W: ?Sized + Write>(&mut self, w: &mut W) -> std::io::Result<()> {
        self.depth -= 1;
        let had = self.has_value.pop();
        if had == Some(true) {
            w.write_all(b"\n")?;
            write_indent(w, self.depth)?;
        }
        w.write_all(b"}")
    }

    fn begin_object_key<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> std::io::Result<()> {
        if let Some(slot) = self.has_value.last_mut() {
            *slot = true;
        }
        if first {
            w.write_all(b"\n")?;
        } else {
            w.write_all(b",\n")?;
        }
        write_indent(w, self.depth)
    }

    fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> std::io::Result<()> {
        w.write_all(b": ")
    }

    fn begin_array<W: ?Sized + Write>(&mut self, w: &mut W) -> std::io::Result<()> {
        self.depth += 1;
        self.has_value.push(false);
        w.write_all(b"[")
    }

    fn end_array<W: ?Sized + Write>(&mut self, w: &mut W) -> std::io::Result<()> {
        self.depth -= 1;
        let had = self.has_value.pop();
        if had == Some(true) {
            w.write_all(b"\n")?;
            write_indent(w, self.depth)?;
        }
        w.write_all(b"]")
    }

    fn begin_array_value<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> std::io::Result<()> {
        if let Some(slot) = self.has_value.last_mut() {
            *slot = true;
        }
        if first {
            w.write_all(b"\n")?;
        } else {
            w.write_all(b",\n")?;
        }
        write_indent(w, self.depth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn output_matches_qjsondocument_layout() {
        let v = json!({
            "formatVersion": 1,
            "components": [
                { "uid": "net.minecraft", "important": true },
                { "uid": "org.lwjgl3" }
            ],
            "empty_obj": {},
            "empty_arr": []
        });
        let out = to_document_string(&v).unwrap();
        let expected = concat!(
            "{\n",
            "    \"components\": [\n",
            "        {\n",
            "            \"important\": true,\n",
            "            \"uid\": \"net.minecraft\"\n",
            "        },\n",
            "        {\n",
            "            \"uid\": \"org.lwjgl3\"\n",
            "        }\n",
            "    ],\n",
            "    \"empty_arr\": [],\n",
            "    \"empty_obj\": {},\n",
            "    \"formatVersion\": 1\n",
            "}\n"
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn keys_are_alphabetical_like_qjsonobject() {
        let v = json!({ "zebra": 1, "apple": 2, "Mango": 3 });
        let out = to_document_string(&v).unwrap();
        let order: Vec<&str> = ["\"Mango\"", "\"apple\"", "\"zebra\""]
            .into_iter()
            .filter(|k| out.contains(k))
            .collect();
        assert_eq!(order, ["\"Mango\"", "\"apple\"", "\"zebra\""]);
        assert!(out.find("\"Mango\"").unwrap() < out.find("\"apple\"").unwrap());
        assert!(out.find("\"apple\"").unwrap() < out.find("\"zebra\"").unwrap());
    }

    #[test]
    fn parse_rejects_garbage_with_message() {
        assert!(parse("{not json").is_err());
        let v = parse("{\"a\": 1}").unwrap();
        assert_eq!(v["a"], json!(1));
    }
}
