//! The API's operations, once: what each sends and what it returns. The async
//! and blocking clients both take their methods from [`operations!`], so the
//! two cannot drift apart.

use diurn_mic::Mic;
use jiff::Timestamp;
use jiff::civil::Date;

use crate::error::{Error, Result};
use crate::generated::models as m;
use crate::request::{Call, Request};

/// Anything that names a venue: a [`Mic`], or a string spelling one
/// (`"XNYS"`, `"xnys"`).
pub trait IntoMic {
    /// The MIC, or [`Error::InvalidMic`].
    fn into_mic(self) -> Result<Mic>;
}

impl IntoMic for Mic {
    fn into_mic(self) -> Result<Mic> {
        Ok(self)
    }
}

impl IntoMic for &Mic {
    fn into_mic(self) -> Result<Mic> {
        Ok(*self)
    }
}

impl IntoMic for &str {
    fn into_mic(self) -> Result<Mic> {
        Mic::new(&self.trim().to_uppercase()).map_err(|_| Error::InvalidMic(self.to_string()))
    }
}

impl IntoMic for String {
    fn into_mic(self) -> Result<Mic> {
        self.as_str().into_mic()
    }
}

impl IntoMic for &String {
    fn into_mic(self) -> Result<Mic> {
        self.as_str().into_mic()
    }
}

pub(crate) fn mics(mics: impl IntoIterator<Item = impl IntoMic>) -> Result<Vec<Mic>> {
    let out = mics
        .into_iter()
        .map(IntoMic::into_mic)
        .collect::<Result<Vec<_>>>()?;
    if out.is_empty() {
        return Err(Error::InvalidArgument("no venues given".into()));
    }
    Ok(out)
}

/// What to do on reaching a day Diurn cannot answer, when stepping.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OnUnknown {
    /// Return [`Error::Indeterminate`], naming the day (the default).
    #[default]
    Error,
    /// Pass over it uncounted, listing it in `unknown_skipped`.
    Skip,
}

/// Whether a joint step needs all the venues trading, or any one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Every venue trades (the default).
    #[default]
    All,
    /// At least one does.
    Any,
}

/// Optional parameters of [`sessions`](crate::Client::sessions).
#[derive(Debug, Clone, Default)]
pub struct SessionsOptions {
    /// Only days of these kinds; all days when empty.
    pub kinds: Vec<m::DayKindDto>,
    /// The evidence behind each day, on plans that include it.
    pub provenance: bool,
    /// Answer from the dataset served at this instant (Enterprise).
    pub as_of: Option<Timestamp>,
}

/// Optional parameters of [`exceptions`](crate::Client::exceptions).
#[derive(Debug, Clone, Default)]
pub struct ExceptionsOptions {
    /// The kinds wanted; by default `holiday`, `early_close`,
    /// `open_for_some` and `unknown`.
    pub kinds: Vec<m::DayKindDto>,
    /// Answer from the dataset served at this instant (Enterprise).
    pub as_of: Option<Timestamp>,
}

/// Optional parameters of a step through trading days.
#[derive(Debug, Clone, Default)]
pub struct StepOptions {
    /// Trading days to move; 1 when `None`, at most 366.
    pub n: Option<u32>,
    /// Count the starting date itself when it trades.
    pub inclusive: bool,
    /// What reaching an unknown day does.
    pub on_unknown: OnUnknown,
    /// Answer from the dataset served at this instant (Enterprise).
    pub as_of: Option<Timestamp>,
}

/// Optional parameters of a joint step across venues.
#[derive(Debug, Clone, Default)]
pub struct JointOptions {
    /// Dates to move; 1 when `None`, at most 366.
    pub n: Option<u32>,
    /// All the venues trading, or any one.
    pub mode: Mode,
    /// Count the starting date itself when it qualifies.
    pub inclusive: bool,
    /// What reaching an unknown date does.
    pub on_unknown: OnUnknown,
    /// Answer from the dataset served at this instant (Enterprise).
    pub as_of: Option<Timestamp>,
}

/// Filters for [`venues`](crate::Client::venues); everything by default.
#[derive(Debug, Clone, Default)]
pub struct VenuesQuery {
    /// Case-insensitive match on MIC, operating MIC, name or acronym.
    pub q: Option<String>,
    /// ISO 3166-1 alpha-2.
    pub country: Option<String>,
    /// `own`, `operating_mic`, `country` or `none`.
    pub resolution: Vec<String>,
    /// Registry statuses (`active`, `updated`, `expired`) or `all`; `active`
    /// by default.
    pub status: Vec<String>,
    /// At most 1,000; 100 by default.
    pub limit: Option<u32>,
    /// `next_cursor` from the previous page.
    pub cursor: Option<String>,
}

// --- the calls --------------------------------------------------------------

pub(crate) fn health() -> Call<m::Health> {
    Call::new(Request::get("/health"))
}

