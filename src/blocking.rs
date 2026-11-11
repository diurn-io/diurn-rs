//! The same client without async, for programs with no runtime (the
//! `blocking` feature). Its methods are the async client's, generated from
//! the same list, so the two cannot drift apart.
//!
//! ```no_run
//! # fn run() -> diurn::Result<()> {
//! let client = diurn::blocking::Client::new()?;
//! let next = client.next_trading_day("XNYS", "2026-11-25".parse().unwrap(), Default::default())?;
//! # Ok(()) }
//! ```
//!
//! Do not call it from inside an async runtime: `reqwest`'s blocking client
//! starts one of its own.

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

/// A connection to the Diurn API that blocks. See [`crate::Client`] for the
/// settings, retries and key.
#[derive(Debug)]
pub struct Client {
    http: reqwest::blocking::Client,
    config: Config,
    last: Mutex<Vec<(String, String)>>,
}

impl Client {
    /// A client from the environment: `DIURN_API_KEY` and `DIURN_API_URL`.
    pub fn new() -> Result<Self> {
        ClientBuilder::default().build_blocking()
    }

    /// A client with settings of its own; finish with
    /// [`build_blocking`](ClientBuilder::build_blocking).
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// The headers of the last response, lower-cased.
    pub fn last_headers(&self) -> Vec<(String, String)> {
        self.last.lock().map(|h| h.clone()).unwrap_or_default()
    }

    fn run<T: DeserializeOwned>(&self, call: Call<T>) -> Result<T> {
        let mut attempt = 0;
        loop {
            let (outcome, raw) = match self.send(&call) {
                Ok(raw) => (finish(&raw), Some(raw)),
                Err(e) => (Err(e), None),
            };
            match outcome {
                Ok(answer) => return Ok(answer),
                Err(error) => match self.config.policy.wait(&error, attempt, raw.as_ref()) {
                    Some(wait) => {
                        std::thread::sleep(wait);
                        attempt += 1;
                    }
                    None => return Err(error),
                },
            }
        }
    }

    fn send<T>(&self, call: &Call<T>) -> Result<Raw> {
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

    /// A venue's calendar over a range of dates, loaded once. See
    /// [`crate::Client::calendar`].
    pub fn calendar(&self, mic: impl IntoMic, range: RangeInclusive<Date>) -> Result<Calendar> {
        self.calendar_with(mic, range, CalendarOptions::default())
    }

    /// [`calendar`](Self::calendar), with a product schedule or an `as_of`.
    pub fn calendar_with(
        &self,
        mic: impl IntoMic,
        range: RangeInclusive<Date>,
        options: CalendarOptions,
    ) -> Result<Calendar> {
        let mic = mic.into_mic()?;
        let record = self.venue(mic)?;
        let mut days = Vec::new();
        for (from, to) in chunks(&range)? {
            let sessions = SessionsOptions {
                as_of: options.as_of,
                ..SessionsOptions::default()
            };
            days.extend(self.sessions(mic, from, to, sessions)?.days);
        }
        Calendar::assemble(record, days, range, options.product)
    }
}

impl ClientBuilder {
    /// The blocking client.
    pub fn build_blocking(self) -> Result<Client> {
        let http = reqwest::blocking::Client::builder()
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

macro_rules! blocking_methods {
    ($( $(#[$doc:meta])* fn $name:ident($($arg:ident: $ty:ty),*) -> $ret:ty = $call:expr; )*) => {
        impl Client {
            $(
                $(#[$doc])*
                pub fn $name(&self, $($arg: $ty),*) -> Result<$ret> {
                    let call: Call<$ret> = $call;
                    self.run(call)
                }
            )*
        }
    };
}

ops::operations!(blocking_methods);
