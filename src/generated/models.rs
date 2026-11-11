// Generated from openapi.json by `cargo run -p xtask -- generate`. Do not edit.
//
// The API's request and response shapes, one per schema. Instants are
// `jiff` types (see INSTANTS in xtask/src/main.rs); dates are
// `jiff::civil::Date`.

#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DayKindDto {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "weekend")]
    Weekend,
    #[serde(rename = "holiday")]
    Holiday,
    #[serde(rename = "early_close")]
    EarlyClose,
    #[serde(rename = "open_for_some")]
    OpenForSome,
    #[serde(rename = "unknown")]
    Unknown,
    /// A value this version of the crate does not know: the API added it
    /// after this release.
    #[serde(other)]
    Unrecognised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OverLimitError {
    #[serde(rename = "not_in_plan")]
    NotInPlan,
    #[serde(rename = "out_of_window")]
    OutOfWindow,
    #[serde(rename = "mic_cap_exceeded")]
    MicCapExceeded,
    #[serde(rename = "rate_limited")]
    RateLimited,
    /// A value this version of the crate does not know: the API added it
    /// after this release.
    #[serde(other)]
    Unrecognised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OverLimitLimitKind {
    #[serde(rename = "mics_day")]
    MicsDay,
    #[serde(rename = "mics_month")]
    MicsMonth,
    #[serde(rename = "rate")]
    Rate,
    /// A value this version of the crate does not know: the API added it
    /// after this release.
    #[serde(other)]
    Unrecognised,
}

/// Whether the venue is trading **at an instant** — the intraday axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum StateDto {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "unknown")]
    Unknown,
    /// A value this version of the crate does not know: the API added it
    /// after this release.
    #[serde(other)]
    Unrecognised,
}

/// How much to trust a served day. Always present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
pub enum VerificationDto {
    #[serde(rename = "verified")]
    Verified,
    #[serde(rename = "corroborated")]
    Corroborated,
    #[serde(rename = "inferreddefinitional")]
    Inferreddefinitional,
    #[serde(rename = "inferredpredictive")]
    Inferredpredictive,
    #[serde(rename = "inferredadvisory")]
    Inferredadvisory,
    /// A value this version of the crate does not know: the API added it
    /// after this release.
    #[serde(other)]
    Unrecognised,
}

/// Typed, because SPEC: client-shared-surface requires every client to
/// distinguish auth failure from an unknown calendar from an out-of-range
/// query, and it cannot if the server returns prose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApiError {
    /// A stable machine-readable tag. Clients switch on this, never on prose.
    pub error: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BreakDto {
    pub end: jiff::Zoned,
    pub start: jiff::Zoned,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BulkBody {
    /// RFC 3339. Answer from the dataset Diurn was serving at that instant
    /// (Enterprise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    pub from: jiff::civil::Date,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mics: Vec<String>,
    pub to: jiff::civil::Date,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BulkCalendar {
    /// One character per date, in the order of `dates`.
    pub kinds: String,
    /// Likewise. Present on every row: it is the cheap half of provenance and
    /// the half that decides whether to trust the cell (SPEC: provenance-is-stored-not-shipped).
    pub verification: String,
}

/// Column-oriented: the dates once, then one short code per day per calendar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BulkResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    pub dates: Vec<jiff::civil::Date>,
    pub from: jiff::civil::Date,
    pub legend: Legend,
    pub to: jiff::civil::Date,
    /// One series per requested MIC, keyed by the MIC.
    pub venues: std::collections::BTreeMap<String, BulkCalendar>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CountResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    /// Trading days in the range, inclusive. **A lower bound when `unknown`
    /// is above zero**: those days may trade.
    pub count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    pub from: jiff::civil::Date,
    pub mic: String,
    pub to: jiff::civil::Date,
    /// Days in the range the engine cannot answer.
    pub unknown: i64,
}

/// The dates the plan may ask about, from today (UTC). Null is unbounded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DataWindow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<jiff::civil::Date>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<jiff::civil::Date>,
}

/// The dataset that answered an `as_of` request (SPEC-pricing §5.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DatasetServed {
    /// First twelve hex of the dataset's content id: what `/health` calls
    /// `dataset`, and the same for every build of the same data.
    pub id: String,
    /// True for a model rebuilt after the fact rather than served at the
    /// time. Nothing is rebuilt yet, so this is false.
    pub reconstructed: bool,
    /// When Diurn began serving it.
    pub served_from: jiff::Timestamp,
    /// When the next dataset replaced it; null while it is still served.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub served_until: Option<jiff::Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DayDto {
    pub date: jiff::civil::Date,
    pub kind: DayKindDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub products: Option<ProductSplitDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<SessionDto>,
    pub verification: VerificationDto,
}