pub(crate) fn venue(mic: Mic) -> Call<m::VenueResponse> {
    Call::new(Request::get(format!("/v1/venues/{mic}")))
}

pub(crate) fn venues(q: VenuesQuery) -> Call<m::VenuesResponse> {
    Call::new(
        Request::get("/v1/venues")
            .param("q", q.q)
            .param("country", q.country)
            .param("resolution", q.resolution)
            .param("status", q.status)
            .param("limit", q.limit)
            .param("cursor", q.cursor),
    )
}

pub(crate) fn usage() -> Call<m::UsageView> {
    Call::new(Request::get("/v1/usage"))
}

pub(crate) fn sessions(
    mic: Mic,
    from: Date,
    to: Date,
    o: SessionsOptions,
) -> Call<m::DaysResponse> {
    Call::new(
        Request::get(format!("/v1/venues/{mic}/sessions"))
            .param("from", from)
            .param("to", to)
            .param("kinds", o.kinds)
            .param("expand", o.provenance.then_some("provenance"))
            .param("as_of", o.as_of),
    )
}

pub(crate) fn day(mic: Mic, date: Date, as_of: Option<Timestamp>) -> Call<m::DayResponse> {
    Call::new(Request::get(format!("/v1/venues/{mic}/days/{date}")).param("as_of", as_of))
}

pub(crate) fn status(
    mic: Mic,
    at: Option<Timestamp>,
    as_of: Option<Timestamp>,
) -> Call<m::StatusResponse> {
    let at = at.unwrap_or_else(Timestamp::now);
    Call::new(
        Request::get(format!("/v1/venues/{mic}/status"))
            .param("at", at)
            .param("as_of", as_of),
    )
}

pub(crate) fn edge(
    mic: Mic,
    which: &str,
    from: Timestamp,
    as_of: Option<Timestamp>,
) -> Call<m::InstantResponse> {
    Call::new(
        Request::get(format!("/v1/venues/{mic}/{which}"))
            .param("from", from)
            .param("as_of", as_of),
    )
}

pub(crate) fn step(mic: Mic, direction: &str, from: Date, o: StepOptions) -> Call<m::StepResponse> {
    Call::new(
        Request::get(format!("/v1/venues/{mic}/trading-days/{direction}"))
            .param("from", from)
            .param("n", o.n)
            .param("inclusive", o.inclusive.then_some(true))
            .param(
                "on_unknown",
                (o.on_unknown != OnUnknown::Error).then_some(o.on_unknown),
            )
            .param("as_of", o.as_of),
    )
}

pub(crate) fn count(
    mic: Mic,
    from: Date,
    to: Date,
    as_of: Option<Timestamp>,
) -> Call<m::CountResponse> {
    Call::new(
        Request::get(format!("/v1/venues/{mic}/trading-days/count"))
            .param("from", from)
            .param("to", to)
            .param("as_of", as_of),
    )
}

pub(crate) fn statuses(
    mics: Vec<Mic>,
    at: Option<Timestamp>,
    as_of: Option<Timestamp>,
) -> Call<m::StatusesResponse> {
    Call::new(
        Request::get("/v1/status")
            .param("mics", mics)
            .param("at", at)
            .param("as_of", as_of)
            .many(),
    )
}

pub(crate) fn exceptions(
    mics: Vec<Mic>,
    from: Date,
    to: Date,
    o: ExceptionsOptions,
) -> Call<m::ExceptionsResponse> {
    Call::new(
        Request::get("/v1/exceptions")
            .param("mics", mics)
            .param("from", from)
            .param("to", to)
            .param("kinds", o.kinds)
            .param("as_of", o.as_of)
            .many(),
    )
}

pub(crate) fn sessions_bulk(
    mics: Vec<Mic>,
    from: Date,
    to: Date,
    as_of: Option<Timestamp>,
) -> Call<m::BulkResponse> {
    Call::new(
        Request::get("/v1/sessions")
            .param("mics", mics)
            .param("from", from)
            .param("to", to)
            .param("as_of", as_of)
            .many(),
    )
}

pub(crate) fn trading_days(
    mics: Vec<Mic>,
    from: Date,
    to: Date,
    as_of: Option<Timestamp>,
) -> Call<m::TradingDaysResponse> {
    Call::new(
        Request::get("/v1/trading-days")
            .param("mics", mics)
            .param("from", from)
            .param("to", to)
            .param("as_of", as_of)
            .many(),
    )
}

pub(crate) fn joint(
    mics: Vec<Mic>,
    direction: &str,
    from: Date,
    o: JointOptions,
) -> Call<m::JointStepResponse> {
    Call::new(
        Request::get(format!("/v1/trading-days/{direction}"))
            .param("mics", mics)
            .param("from", from)
            .param("n", o.n)
            .param("mode", (o.mode != Mode::All).then_some(o.mode))
            .param("inclusive", o.inclusive.then_some(true))
            .param(
                "on_unknown",
                (o.on_unknown != OnUnknown::Error).then_some(o.on_unknown),
            )
            .param("as_of", o.as_of)
            .many(),
    )
}

