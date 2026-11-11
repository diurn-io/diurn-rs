//! Tasks for working on diurn-rs.
//!
//! ```sh
//! cargo run -p xtask -- generate          # rewrite src/generated/models.rs
//! cargo run -p xtask -- generate --check  # exit 1 if it would change (CI)
//! cargo run -p xtask -- record http://127.0.0.1:8099   # re-record tests/fixtures
//! ```
//!
//! **The wire layer is generated and committed** (SPEC §7.1): one struct per
//! object schema in `openapi.json`, one enum per string enum, `serde` doing the
//! parsing. The rules are diurn-py's `tools/generate.py`, in Rust. Refresh the
//! document first, from a diurn-platform checkout:
//!
//! ```sh
//! cargo run -p diurn-api -- --openapi > ../diurn-rs/openapi.json
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Map, Value};

/// Instants, which the document declares only as strings. The server writes
/// the first kind with its IANA zone in brackets (RFC 9557), which
/// `jiff::Zoned` keeps; the second in UTC. A field missing here stays a
/// `String`, and the fixture test that deserialises every recorded response
/// would show it.
const INSTANTS: &[(&str, &str, &str)] = &[
    ("BreakDto", "start", "jiff::Zoned"),
    ("BreakDto", "end", "jiff::Zoned"),
    ("SessionDto", "open", "jiff::Zoned"),
    ("SessionDto", "close", "jiff::Zoned"),
    ("RunningSession", "open", "jiff::Zoned"),
    ("RunningSession", "close", "jiff::Zoned"),
    ("InstantResponse", "at", "jiff::Zoned"),
    ("StatusResponse", "next_open", "jiff::Zoned"),
    ("StatusResponse", "next_close", "jiff::Zoned"),
    ("VenueStatus", "next_open", "jiff::Zoned"),
    ("VenueStatus", "next_close", "jiff::Zoned"),
    ("InstantResponse", "from", "jiff::Timestamp"),
    ("StatusResponse", "at", "jiff::Timestamp"),
    ("StatusesResponse", "at", "jiff::Timestamp"),
    ("StatusesBody", "at", "jiff::Timestamp"),
    ("DatasetServed", "served_from", "jiff::Timestamp"),
    ("DatasetServed", "served_until", "jiff::Timestamp"),
    ("MicWindow", "resets_at", "jiff::Timestamp"),
    ("VenueDays", "resets_at", "jiff::Timestamp"),
    ("Health", "built_at", "jiff::Timestamp"),
];

/// Every `as_of`, in a request or a response, is a UTC instant.
const AS_OF: &str = "as_of";

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "gen",
];

struct Field {
    json: String,
    ty: String,
    /// Required and not null: the type as is. Otherwise an `Option`, or for an
    /// array an empty `Vec` (the API omits `sessions` on a day without them).
    shape: Shape,
    doc: String,
}

#[derive(PartialEq)]
enum Shape {
    Required,
    Optional,
    EmptyVec,
}

struct Model {
    doc: String,
    fields: Vec<Field>,
}

struct Generator<'a> {
    schemas: &'a Map<String, Value>,
    enums: BTreeMap<String, (String, Vec<String>)>,
    models: BTreeMap<String, Model>,
}

fn camel(name: &str) -> String {
    name.split(['_', '-'])
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect()
}

fn ref_name(r: &str) -> &str {
    r.rsplit('/').next().unwrap_or(r)
}