/// One day, and with `as_of`, which dataset answered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DayResponse {
    pub date: jiff::civil::Date,
    pub kind: DayKindDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub products: Option<ProductSplitDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<SessionDto>,
    pub verification: VerificationDto,
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DaysResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    pub days: Vec<DayDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExceptionsBody {
    /// RFC 3339. Answer from the dataset Diurn was serving at that instant
    /// (Enterprise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    pub from: jiff::civil::Date,
    /// Day kinds. Default `holiday`, `early_close`, `open_for_some`,
    /// `unknown`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mics: Vec<String>,
    pub to: jiff::civil::Date,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExceptionsResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    /// The days of the kinds asked for, by date and then MIC. No provenance:
    /// ask for the day itself.
    pub days: Vec<VenueDay>,
    pub from: jiff::civil::Date,
    pub to: jiff::civil::Date,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Health {
    /// First twelve hex of the served artifact's SHA-256, or `"built-in"` for
    /// the copy baked into the image.
    pub artifact: String,
    pub built_at: jiff::Timestamp,
    /// First twelve hex of the served dataset's content id: what its data
    /// says, without when it was built. The same for every build of the same
    /// data, which `artifact` is not, and the name the point-in-time archive
    /// uses (SPEC: point-in-time-archive).
    pub dataset: String,
    /// How many MICs this process can answer for by explicit mapping.
    pub mapped_venues: i64,
    /// Publication date of the ISO vintage this process is serving.
    pub registry_published: jiff::civil::Date,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InstantResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    /// `null` when the engine cannot say — past an anchor's range, beyond the
    /// scan horizon, or on a calendar that publishes no hours at all, as
    /// `US-FUTURES-ICE` does. Not the same as "never".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<jiff::Zoned>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    pub from: jiff::Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic: Option<String>,
}

/// A date passed over: the venues that did not trade it, other than for a
/// weekend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct JointSkipped {
    pub closed: std::collections::BTreeMap<String, SkippedDay>,
    pub date: jiff::civil::Date,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct JointStepBody {
    /// RFC 3339. Answer from the dataset Diurn was serving at that instant
    /// (Enterprise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    pub from: jiff::civil::Date,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inclusive: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mics: Vec<String>,
    /// `all` (default) or `any`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_unknown: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct JointStepResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    /// The date reached.
    pub date: jiff::civil::Date,
    pub from: jiff::civil::Date,
    pub mics: Vec<String>,
    /// `all` or `any`.
    pub mode: String,
    pub n: i64,
    /// Dates passed over on which some venue was closed other than for a
    /// weekend.
    pub skipped: Vec<JointSkipped>,
    /// With `on_unknown=skip`: dates passed over uncounted because the answer
    /// for them is unknown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_skipped: Vec<jiff::civil::Date>,
    /// Each venue's kind of day on `date`.
    pub venues: std::collections::BTreeMap<String, DayKindDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Legend {
    pub kinds: std::collections::BTreeMap<String, String>,
    pub verification: std::collections::BTreeMap<String, String>,
}

/// One MIC cap's count (SPEC-pricing §6.4). `max` is null on a plan with
/// every MIC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MicWindow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
    /// The MICs in the window: listed for the 24-hour one, so a caller can
    /// see what holds their slots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mics: Option<Vec<String>>,
    /// When the oldest MIC in the window leaves it, freeing a slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<jiff::Timestamp>,
    pub used: i64,
    /// `rolling_24h` or `rolling_30d`.
    pub window: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OverLimit {
    /// For `out_of_window`: the dates the plan may ask about; null is unbounded
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed: Option<OverLimitAllowed>,
    pub error: OverLimitError,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<OverLimitLimit>,
    pub message: String,
    /// The MICs holding the window's slots
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mics_in_use: Vec<String>,
    /// When a slot frees, the budget renews, or the rate allows another request
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<jiff::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgrade: Option<String>,
}

/// For `out_of_window`: the dates the plan may ask about; null is unbounded
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OverLimitAllowed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<jiff::civil::Date>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<jiff::civil::Date>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OverLimitLimit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<OverLimitLimitKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used: Option<i64>,
}

/// Which products traded on a day that did not treat them alike.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProductSplitDto {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closed: Vec<String>,
    /// What the lists cannot carry — a venue saying some hours will be
    /// announced later, which leaves a group open and its hours unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open: Vec<String>,
}

/// The evidence behind a day. Only ever sent when asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Provenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieved: Option<jiff::civil::Date>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Rate {
    /// The multi-venue endpoints' own rate.
    pub bulk_per_minute: i64,
    pub per_minute: i64,
}

