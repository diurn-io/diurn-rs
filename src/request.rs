//! The core both clients share, without I/O: what a request is, how its
//! parameters are spelled, what a response means, and when to retry. The
//! async and blocking clients only send and sleep.

use std::marker::PhantomData;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::{Error, Result};

pub(crate) const DEFAULT_URL: &str = "https://api.diurn.io";
pub(crate) const USER_AGENT: &str = concat!("diurn-rs/", env!("CARGO_PKG_VERSION"));

/// A MIC list longer than this is sent as the POST form, past URL limits.
pub(crate) const POST_ABOVE: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
    Get,
    Post,
}

/// A request: its method, path, and parameters as JSON values, spelled for
/// the query string or the body when it is sent.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    pub method: Method,
    pub path: String,
    pub params: Map<String, Value>,
}

impl Request {
    pub(crate) fn get(path: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            path: path.into(),
            params: Map::new(),
        }
    }

    /// A parameter; `None` and empty lists are left out.
    pub(crate) fn param(mut self, key: &str, value: impl serde::Serialize) -> Self {
        let value = serde_json::to_value(value).unwrap_or(Value::Null);
        let empty =
            matches!(&value, Value::Null) || matches!(&value, Value::Array(a) if a.is_empty());
        if !empty {
            self.params.insert(key.to_string(), value);
        }
        self
    }

    /// GET for a short MIC list, the POST form for a long one.
    pub(crate) fn many(mut self) -> Self {
        let long = self
            .params
            .get("mics")
            .and_then(Value::as_array)
            .is_some_and(|m| m.len() > POST_ABOVE);
        if long {
            self.method = Method::Post;
        }
        self
    }

    /// The query string, for a GET.
    pub(crate) fn query(&self) -> Vec<(String, String)> {
        self.params
            .iter()
            .map(|(k, v)| (k.clone(), query_value(v)))
            .collect()
    }

    /// The JSON body, for a POST.
    pub(crate) fn body(&self) -> Vec<u8> {
        serde_json::to_vec(&self.params).unwrap_or_default()
    }
}

fn query_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(query_value).collect::<Vec<_>>().join(","),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// A request and the type its answer decodes to.
#[derive(Debug, Clone)]
pub(crate) struct Call<T> {
    pub request: Request,
    decode: PhantomData<fn() -> T>,
}

impl<T> Call<T> {
    pub(crate) fn new(request: Request) -> Self {
        Self {
            request,
            decode: PhantomData,
        }
    }
}

/// A response as received: status, lower-cased headers, body.
#[derive(Debug, Clone)]
pub(crate) struct Raw {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Raw {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// What a response means: the decoded answer, or the typed error.
pub(crate) fn finish<T: DeserializeOwned>(raw: &Raw) -> Result<T> {
    let parsed: Option<Value> = serde_json::from_slice(&raw.body).ok();
    if (200..300).contains(&raw.status) {
        let value = parsed
            .ok_or_else(|| Error::Transport(format!("answered {} without JSON", raw.status)))?;
        return serde_json::from_value(value).map_err(|e| Error::Decode(e.to_string()));
    }
    let body = match parsed {
        Some(v @ Value::Object(_)) => v,
        _ if raw.status >= 500 => return Err(Error::Transport(format!("answered {}", raw.status))),
        _ => Value::Object(Map::new()),
    };
    Err(Error::from_response(raw.status, body, retry_after(raw)))
}

pub(crate) fn retry_after(raw: &Raw) -> Option<Duration> {
    raw.header("retry-after")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(Duration::from_secs_f64)
}

/// How a client retries, and how long it waits.
#[derive(Debug, Clone)]
pub(crate) struct Policy {
    pub retries: u32,
    pub max_wait: Duration,
    pub backoff: Duration,
}

impl Policy {
    /// How long to wait before retrying, or `None` to return the error now.
    ///
    /// Retried is what waiting can fix: `rate_limited`, after the server's
    /// `Retry-After`; a 503 or a failed connection, backing off from
    /// `backoff`. A plan's refusal is never retried, and a wait longer than
    /// `max_wait` is returned rather than slept. Every request is a read, so
    /// a retry is always safe.
    pub(crate) fn wait(&self, error: &Error, attempt: u32, last: Option<&Raw>) -> Option<Duration> {
        if attempt >= self.retries {
            return None;
        }
        let told = match error {
            Error::RateLimited { retry_after, .. } => *retry_after,
            Error::Unavailable(_) => last.and_then(retry_after),
            Error::Transport(_) => None,
            _ => return None,
        };
        match told {
            Some(t) if t <= self.max_wait => Some(t),
            Some(_) => None,
            None => Some((self.backoff * 2u32.saturating_pow(attempt)).min(self.max_wait)),
        }
    }
}

/// The settings both clients are built from.
#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub api_key: Option<String>,
    pub base_url: String,
    pub timeout: Duration,
    pub policy: Policy,
}

/// Builds a [`Client`](crate::Client), or with the `blocking` feature a
/// [`blocking::Client`](crate::blocking::Client), from the same settings.
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    pub(crate) config: Config,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            config: Config {
                api_key: std::env::var("DIURN_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty()),
                base_url: std::env::var("DIURN_API_URL")
                    .ok()
                    .filter(|u| !u.is_empty())
                    .unwrap_or_else(|| DEFAULT_URL.to_string()),
                timeout: Duration::from_secs(30),
                policy: Policy {
                    retries: 0,
                    max_wait: Duration::from_secs(60),
                    backoff: Duration::from_millis(500),
                },
            },
        }
    }
}

impl ClientBuilder {
    /// The API key; by default `DIURN_API_KEY`. A free one is at
    /// <https://diurn.io/signin/>.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.config.api_key = Some(key.into());
        self
    }

    /// The API's address; by default `DIURN_API_URL`, else
    /// `https://api.diurn.io`.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.config.base_url = url.into();
        self
    }

    /// How long one request may take; 30 seconds by default.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// How many times to retry what waiting can fix; 0 by default. See
    /// [`Client`](crate::Client) for what is retried.
    pub fn retries(mut self, retries: u32) -> Self {
        self.config.policy.retries = retries;
        self
    }

    /// The longest single wait before a retry; 60 seconds by default. A
    /// server asking for longer gets its error returned instead.
    pub fn max_wait(mut self, max_wait: Duration) -> Self {
        self.config.policy.max_wait = max_wait;
        self
    }

    /// The first wait after a 503 or a failed connection, doubling each time;
    /// half a second by default.
    pub fn backoff(mut self, backoff: Duration) -> Self {
        self.config.policy.backoff = backoff;
        self
    }
}

/// The full URL for a request.
pub(crate) fn url(config: &Config, request: &Request) -> Result<reqwest::Url> {
    let base = config.base_url.trim_end_matches('/');
    let mut url = reqwest::Url::parse(&format!("{base}{}", request.path))
        .map_err(|e| Error::InvalidArgument(format!("base URL {base:?}: {e}")))?;
    if request.method == Method::Get && !request.params.is_empty() {
        url.query_pairs_mut().extend_pairs(request.query());
    }
    Ok(url)
}