// The server's defaults (`inclusive=false`, `on_unknown=error`, `mode=all`)
// are left out of a request rather than spelled.

/// Every operation, as one list: `fn name(args) -> Answer = call;`. A client
/// passes the name of a macro that turns the list into its methods.
macro_rules! operations {
    ($emit:ident) => {
        $emit! {
            /// Liveness, and which dataset the server is serving. Free.
            fn health() -> m::Health = ops::health();
            /// The ISO 10383 record, with the venue's zone, how it is answered,
            /// and its product schedules. Free and unmetered.
            fn venue(mic: impl IntoMic) -> m::VenueResponse = ops::venue(mic.into_mic()?);
            /// List and search venues, a page at a time. Free and unmetered.
            fn venues(query: VenuesQuery) -> m::VenuesResponse = ops::venues(query);
            /// Your usage against your plan. Needs a key; never metered.
            fn usage() -> m::UsageView = ops::usage();
            /// Every day from `from` to `to`, inclusive, with its sessions.
            fn sessions(mic: impl IntoMic, from: Date, to: Date, options: SessionsOptions) -> m::DaysResponse
                = ops::sessions(mic.into_mic()?, from, to, options);
            /// One day, with its provenance in full on plans that include it.
            fn day(mic: impl IntoMic, date: Date, as_of: Option<Timestamp>) -> m::DayResponse
                = ops::day(mic.into_mic()?, date, as_of);
            /// Whether the venue is trading at an instant (now, when `at` is `None`).
            fn status(mic: impl IntoMic, at: Option<Timestamp>, as_of: Option<Timestamp>) -> m::StatusResponse
                = ops::status(mic.into_mic()?, at, as_of);
            /// The next instant the venue starts trading, after `from`.
            fn next_open(mic: impl IntoMic, from: Timestamp, as_of: Option<Timestamp>) -> m::InstantResponse
                = ops::edge(mic.into_mic()?, "next-open", from, as_of);
            /// The next instant the venue stops trading, after `from`.
            fn next_close(mic: impl IntoMic, from: Timestamp, as_of: Option<Timestamp>) -> m::InstantResponse
                = ops::edge(mic.into_mic()?, "next-close", from, as_of);
            /// The `n`th trading day after `from`, with the closures passed over.
            fn next_trading_day(mic: impl IntoMic, from: Date, options: StepOptions) -> m::StepResponse
                = ops::step(mic.into_mic()?, "next", from, options);
            /// The `n`th trading day before `from`.
            fn previous_trading_day(mic: impl IntoMic, from: Date, options: StepOptions) -> m::StepResponse
                = ops::step(mic.into_mic()?, "previous", from, options);
            /// Trading days from `from` to `to`, inclusive; a lower bound where
            /// `unknown` is above zero.
            fn count_trading_days(mic: impl IntoMic, from: Date, to: Date, as_of: Option<Timestamp>) -> m::CountResponse
                = ops::count(mic.into_mic()?, from, to, as_of);
            /// Whether several venues are trading at an instant (now, when `at` is `None`).
            fn statuses(mics: impl IntoIterator<Item = impl IntoMic>, at: Option<Timestamp>, as_of: Option<Timestamp>)
                -> m::StatusesResponse = ops::statuses(ops::mics(mics)?, at, as_of);
            /// Holidays and irregular days across venues, one row each, by date then MIC.
            fn exceptions(mics: impl IntoIterator<Item = impl IntoMic>, from: Date, to: Date, options: ExceptionsOptions)
                -> m::ExceptionsResponse = ops::exceptions(ops::mics(mics)?, from, to, options);
            /// Many venues over a range, column-oriented.
            fn sessions_bulk(mics: impl IntoIterator<Item = impl IntoMic>, from: Date, to: Date, as_of: Option<Timestamp>)
                -> m::BulkResponse = ops::sessions_bulk(ops::mics(mics)?, from, to, as_of);
            /// A date-and-boolean series per venue; `None` where it cannot say.
            fn trading_days(mics: impl IntoIterator<Item = impl IntoMic>, from: Date, to: Date, as_of: Option<Timestamp>)
                -> m::TradingDaysResponse = ops::trading_days(ops::mics(mics)?, from, to, as_of);
            /// The `n`th date after `from` on which all (or any) of the venues trade.
            fn joint_next_trading_day(mics: impl IntoIterator<Item = impl IntoMic>, from: Date, options: JointOptions)
                -> m::JointStepResponse = ops::joint(ops::mics(mics)?, "next", from, options);
            /// The `n`th date before `from` on which all (or any) of the venues trade.
            fn joint_previous_trading_day(mics: impl IntoIterator<Item = impl IntoMic>, from: Date, options: JointOptions)
                -> m::JointStepResponse = ops::joint(ops::mics(mics)?, "previous", from, options);
        }
    };
}

pub(crate) use operations;
