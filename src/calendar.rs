//! A venue's trading calendar over a loaded range of dates.
//!
//! Loaded once by [`Client::calendar`](crate::Client::calendar), then answered
//! from memory: every method here is synchronous and makes no request. The
//! names are `exchange_calendars`', as diurn-py's are.
//!
//! A *session* is a day the venue trades, as the API's `/v1/trading-days`
//! counts one: `open`, `early_close`, `open_for_some`, or a `holiday` that
//! carries a session (NSE's Muhurat).
//!
//! **Nothing is guessed.** A day Diurn cannot answer is
//! [`Error::Indeterminate`], never open and never closed; a question that
//! reaches past the loaded range is [`Error::OutsideLoaded`] (load a wider
//! one); a date that should be a session and is not is
//! [`Error::NotASession`].

use std::ops::RangeInclusive;

use diurn_mic::Mic;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};

use crate::error::{ApiErrorBody, Error, Result};
use crate::generated::models as m;

/// What kind of day it is.
pub type DayKind = m::DayKindDto;
/// How a day, or a session, is evidenced.
pub type Verification = m::VerificationDto;

/// The longest range one request may ask for.
const MAX_DAYS: i32 = 1830;

/// How a calendar is loaded, beyond its venue and range.
#[derive(Debug, Clone, Default)]
pub struct CalendarOptions {
    /// One of the venue's product schedules (see [`Calendar::products`]); by
    /// default the whole venue's sessions are used.
    pub product: Option<String>,
    /// Answer from the dataset Diurn was serving at this instant (Enterprise).
    pub as_of: Option<Timestamp>,
}

/// Whether `date_to_session` may move, and which way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    /// The date must be a session.
    #[default]
    None,
    /// The date, or the next session after it.
    Next,
    /// The date, or the last session before it.
    Previous,
}

/// One session: open and close in the venue's zone, its breaks, and the
/// product it is for (`None`: the whole venue).
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// The calendar's name for it (`regular`, `morning`, ...).
    pub name: String,
    /// When it opens.
    pub open: Zoned,
    /// When it closes.
    pub close: Zoned,
    /// Pauses inside it (Hong Kong's lunch), start and end.
    pub breaks: Vec<(Zoned, Zoned)>,
    /// The product it is for, in the venue's own words; `None` for the venue.
    pub products: Option<String>,
    /// How its hours are evidenced.
    pub verification: Verification,
}

/// One day of the calendar.
#[derive(Debug, Clone, PartialEq)]
pub struct Day {
    /// The date.
    pub date: Date,
    /// What kind of day it is.
    pub kind: DayKind,
    /// How it is evidenced.
    pub verification: Verification,
    /// Why it is not an ordinary day (a holiday's name).
    pub reason: Option<String>,
    /// Its sessions; none on a closed day.
    pub sessions: Vec<Session>,
}

impl Day {
    /// A day Diurn cannot answer.
    pub fn is_unknown(&self) -> bool {
        self.kind == DayKind::Unknown
    }

    /// A day the venue trades: `open`, `early_close`, `open_for_some`, or a
    /// holiday carrying a session.
    pub fn is_session(&self) -> bool {
        matches!(
            self.kind,
            DayKind::Open | DayKind::EarlyClose | DayKind::OpenForSome
        ) || (self.kind == DayKind::Holiday && !self.sessions.is_empty())
    }
}

/// A day's schedule as `exchange_calendars` lays it out: the first open, the
/// last close, and the first break between them; plus Diurn's `kind`,
/// `verification` and `reason`. The times are `None` where Diurn holds no
/// hours for the day.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleRow {
    /// The date.
    pub date: Date,
    /// The first open.
    pub open: Option<Zoned>,
    /// The start of the first break.
    pub break_start: Option<Zoned>,
    /// The end of the first break.
    pub break_end: Option<Zoned>,
    /// The last close.
    pub close: Option<Zoned>,
    /// What kind of day it is.
    pub kind: DayKind,
    /// How it is evidenced.
    pub verification: Verification,
    /// Why it is not an ordinary day.
    pub reason: Option<String>,
}

/// One venue's trading calendar over the range of dates it was loaded with.
#[derive(Debug, Clone)]
pub struct Calendar {
    mic: Mic,
    market_name: String,
    tz: TimeZone,
    tz_name: String,
    products: Vec<String>,
    product: Option<String>,
    from: Date,
    to: Date,
    days: Vec<Day>,
}

