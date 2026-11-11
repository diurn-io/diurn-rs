//! Typed errors: a caller matches on the variant, never on a message.
//!
//! Every error the API returns carries a stable tag (`error`), and each tag has
//! a variant here, as in diurn-py's `errors.py`. Refusals from a plan's limits
//! carry their numbers, so a program can act on them (wait until `resets_at`,
//! narrow to `allowed_from`..`allowed_to`) rather than retry blindly.

use jiff::civil::Date;

/// The body of an API error response, as sent: its tag, its message, and the
/// whole JSON for anything not lifted into a variant's fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiErrorBody {
    /// The HTTP status.
    pub status: u16,
    /// The API's stable tag, such as `out_of_window`.
    pub error: String,
    /// The API's message, for a person.
    pub message: String,
    /// The response as sent.
    pub raw: serde_json::Value,
}

impl std::fmt::Display for ApiErrorBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.message.is_empty() {
            write!(f, "{} ({})", self.error, self.status)
        } else {
            write!(f, "{}: {} ({})", self.error, self.message, self.status)
        }
    }
}

/// Everything this crate returns as an error. The API's response is boxed,
/// so a `Result` stays small.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// No key (`needs_key`), or a key that is not active (`unauthorized`).
    /// Keys are free at <https://diurn.io/signin/>; set `DIURN_API_KEY`.
    #[error("{0}")]
    Auth(Box<ApiErrorBody>),
    /// No such MIC, or no calendar answers it.
    #[error("{0}")]
    UnknownVenue(Box<ApiErrorBody>),
    /// The request could not be understood: a malformed date, too many days,
    /// or `as_of` outside the archive's record.
    #[error("{0}")]
    BadRequest(Box<ApiErrorBody>),
    /// The plan does not include this: an endpoint, provenance or `as_of`.
    #[error("{0}")]
    NotInPlan(Box<ApiErrorBody>),
    /// Dates outside the plan's window, which is refused rather than trimmed.
    #[error("{body}")]
    OutOfWindow {
        /// The first date the plan may ask about; `None` is unbounded.
        allowed_from: Option<Date>,
        /// The last; `None` is unbounded.
        allowed_to: Option<Date>,
        /// The response.
        body: Box<ApiErrorBody>,
    },
    /// Asking for this MIC would exceed a unique-MIC cap. Asking again for one
    /// of `mics_in_use` is free.
    #[error("{body}")]
    MicCapExceeded {
        /// The MICs holding the window's slots.
        mics_in_use: Vec<String>,
        /// When a slot frees.
        resets_at: Option<jiff::Timestamp>,
        /// The response.
        body: Box<ApiErrorBody>,
    },
    /// Too many requests a minute.
    #[error("{body}")]
    RateLimited {
        /// The server's `Retry-After`, where it sent one.
        retry_after: Option<std::time::Duration>,
        /// The response.
        body: Box<ApiErrorBody>,
    },
    /// Any other refusal from a plan's limits (a 429 this version does not
    /// name), with its `limit` and `resets_at` in `raw`.
    #[error("{0}")]
    OverLimit(Box<ApiErrorBody>),
    /// Diurn cannot say whether the market trades on a date: the API's
    /// `422 indeterminate`, or a calendar asked about an `unknown` day.
    /// Diurn never reports such a day as open or closed.
    #[error("cannot be answered for {}", date.map_or_else(|| "a date".to_string(), |d| d.to_string()))]
    Indeterminate {
        /// The day that cannot be answered.
        date: Option<Date>,
        /// The response, when the API said so.
        body: Option<Box<ApiErrorBody>>,
    },
    /// The service could not answer just now (503): for example an `as_of`
    /// dataset that could not be loaded.
    #[error("{0}")]
    Unavailable(Box<ApiErrorBody>),
    /// Any other error the API returned.
    #[error("{0}")]
    Api(Box<ApiErrorBody>),
    /// The API could not be reached, or answered with something that is not
    /// the API (a proxy's page, a broken connection).
    #[error("transport: {0}")]
    Transport(String),
    /// A response that does not parse as the shape it should have.
    #[error("decoding the response: {0}")]
    Decode(String),
    /// Not a MIC: four characters, letters and digits.
    #[error("not a MIC: {0:?}")]
    InvalidMic(String),
    /// A calendar was asked about a date outside the range it loaded. It
    /// never fetches more on its own: load a wider range.
    #[error("{date} is outside the loaded range {from} to {to}")]
    OutsideLoaded {
        /// The date asked about, or reached by a walk.
        date: Date,
        /// The first loaded date.
        from: Date,
        /// The last loaded date.
        to: Date,
    },
    /// A date that is not a session, where one was required.
    #[error("{0} is not a session")]
    NotASession(Date),
    /// An argument the API would refuse, caught before any request.
    #[error("{0}")]
    InvalidArgument(String),
}

/// `Result` with this crate's error.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// The API's response, for an error that came from one.
    pub fn body(&self) -> Option<&ApiErrorBody> {
        match self {
            Error::Auth(b)
            | Error::UnknownVenue(b)
            | Error::BadRequest(b)
            | Error::NotInPlan(b)
            | Error::OverLimit(b)
            | Error::Unavailable(b)
            | Error::Api(b) => Some(b),
            Error::OutOfWindow { body, .. }
            | Error::MicCapExceeded { body, .. }
            | Error::RateLimited { body, .. } => Some(body),
            Error::Indeterminate { body, .. } => body.as_deref(),
            _ => None,
        }
    }

    /// The error for an API error response: by its tag, then by its status.
    pub(crate) fn from_response(
        status: u16,
        raw: serde_json::Value,
        retry_after: Option<std::time::Duration>,
    ) -> Self {
        let text = |key: &str| {
            raw.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let error = match text("error") {
            e if e.is_empty() => format!("http_{status}"),
            e => e,
        };
        let date =
            |v: Option<&serde_json::Value>| v.and_then(|v| v.as_str()).and_then(|s| s.parse().ok());
        let body = Box::new(ApiErrorBody {
            status,
            error: error.clone(),
            message: text("message"),
            raw: raw.clone(),
        });
        match error.as_str() {
            "needs_key" | "unauthorized" => Error::Auth(body),
            "not_in_plan" => Error::NotInPlan(body),
            "out_of_window" => {
                let allowed = raw.get("allowed");
                Error::OutOfWindow {
                    allowed_from: date(allowed.and_then(|a| a.get("from"))),
                    allowed_to: date(allowed.and_then(|a| a.get("to"))),
                    body,
                }
            }
            "mic_cap_exceeded" => Error::MicCapExceeded {
                mics_in_use: raw
                    .get("mics_in_use")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|m| m.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                resets_at: raw
                    .get("resets_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok()),
                body,
            },
            "rate_limited" => Error::RateLimited { retry_after, body },
            "indeterminate" => Error::Indeterminate {
                date: date(raw.get("date")),
                body: Some(body),
            },
            _ => match status {
                400 => Error::BadRequest(body),
                401 => Error::Auth(body),
                404 => Error::UnknownVenue(body),
                422 => Error::Indeterminate {
                    date: date(raw.get("date")),
                    body: Some(body),
                },
                429 => Error::OverLimit(body),
                503 => Error::Unavailable(body),
                _ => Error::Api(body),
            },
        }
    }
}
