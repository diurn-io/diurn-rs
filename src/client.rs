//! The async client: one method per API operation.

use std::ops::RangeInclusive;
use std::sync::Mutex;

use jiff::Timestamp;
use jiff::civil::Date;
use serde::de::DeserializeOwned;

use crate::calendar::{Calendar, CalendarOptions, chunks};
use crate::error::{Error, Result};
use crate::generated::models as m;
use crate::ops::{
    self, ExceptionsOptions, IntoMic, JointOptions, SessionsOptions, StepOptions, VenuesQuery,
};
use crate::request::{Call, ClientBuilder, Config, Method, Raw, USER_AGENT, finish, url};

/// A connection to the Diurn API, async.
///
/// ```no_run
/// # async fn run() -> diurn::Result<()> {
/// let client = diurn::Client::new()?; // DIURN_API_KEY, DIURN_API_URL
/// let now = client.statuses(["XNYS", "XLON", "XTKS"], None, None).await?;
/// # Ok(()) }
/// ```
///
/// Every endpoint but the registry record, `health` and the venue list needs
/// a key; a free one is at <https://diurn.io/signin/>.
///
/// **Retries** are off by default. With
/// [`ClientBuilder::retries`], a client retries what waiting can fix: a rate
/// limit, after the server's `Retry-After`; a 503 or a failed connection,
/// backing off from half a second. A plan's refusal (a MIC cap, a date
/// outside the window) is never retried, and a wait longer than
/// [`ClientBuilder::max_wait`] is returned as the error rather than slept.
#[derive(Debug)]
pub struct Client {
    http: reqwest::Client,
    config: Config,
    last: Mutex<Vec<(String, String)>>,
}

impl Client {
    /// A client from the environment: `DIURN_API_KEY` and `DIURN_API_URL`.
    pub fn new() -> Result<Self> {
        ClientBuilder::default().build()
    }

    /// A client with settings of its own.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// The headers of the last response, lower-cased: the usage headers
    /// (`diurn-mics-day`, `ratelimit-remaining`, ...) and `x-diurn-dataset`.
    pub fn last_headers(&self) -> Vec<(String, String)> {
        self.last.lock().map(|h| h.clone()).unwrap_or_default()
    }

    async fn run<T: DeserializeOwned>(&self, call: Call<T>) -> Result<T> {
        let mut attempt = 0;
        loop {
            let (outcome, raw) = match self.send(&call).await {
                Ok(raw) => (finish(&raw), Some(raw)),
                Err(e) => (Err(e), None),
            };
            match outcome {
                Ok(answer) => return Ok(answer),
                Err(error) => match self.config.policy.wait(&error, attempt, raw.as_ref()) {
                    Some(wait) => {
                        tokio::time::sleep(wait).await;
                        attempt += 1;
                    }
                    None => return Err(error),
                },
            }
        }
    }

    async fn send<T>(&self, call: &Call<T>) -> Result<Raw> {
        let request = &call.request;
        let mut builder = match request.method {
            Method::Get => self.http.get(url(&self.config, request)?),
            Method::Post => self
                .http
                .post(url(&self.config, request)?)
                .header("content-type", "application/json")
                .body(request.body()),
        }
        .header("accept", "application/json");
        if let Some(key) = &self.config.api_key {
            builder = builder.bearer_auth(key);
        }
        let response = builder
            .send()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        let status = response.status().as_u16();
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_ascii_lowercase(),
                    v.to_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        let body = response
            .bytes()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?
            .to_vec();
        if let Ok(mut last) = self.last.lock() {
            last.clone_from(&headers);
        }
        Ok(Raw {
            status,
            headers,
            body,
        })
    }

    /// A venue's calendar over a range of dates, loaded once: one call for
    /// the venue's record and one per 1,830 days of sessions. Every question
    /// a [`Calendar`] answers is then answered from what was loaded, without
    /// another request.
    pub async fn calendar(
        &self,
        mic: impl IntoMic,
        range: RangeInclusive<Date>,
    ) -> Result<Calendar> {
        self.calendar_with(mic, range, CalendarOptions::default())
            .await
    }

    /// [`calendar`](Self::calendar), with a product schedule or an `as_of`.
    pub async fn calendar_with(
        &self,
        mic: impl IntoMic,
        range: RangeInclusive<Date>,
        options: CalendarOptions,
    ) -> Result<Calendar> {
        let mic = mic.into_mic()?;
        let record = self.venue(mic).await?;
        let mut days = Vec::new();
        for (from, to) in chunks(&range)? {
            let sessions = SessionsOptions {
                as_of: options.as_of,
                ..SessionsOptions::default()
            };
            days.extend(self.sessions(mic, from, to, sessions).await?.days);
        }
        Calendar::assemble(record, days, range, options.product)
    }
}

impl ClientBuilder {
    /// The async client.
    pub fn build(self) -> Result<Client> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(self.config.timeout)
            .build()
            .map_err(|e| Error::Transport(e.to_string()))?;
        Ok(Client {
            http,
            config: self.config,
            last: Mutex::new(Vec::new()),
        })
    }
}

macro_rules! async_methods {
    ($( $(#[$doc:meta])* fn $name:ident($($arg:ident: $ty:ty),*) -> $ret:ty = $call:expr; )*) => {
        impl Client {
            $(
                $(#[$doc])*
                pub async fn $name(&self, $($arg: $ty),*) -> Result<$ret> {
                    let call: Call<$ret> = $call;
                    self.run(call).await
                }
            )*
        }
    };
}

ops::operations!(async_methods);