/// The requests a range needs: one per 1,830 days, the API's limit.
pub(crate) fn chunks(range: &RangeInclusive<Date>) -> Result<Vec<(Date, Date)>> {
    let (start, end) = (*range.start(), *range.end());
    if end < start {
        return Err(Error::InvalidArgument(format!("{end} is before {start}")));
    }
    let mut out = Vec::new();
    let mut from = start;
    loop {
        let to = from
            .checked_add(jiff::Span::new().days(MAX_DAYS - 1))
            .unwrap_or(end)
            .min(end);
        out.push((from, to));
        if to >= end {
            return Ok(out);
        }
        from = to
            .tomorrow()
            .map_err(|e| Error::InvalidArgument(e.to_string()))?;
    }
}

fn session(s: m::SessionDto) -> Session {
    Session {
        name: s.name,
        open: s.open,
        close: s.close,
        breaks: s.breaks.into_iter().map(|b| (b.start, b.end)).collect(),
        products: s.products,
        verification: s.verification,
    }
}

impl Calendar {
    pub(crate) fn assemble(
        record: m::VenueResponse,
        days: Vec<m::DayDto>,
        range: RangeInclusive<Date>,
        product: Option<String>,
    ) -> Result<Self> {
        let no_calendar = || {
            Error::UnknownVenue(Box::new(ApiErrorBody {
                status: 404,
                error: "no_calendar".into(),
                message: format!("no calendar answers {}", record.mic),
                raw: serde_json::Value::Null,
            }))
        };
        let tz_name = record.timezone.clone().ok_or_else(no_calendar)?;
        if record.resolution.is_none() {
            return Err(no_calendar());
        }
        if let Some(p) = &product {
            if !record.products.contains(p) {
                return Err(Error::InvalidArgument(format!(
                    "{} has no product {p:?}; it has {:?}",
                    record.mic, record.products
                )));
            }
        }
        let tz = TimeZone::get(&tz_name)
            .map_err(|e| Error::Decode(format!("time zone {tz_name}: {e}")))?;
        let mic = Mic::new(&record.mic).map_err(|_| Error::InvalidMic(record.mic.clone()))?;
        let (from, to) = (*range.start(), *range.end());
        let days: Vec<Day> = days
            .into_iter()
            .map(|d| Day {
                date: d.date,
                kind: d.kind,
                verification: d.verification,
                reason: d.reason,
                sessions: d.sessions.into_iter().map(session).collect(),
            })
            .collect();
        let expected = from
            .until(to)
            .map(|s| s.get_days() as usize + 1)
            .unwrap_or(0);
        if days.len() != expected || days.first().map(|d| d.date) != Some(from) {
            return Err(Error::Decode(format!(
                "expected {expected} days from {from}, received {}",
                days.len()
            )));
        }
        Ok(Self {
            mic,
            market_name: record.name,
            tz,
            tz_name,
            products: record.products,
            product,
            from,
            to,
            days,
        })
    }

    // --- what it is ------------------------------------------------------------

    /// The venue's MIC.
    pub fn mic(&self) -> Mic {
        self.mic
    }

    /// The MIC, as `exchange_calendars` names a calendar by its code.
    pub fn name(&self) -> &str {
        self.mic.as_str()
    }

    /// The venue's name in the ISO 10383 registry.
    pub fn market_name(&self) -> &str {
        &self.market_name
    }

    /// The zone the venue's sessions are quoted in.
    pub fn tz(&self) -> &TimeZone {
        &self.tz
    }

    /// The zone's IANA name.
    pub fn tz_name(&self) -> &str {
        &self.tz_name
    }

    /// The product schedules the venue keeps; empty for almost every one.
    pub fn products(&self) -> &[String] {
        &self.products
    }

    /// The product schedule this calendar answers for, if one was chosen.
    pub fn product(&self) -> Option<&str> {
        self.product.as_deref()
    }

    /// The dates loaded.
    pub fn range(&self) -> RangeInclusive<Date> {
        self.from..=self.to
    }

    // --- days ------------------------------------------------------------------

    /// The day, whatever its kind.
    pub fn day(&self, date: Date) -> Result<&Day> {
        let outside = || Error::OutsideLoaded {
            date,
            from: self.from,
            to: self.to,
        };
        if date < self.from || date > self.to {
            return Err(outside());
        }
        let index = self.from.until(date).map_err(|_| outside())?.get_days() as usize;
        self.days.get(index).ok_or_else(outside)
    }

