//! A fake API for the tests: a `wiremock` server answering from the responses
//! recorded in `tests/fixtures/` (`cargo run -p xtask -- record <url>`), so
//! nothing here needs a network or a key.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use wiremock::{MockServer, Request, Respond, ResponseTemplate};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// A path and its query, the query's pairs sorted, so a request is found
/// whatever order its parameters were written in.
fn key(path: &str, query: &str) -> String {
    let mut pairs: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.sort();
    let query: Vec<String> = pairs.into_iter().map(|(k, v)| format!("{k}={v}")).collect();
    format!("{path}?{}", query.join("&"))
}

pub struct Fixtures {
    by_key: BTreeMap<String, Value>,
}

impl Fixtures {
    pub fn load() -> Self {
        let index: BTreeMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(fixtures().join("index.json")).unwrap())
                .unwrap();
        let by_key = index
            .into_iter()
            .map(|(path, file)| {
                let (p, q) = path.split_once('?').unwrap_or((&path, ""));
                let body =
                    serde_json::from_str(&std::fs::read_to_string(fixtures().join(file)).unwrap())
                        .unwrap();
                (key(p, q), body)
            })
            .collect();
        Self { by_key }
    }

    /// A recorded answer, or for a sessions range not recorded as such, one
    /// assembled from the recorded years it spans.
    fn answer(&self, path: &str, query: &str) -> Option<Value> {
        if let Some(v) = self.by_key.get(&key(path, query)) {
            return Some(v.clone());
        }
        if !path.ends_with("/sessions") || !path.starts_with("/v1/venues/") {
            return None;
        }
        let params: BTreeMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        let (from, to) = (params.get("from")?.clone(), params.get("to")?.clone());
        let mut days = Vec::new();
        let mut first = None;
        for year in from[..4].parse::<i32>().ok()?..=to[..4].parse::<i32>().ok()? {
            let whole = self
                .by_key
                .get(&key(path, &format!("from={year}-01-01&to={year}-12-31")))?;
            first.get_or_insert_with(|| whole.clone());
            for d in whole["days"].as_array()? {
                let date = d["date"].as_str()?;
                if from.as_str() <= date && date <= to.as_str() {
                    days.push(d.clone());
                }
            }
        }
        let mut out = first?;
        out["days"] = Value::Array(days);
        Some(out)
    }
}

impl Respond for Fixtures {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let path = request.url.path();
        let query = request.url.query().unwrap_or("");
        match self.answer(path, query) {
            Some(body) => ResponseTemplate::new(200).set_body_json(body),
            // A server with no key store meters nothing, as a local one.
            None if path == "/v1/usage" => ResponseTemplate::new(404).set_body_json(
                serde_json::json!({"error": "not_metered", "message": "no key store"}),
            ),
            None => ResponseTemplate::new(404).set_body_json(
                serde_json::json!({"error": "not_found", "message": format!("{path}?{query}")}),
            ),
        }
    }
}

/// A server answering from the fixtures, at the lowest priority, so a test
/// can mount its own answers in front.
pub async fn server() -> MockServer {
    let server = MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(Fixtures::load())
        .with_priority(u8::MAX)
        .mount(&server)
        .await;
    server
}

pub fn client(server: &MockServer) -> diurn::Client {
    diurn::Client::builder()
        .api_key("diurn_key_test")
        .base_url(server.uri())
        .build()
        .unwrap()
}

pub fn date(s: &str) -> jiff::civil::Date {
    s.parse().unwrap()
}

pub fn instant(s: &str) -> jiff::Timestamp {
    s.parse().unwrap()
}