/// How a MIC reaches its answer (SPEC: calendar-membership). The MIC pages
/// state the same.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResolutionDto {
    /// `own`: the venue is answered directly. `operating_mic`: through its
    /// operating MIC. `country`: through the one calendar for its country.
    /// The last two are inferences, not checked claims.
    pub kind: String,
    /// The MIC or country that answers.
    pub via: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RunningSession {
    pub close: jiff::Zoned,
    pub name: String,
    pub open: jiff::Zoned,
    /// Which products these hours are for, in the venue's own words. **Absent
    /// means the whole venue**, which is almost every calendar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub products: Option<String>,
    /// The day this session belongs to, which may not be the day it started on.
    pub trade_date: jiff::civil::Date,
    pub verification: VerificationDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SessionDto {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breaks: Vec<BreakDto>,
    pub close: jiff::Zoned,
    pub name: String,
    /// RFC 3339, in the venue's own zone. A session close is a zoned instant,
    /// never a naive local time.
    pub open: jiff::Zoned,
    /// Which products keep these hours, in the venue's own words. **Absent
    /// means the whole venue**, which is almost every calendar and every
    /// session on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub products: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    pub verification: VerificationDto,
}

/// A day stepped over: not a weekend, and not trading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkippedDay {
    pub date: jiff::civil::Date,
    pub kind: DayKindDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StatusResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    pub at: jiff::Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    /// The civil day the instant falls on, in the venue's zone.
    pub day: DayDto,
    /// Present when the request was keyed by MIC rather than by calendar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_close: Option<jiff::Zoned>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_open: Option<jiff::Zoned>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<RunningSession>,
    /// `running`, `closed`, or `unknown`. **Not a boolean**: a day the engine
    /// cannot compute is not a closed one, and collapsing the two is the
    /// failure that loses trust.
    pub state: StateDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StatusesBody {
    /// RFC 3339. Answer from the dataset Diurn was serving at that instant
    /// (Enterprise).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    /// RFC 3339. Default: now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<jiff::Timestamp>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StatusesResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    pub at: jiff::Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    pub venues: std::collections::BTreeMap<String, VenueStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StepResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    /// The trading day reached.
    pub date: jiff::civil::Date,
    pub from: jiff::civil::Date,
    pub kind: DayKindDto,
    pub mic: String,
    pub n: i64,
    /// Non-trading days passed over that are not weekends: why the answer is
    /// where it is.
    pub skipped: Vec<SkippedDay>,
    /// With `on_unknown=skip`: `unknown` days passed over uncounted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_skipped: Vec<jiff::civil::Date>,
}

/// A date and whether the venue trades on it, and nothing else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TradingDaysResponse {
    /// With `as_of`: the instant asked about. Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<jiff::Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<DatasetServed>,
    /// The dates once, in order, shared by every venue below. Columnar rather
    /// than pairs because the endpoint takes many venues, and pairs would
    /// repeat the dates for each — and because `zip(dates, trading)` is one
    /// line in every language that would ask for this.
    pub dates: Vec<jiff::civil::Date>,
    pub from: jiff::civil::Date,
    pub to: jiff::civil::Date,
    /// One series per requested MIC. See \[`BulkResponse::venues`\] for why they
    /// are not deduplicated.
    pub venues: std::collections::BTreeMap<String, TradingSeries>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TradingSeries {
    /// `true` when any session runs — an early close and a shortened holiday
    /// session both count, because the venue does trade.
    pub trading: Vec<Option<bool>>,
    /// How many entries above are `null`.
    pub unknown: i64,
}

/// An account's usage against its plan (SPEC-pricing §6.4): what
/// `GET /v1/usage` answers, and what the account page shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsageView {
    pub horizon: DataWindow,
    pub mics_day: MicWindow,
    pub mics_month: MicWindow,
    /// The plan id.
    pub plan: String,
    pub rate: Rate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub venue_days: Option<VenueDays>,
}

/// One venue's day, in a row-oriented list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VenueDay {
    pub date: jiff::civil::Date,
    pub kind: DayKindDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub products: Option<ProductSplitDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<SessionDto>,
    pub verification: VerificationDto,
    pub mic: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VenueDays {
    pub max: i64,
    /// The start of the next billing month.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<jiff::Timestamp>,
    pub used: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VenueResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    pub mic: String,
    pub name: String,
    pub operating_mic: String,
    /// The product schedules the venue's answers distinguish, in the venue's
    /// own names. Empty for almost every venue.
    pub products: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<ResolutionDto>,
    pub status: String,
    /// The IANA zone the venue's sessions are quoted in. Absent when no
    /// calendar answers the venue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

/// One venue's entry: `StatusResponse` without the `at` the response states
/// once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VenueStatus {
    pub day: DayDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_close: Option<jiff::Zoned>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_open: Option<jiff::Zoned>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<RunningSession>,
    pub state: StateDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VenuesResponse {
    /// Pass as `cursor` for the next page; null on the last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub venues: Vec<VenueResponse>,
}