    /// Every day loaded.
    pub fn days(&self) -> &[Day] {
        &self.days
    }

    fn known(&self, date: Date) -> Result<&Day> {
        let day = self.day(date)?;
        if day.is_unknown() {
            return Err(Error::Indeterminate {
                date: Some(date),
                body: None,
            });
        }
        Ok(day)
    }

    fn days_in(&self, range: &RangeInclusive<Date>) -> Result<Vec<&Day>> {
        let (a, b) = (*range.start(), *range.end());
        if b < a {
            return Ok(Vec::new());
        }
        self.day(a)?;
        self.day(b)?;
        let start = self
            .from
            .until(a)
            .map(|s| s.get_days() as usize)
            .unwrap_or(0);
        let end = self
            .from
            .until(b)
            .map(|s| s.get_days() as usize)
            .unwrap_or(0);
        Ok(self.days[start..=end].iter().collect())
    }

    // --- the exchange_calendars interface -----------------------------------------

    /// Whether the venue trades on the date.
    pub fn is_session(&self, date: Date) -> Result<bool> {
        Ok(self.known(date)?.is_session())
    }

    /// The sessions in a range, inclusive.
    pub fn sessions_in_range(&self, range: RangeInclusive<Date>) -> Result<Vec<Date>> {
        let mut out = Vec::new();
        for day in self.days_in(&range)? {
            if day.is_unknown() {
                return Err(Error::Indeterminate {
                    date: Some(day.date),
                    body: None,
                });
            }
            if day.is_session() {
                out.push(day.date);
            }
        }
        Ok(out)
    }

    /// The first session after `date`, which need not be a session itself.
    pub fn next_session(&self, date: Date) -> Result<Date> {
        self.step(date, true)
    }

    /// The last session before `date`, which need not be a session itself.
    pub fn previous_session(&self, date: Date) -> Result<Date> {
        self.step(date, false)
    }

    fn step(&self, start: Date, forward: bool) -> Result<Date> {
        let mut d = start;
        loop {
            d = if forward { d.tomorrow() } else { d.yesterday() }
                .map_err(|e| Error::InvalidArgument(e.to_string()))?;
            if self.known(d)?.is_session() {
                return Ok(d);
            }
        }
    }

    /// `date` if it is a session; otherwise the next or previous one, or with
    /// [`Direction::None`], [`Error::NotASession`].
    pub fn date_to_session(&self, date: Date, direction: Direction) -> Result<Date> {
        if self.is_session(date)? {
            return Ok(date);
        }
        match direction {
            Direction::Next => self.next_session(date),
            Direction::Previous => self.previous_session(date),
            Direction::None => Err(Error::NotASession(date)),
        }
    }

    /// `count.abs()` sessions from `session`: forwards for a positive count,
    /// backwards and ending at `session` for a negative one.
    pub fn sessions_window(&self, session: Date, count: i32) -> Result<Vec<Date>> {
        if !self.is_session(session)? {
            return Err(Error::NotASession(session));
        }
        if count == 0 {
            return Err(Error::InvalidArgument("count is not zero".into()));
        }
        let mut out = vec![session];
        while out.len() < count.unsigned_abs() as usize {
            let last = *out.last().unwrap_or(&session);
            out.push(self.step(last, count > 0)?);
        }
        if count < 0 {
            out.reverse();
        }
        Ok(out)
    }

    /// The number of sessions from `start` to `end`, both included; negative
    /// when `end` is before `start`. A range with an unknown day is
    /// [`Error::Indeterminate`]: a count that skipped it would be wrong.
    pub fn sessions_distance(&self, start: Date, end: Date) -> Result<i64> {
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let count = self.sessions_in_range(lo..=hi)?.len() as i64;
        Ok(if start <= end { count } else { -count })
    }