fn rust_field(name: &str) -> String {
    if RUST_KEYWORDS.contains(&name) {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

/// A description as rustdoc reads it, safely: the first paragraph only (the
/// rest is often about the server), links and HTML-like text escaped so
/// `-D warnings` holds, and no indentation that would make a doctest.
fn doc(text: Option<&Value>) -> String {
    let text = text.and_then(Value::as_str).unwrap_or("").trim();
    let first = text.split("\n\n").next().unwrap_or("");
    let lines: Vec<&str> = first.lines().map(str::trim).collect();
    let joined = lines.join("\n");
    let mut out = String::new();
    for (i, part) in joined.split('`').enumerate() {
        if i > 0 {
            out.push('`');
        }
        if i % 2 == 1 {
            out.push_str(part); // inside backticks: as written
            continue;
        }
        out.push_str(&escape_prose(part));
    }
    out
}

fn escape_prose(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        let next_url = ["https://", "http://"]
            .iter()
            .filter_map(|p| rest.find(p))
            .min();
        let (before, url) = match next_url {
            Some(at) => (&rest[..at], Some(at)),
            None => (rest, None),
        };
        for c in before.chars() {
            match c {
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '[' => out.push_str("\\["),
                ']' => out.push_str("\\]"),
                _ => out.push(c),
            }
        }
        match url {
            Some(at) => {
                let tail = &rest[at..];
                let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
                let mut link = &tail[..end];
                while link.ends_with(['.', ',', ';', ':', ')']) {
                    link = &link[..link.len() - 1];
                }
                let _ = write!(out, "<{link}>");
                rest = &tail[link.len()..];
            }
            None => rest = "",
        }
    }
    out
}

impl<'a> Generator<'a> {
    fn new(schemas: &'a Map<String, Value>) -> Self {
        Self {
            schemas,
            enums: BTreeMap::new(),
            models: BTreeMap::new(),
        }
    }

    /// A schema without `null`, saying whether it was there.
    fn nullable(s: &Value) -> (Value, bool) {
        if let Some(Value::Array(types)) = s.get("type") {
            if types.iter().any(|t| t == "null") {
                let rest: Vec<Value> = types.iter().filter(|t| *t != "null").cloned().collect();
                let mut out = s.clone();
                out["type"] = if rest.len() == 1 {
                    rest[0].clone()
                } else {
                    Value::Array(rest)
                };
                return (out, true);
            }
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(Value::Array(options)) = s.get(key) {
                let kept: Vec<&Value> = options
                    .iter()
                    .filter(|o| o.get("type").is_none_or(|t| t != "null"))
                    .collect();
                if kept.len() == 1 && kept.len() < options.len() {
                    return (kept[0].clone(), true);
                }
            }
        }
        (s.clone(), false)
    }

    fn resolve(&mut self, s: &Value, owner: &str, prop: &str) -> String {
        if let Some(r) = s.get("$ref").and_then(Value::as_str) {
            return ref_name(r).to_string();
        }
        if let Some(Value::Array(values)) = s.get("enum") {
            let alias = format!("{owner}{}", camel(prop));
            let values = values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            self.enums
                .insert(alias.clone(), (doc(s.get("description")), values));
            return alias;
        }
        match s.get("type").and_then(Value::as_str) {
            Some("string") => {
                if let Some(&(_, _, ty)) =
                    INSTANTS.iter().find(|(o, p, _)| *o == owner && *p == prop)
                {
                    return ty.to_string();
                }
                match s.get("format").and_then(Value::as_str) {
                    Some("date") => "jiff::civil::Date".into(),
                    Some("date-time") => "jiff::Timestamp".into(),
                    _ if prop == AS_OF => "jiff::Timestamp".into(),
                    _ => "String".into(),
                }
            }
            Some("integer") => "i64".into(),
            Some("number") => "f64".into(),
            Some("boolean") => "bool".into(),
            Some("array") => {
                let item = self.resolve_maybe_null(&s["items"], owner, prop);
                format!("Vec<{item}>")
            }
            Some("object")
                if s.get("additionalProperties").is_some() && s.get("properties").is_none() =>
            {
                let value = &s["additionalProperties"];
                if value == &Value::Bool(true) || value.as_object().is_some_and(Map::is_empty) {
                    "std::collections::BTreeMap<String, serde_json::Value>".into()
                } else {
                    let v = self.resolve_maybe_null(value, owner, prop);
                    format!("std::collections::BTreeMap<String, {v}>")
                }
            }
            Some("object") if s.get("properties").is_some() => {
                let name = format!("{owner}{}", camel(prop));
                self.model(&name, s);
                name
            }
            _ => "serde_json::Value".into(),
        }
    }

    fn resolve_maybe_null(&mut self, s: &Value, owner: &str, prop: &str) -> String {
        let (inner, null) = Self::nullable(s);
        let ty = self.resolve(&inner, owner, prop);
        if null { format!("Option<{ty}>") } else { ty }
    }

    /// Properties and required keys, merging `allOf`.
    fn properties(&self, s: &Value) -> (Vec<(String, Value)>, Vec<String>) {
        if let Some(Value::Array(parts)) = s.get("allOf") {
            let mut props = Vec::new();
            let mut required = Vec::new();
            for part in parts {
                let part = match part.get("$ref").and_then(Value::as_str) {
                    Some(r) => &self.schemas[ref_name(r)],
                    None => part,
                };
                let (p, r) = self.properties(part);
                for (k, v) in p {
                    props.retain(|(existing, _): &(String, Value)| *existing != k);
                    props.push((k, v));
                }
                required.extend(r);
            }
            return (props, required);
        }
        let props = s
            .get("properties")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let required = s
            .get("required")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        (props, required)
    }

    fn model(&mut self, name: &str, s: &Value) {
        if self.models.contains_key(name) {
            return;
        }
        self.models.insert(
            name.to_string(),
            Model {
                doc: doc(s.get("description")),
                fields: vec![],
            },
        );
        let (props, required) = self.properties(s);
        let mut fields = Vec::new();
        for (key, prop) in props {
            let (inner, null) = Self::nullable(&prop);
            let ty = self.resolve(&inner, name, &key);
            let is_array = inner.get("type").and_then(Value::as_str) == Some("array");
            let shape = if required.contains(&key) && !null {
                Shape::Required
            } else if is_array && !null {
                Shape::EmptyVec
            } else {
                Shape::Optional
            };
            fields.push(Field {
                json: key,
                ty,
                shape,
                doc: doc(prop.get("description")),
            });
        }
        if let Some(m) = self.models.get_mut(name) {
            m.fields = fields;
        }
    }

    fn run(mut self) -> String {
        let names: Vec<String> = self.schemas.keys().cloned().collect();
        for name in names {
            let s = self.schemas[&name].clone();
            if let Some(Value::Array(values)) = s.get("enum") {
                let values = values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect();
                self.enums.insert(name, (doc(s.get("description")), values));
            } else {
                self.model(&name, &s);
            }
        }
        self.render()
    }

    fn render(&self) -> String {
        let mut out = String::from(
            "// Generated from openapi.json by `cargo run -p xtask -- generate`. Do not edit.\n\
             //\n\
             // The API's request and response shapes, one per schema. Instants are\n\
             // `jiff` types (see INSTANTS in xtask/src/main.rs); dates are\n\
             // `jiff::civil::Date`.\n\n\
             #![allow(missing_docs)]\n\n\
             use serde::{Deserialize, Serialize};\n",
        );
        for (name, (description, values)) in &self.enums {
            out.push('\n');
            write_doc(&mut out, description, "");
            out.push_str(
                "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]\n\
                 #[non_exhaustive]\n",
            );
            let _ = writeln!(out, "pub enum {name} {{");
            for v in values {
                let _ = writeln!(out, "    #[serde(rename = \"{v}\")]\n    {},", camel(v));
            }
            out.push_str(
                "    /// A value this version of the crate does not know: the API added it\n    \
                 /// after this release.\n    #[serde(other)]\n    Unrecognised,\n}\n",
            );
        }
        for (name, model) in &self.models {
            out.push('\n');
            write_doc(&mut out, &model.doc, "");
            out.push_str(
                "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n#[non_exhaustive]\n",
            );
            let _ = writeln!(out, "pub struct {name} {{");
            for f in &model.fields {
                write_doc(&mut out, &f.doc, "    ");
                let field = rust_field(&f.json);
                if field != f.json {
                    let _ = writeln!(out, "    #[serde(rename = \"{}\")]", f.json);
                }
                let ty = match f.shape {
                    Shape::Required => f.ty.clone(),
                    Shape::EmptyVec => {
                        out.push_str(
                            "    #[serde(default, skip_serializing_if = \"Vec::is_empty\")]\n",
                        );
                        f.ty.clone()
                    }
                    Shape::Optional => {
                        out.push_str(
                            "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n",
                        );
                        format!("Option<{}>", f.ty)
                    }
                };
                let _ = writeln!(out, "    pub {field}: {ty},");
            }
            out.push_str("}\n");
        }
        out
    }
}

fn write_doc(out: &mut String, text: &str, indent: &str) {
    for line in text.lines() {
        if line.is_empty() {
            let _ = writeln!(out, "{indent}///");
        } else {
            let _ = writeln!(out, "{indent}/// {line}");
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root")
        .to_path_buf()
}

fn generate(check: bool) -> ExitCode {
    let root = root();
    let spec: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("openapi.json")).expect("openapi.json is readable"),
    )
    .expect("openapi.json is JSON");
    let schemas = spec["components"]["schemas"]
        .as_object()
        .expect("components.schemas");
    let text = Generator::new(schemas).run();
    let out = root.join("src/generated/models.rs");
    if check {
        if std::fs::read_to_string(&out).ok().as_deref() != Some(text.as_str()) {
            eprintln!("src/generated/models.rs is stale: run cargo run -p xtask -- generate");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }
    std::fs::write(&out, text).expect("models.rs is writable");
    println!("wrote src/generated/models.rs");
    ExitCode::SUCCESS
}

/// The responses the tests replay, recorded from a local diurn-api (no key).
const RECORD: &[&str] = &[
    "/health",
    "/v1/venues/XNYS",
    "/v1/venues/XTKS",
    "/v1/venues/XHKG",
    "/v1/venues/XCEC",
    "/v1/venues/XNSE",
    "/v1/venues/XNYS/sessions?from=2026-01-01&to=2026-12-31",
    "/v1/venues/XNYS/sessions?from=2027-01-01&to=2027-12-31",
    "/v1/venues/XTKS/sessions?from=2026-01-01&to=2026-12-31",
    "/v1/venues/XHKG/sessions?from=2026-01-01&to=2026-12-31",
    "/v1/venues/XCEC/sessions?from=2026-01-01&to=2026-12-31",
    "/v1/venues/XNSE/sessions?from=2026-01-01&to=2026-12-31",
    "/v1/venues/XNYS/status?at=2026-11-27T15:00:00Z",
    "/v1/venues/XNYS/next-close?from=2026-11-27T15:00:00Z",
    "/v1/venues/XNYS/trading-days/next?from=2026-11-25",
    "/v1/venues/XNYS/trading-days/count?from=2026-11-01&to=2026-11-30",
    "/v1/venues/XNYS/days/2026-11-26",
    "/v1/status?mics=XNYS,XLON&at=2026-11-26T15:00:00Z",
    "/v1/exceptions?mics=XNYS,XLON&from=2026-11-20&to=2026-12-31",
    "/v1/trading-days?mics=XNYS,XLON&from=2026-11-20&to=2026-11-30",
    "/v1/sessions?mics=XNYS,XLON&from=2026-11-20&to=2026-11-30",
    "/v1/trading-days/next?mics=XNYS,XLON&from=2026-12-23&n=2",
    "/v1/venues?country=JP&limit=5",
];

/// The fixture file for a request path; the tests use the same rule.
fn fixture_name(path: &str) -> String {
    let mut name = path
        .trim_matches('/')
        .replace('/', "_")
        .replace('?', "__")
        .replace('&', "_");
    name = name.replace('=', "-").replace(':', "").replace(',', "+");
    format!("{name}.json")
}

fn record(base: &str) -> ExitCode {
    let out = root().join("tests/fixtures");
    let mut index = serde_json::Map::new();
    for path in RECORD {
        let body: Value = match ureq::get(&format!("{}{path}", base.trim_end_matches('/'))).call() {
            Ok(mut r) => {
                serde_json::from_str(&r.body_mut().read_to_string().expect("a body")).expect("JSON")
            }
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::FAILURE;
            }
        };
        let text = serde_json::to_string_pretty(&body).expect("serialises") + "\n";
        std::fs::write(out.join(fixture_name(path)), text).expect("writable");
        index.insert((*path).to_string(), Value::String(fixture_name(path)));
        println!("recorded {path}");
    }
    // The tests look a request up here, by path and query, in any order.
    let text = serde_json::to_string_pretty(&index).expect("serialises") + "\n";
    std::fs::write(out.join("index.json"), text).expect("writable");
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["generate"] => generate(false),
        ["generate", "--check"] => generate(true),
        ["record", base] => record(base),
        _ => {
            eprintln!("usage: cargo run -p xtask -- generate [--check] | record <base-url>");
            ExitCode::FAILURE
        }
    }
}