    /// Whether a session is running at the instant, outside its breaks.
    pub fn is_open_at_time(&self, at: Timestamp) -> Result<bool> {
        let local = at.to_zoned(self.tz.clone()).date();
        self.known(local)?;
        for date in [local.yesterday().ok(), Some(local), local.tomorrow().ok()]
            .into_iter()
            .flatten()
        {
            let Ok(day) = self.day(date) else { continue };
            for s in self.sessions(day) {
                let inside = s.open.timestamp() <= at && at < s.close.timestamp();
                let paused = s
                    .breaks
                    .iter()
                    .any(|(b0, b1)| b0.timestamp() <= at && at < b1.timestamp());
                if inside && !paused {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// The first session open after the instant.
    pub fn next_open(&self, after: Timestamp) -> Result<Zoned> {
        self.next_edge(after, true)
    }

    /// The first session close after the instant.
    pub fn next_close(&self, after: Timestamp) -> Result<Zoned> {
        self.next_edge(after, false)
    }

    fn next_edge(&self, after: Timestamp, open: bool) -> Result<Zoned> {
        let local = after.to_zoned(self.tz.clone()).date();
        // A session can open the evening before its date (a night session).
        let mut d = local.yesterday().unwrap_or(local).max(self.from);
        loop {
            let day = self.day(d)?;
            if day.is_unknown() && d >= local {
                return Err(Error::Indeterminate {
                    date: Some(d),
                    body: None,
                });
            }
            let mut edges: Vec<&Zoned> = self
                .sessions(day)
                .iter()
                .map(|s| if open { &s.open } else { &s.close })
                .collect();
            edges.sort();
            if let Some(edge) = edges.into_iter().find(|e| e.timestamp() > after) {
                return Ok(edge.clone());
            }
            d = d
                .tomorrow()
                .map_err(|e| Error::InvalidArgument(e.to_string()))?;
        }
    }

    // --- one session's times -----------------------------------------------------

    /// The session's first open; `None` where Diurn holds no hours for it.
    pub fn session_open(&self, session: Date) -> Result<Option<Zoned>> {
        Ok(self.session_row(session)?.open)
    }

    /// The session's last close.
    pub fn session_close(&self, session: Date) -> Result<Option<Zoned>> {
        Ok(self.session_row(session)?.close)
    }

    /// The start of the session's break (Tokyo's lunch); `None` without one.
    pub fn session_break_start(&self, session: Date) -> Result<Option<Zoned>> {
        Ok(self.session_row(session)?.break_start)
    }

    /// The end of the session's break.
    pub fn session_break_end(&self, session: Date) -> Result<Option<Zoned>> {
        Ok(self.session_row(session)?.break_end)
    }

    fn session_row(&self, date: Date) -> Result<ScheduleRow> {
        let day = self.known(date)?;
        if !day.is_session() {
            return Err(Error::NotASession(date));
        }
        Ok(self.row(day))
    }

    /// One row per session in a range; with `all_days`, one per day, so
    /// holidays and unknown days appear with their kind and no times.
    pub fn schedule(
        &self,
        range: RangeInclusive<Date>,
        all_days: bool,
    ) -> Result<Vec<ScheduleRow>> {
        let mut out = Vec::new();
        for day in self.days_in(&range)? {
            if !all_days {
                if day.is_unknown() {
                    return Err(Error::Indeterminate {
                        date: Some(day.date),
                        body: None,
                    });
                }
                if !day.is_session() {
                    continue;
                }
            }
            out.push(self.row(day));
        }
        Ok(out)
    }

    // --- internals -----------------------------------------------------------------

    /// The day's sessions for this calendar: the chosen product's (and the
    /// venue's), or the venue's, or, on a venue whose every session is scoped
    /// to a product, all of them.
    fn sessions<'a>(&self, day: &'a Day) -> Vec<&'a Session> {
        match &self.product {
            Some(p) => day
                .sessions
                .iter()
                .filter(|s| s.products.as_deref().is_none_or(|sp| sp == p))
                .collect(),
            None => {
                let whole: Vec<&Session> = day
                    .sessions
                    .iter()
                    .filter(|s| s.products.is_none())
                    .collect();
                if whole.is_empty() {
                    day.sessions.iter().collect()
                } else {
                    whole
                }
            }
        }
    }

    fn row(&self, day: &Day) -> ScheduleRow {
        let mut sessions = self.sessions(day);
        sessions.sort_by(|a, b| a.open.cmp(&b.open));
        let (open, close, gap) = match (sessions.first(), sessions.last()) {
            (Some(first), Some(last)) => {
                let gap = if sessions.len() > 1 {
                    Some((first.close.clone(), sessions[1].open.clone()))
                } else {
                    first.breaks.first().cloned()
                };
                (Some(first.open.clone()), Some(last.close.clone()), gap)
            }
            _ => (None, None, None),
        };
        let (break_start, break_end) = gap.map_or((None, None), |(a, b)| (Some(a), Some(b)));
        ScheduleRow {
            date: day.date,
            open,
            break_start,
            break_end,
            close,
            kind: day.kind,
            verification: day.verification,
            reason: day.reason.clone(),
        }
    }
}
