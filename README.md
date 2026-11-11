# diurn

Exchange calendars and trading hours for the world's markets, from the
[Diurn API](https://diurn.io): every session, holiday and early close, keyed
by MIC, with how each day is evidenced.

```toml
[dependencies]
diurn = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

An API key is free at <https://diurn.io/signin/>; set `DIURN_API_KEY`.

## A calendar

Load a venue's days for a range once; every question after that is answered
from memory, without another request.

```rust,no_run
use diurn::{Client, Direction};

#[tokio::main]
async fn main() -> diurn::Result<()> {
    let client = Client::new()?; // DIURN_API_KEY, DIURN_API_URL
    let nov = |d: i8| jiff::civil::date(2026, 11, d);
    let xnys = client.calendar("XNYS", nov(1)..=nov(30)).await?;

    assert!(!xnys.is_session(nov(26))?); // Thanksgiving
    assert_eq!(xnys.next_session(nov(25))?, nov(27)); // an early close is a session
    assert_eq!(xnys.sessions_distance(nov(1), nov(30))?, 20);
    assert_eq!(xnys.date_to_session(nov(26), Direction::Next)?, nov(27));

    let close = xnys.session_close(nov(27))?.expect("hours are held");
    println!("{close}"); // 2026-11-27T13:00:00-05:00[America/New_York]
    for row in xnys.schedule(nov(23)..=nov(27), true)? {
        println!("{} {:?} {:?}", row.date, row.kind, row.reason);
    }
    Ok(())
}
```

The methods are `exchange_calendars`' names, as in the Python package:
`is_session`, `sessions_in_range`, `next_session`, `previous_session`,
`date_to_session`, `sessions_window`, `sessions_distance`, `is_open_at_time`,
`next_open`, `next_close`, `session_open`, `session_close`,
`session_break_start`, `session_break_end` and `schedule`. Dates are
`jiff::civil::Date`; instants are `jiff::Timestamp` in, and `jiff::Zoned` in
the venue's own zone out.

- **A session is a day the venue trades**, as the API's `/v1/trading-days`
  counts one: `open`, `early_close`, `open_for_some`, and a `holiday` that
  carries a session, such as NSE's Muhurat.
- **Nothing is guessed.** A day Diurn cannot answer is
  `Error::Indeterminate`, never open and never closed. A question that walks
  past the loaded range is `Error::OutsideLoaded`: a calendar never fetches
  more on its own, so load a wider range.
- **A session's times are `None`** where Diurn holds no hours for it, and a
  date that is not a session is `Error::NotASession`.
- **Product schedules.** Where a venue keeps several (`calendar.products()`),
  `client.calendar_with(mic, range, CalendarOptions { product: Some(..), .. })`
  picks one. A product with no hours of its own has none: it is never given
  another product's.
- **`name()` is the MIC**, as `exchange_calendars` names a calendar by its
  code; the registry's name is `market_name()`.

## The API

`Client` has one method per API operation, returning the API's own shapes
(`diurn::models`): venues and venue search, status for one venue or many,
sessions, single days with their evidence, holidays and irregular days across
venues, trading-day arithmetic for one venue or jointly across several, and
your plan's usage.

```rust,no_run
use diurn::{Client, JointOptions, StepOptions};
use jiff::civil::date;

#[tokio::main]
async fn main() -> diurn::Result<()> {
    let client = Client::new()?;
    let now = client.statuses(["XNYS", "XLON", "XTKS"], None, None).await?;
    for (mic, venue) in &now.venues {
        println!("{mic}: {:?}", venue.state);
    }
    let next = client.next_trading_day("XNYS", date(2026, 11, 25), StepOptions::default()).await?;
    println!("{} ({:?})", next.date, next.skipped); // 2026-11-27, past Thanksgiving
    let both = client
        .joint_next_trading_day(["XNYS", "XLON"], date(2026, 12, 23), JointOptions { n: Some(2), ..Default::default() })
        .await?;
    println!("{}", both.date); // 2026-12-29: Christmas, then London's Boxing Day
    Ok(())
}
```

MICs are `diurn::Mic` (from `diurn-mic`) or strings that spell one. Every
metered method takes an `as_of` instant (Enterprise): the answer Diurn was
serving then.

**Retries** are off by default. `Client::builder().retries(3).build()?`
retries what waiting can fix: a rate limit, after the server's `Retry-After`;
a 503 or a dropped connection, backing off from half a second. A plan's
refusal (a MIC cap, a date outside the window) is never retried, and a wait
longer than `max_wait` (60 seconds) is returned as the error rather than
slept.

## Without async

The `blocking` feature adds `diurn::blocking::Client`, with the same methods,
generated from the same list:

```toml
diurn = { version = "0.1", features = ["blocking"] }
```

## Errors

Typed, never prose: `Error::Auth` (no or inactive key), `UnknownVenue`,
`BadRequest`, `NotInPlan`, `OutOfWindow` (with `allowed_from` and
`allowed_to`), `MicCapExceeded` (with `mics_in_use` and `resets_at`),
`RateLimited` (with `retry_after`), `OverLimit` (any other plan refusal),
`Indeterminate`, `Unavailable`, `Transport` and `Decode`. An error from the
API keeps its response: `error.body()`.

## Features

- `rustls` (default): TLS with rustls. `native-tls` uses the platform's TLS
  instead.
- `blocking`: `diurn::blocking::Client`.

Rust 1.85 or later.

## Development

The wire layer (`src/generated/models.rs`) is generated from the API's OpenAPI
document and committed. To refresh it, from a checkout of `diurn-platform`:

```sh
cargo run -p diurn-api -- --openapi > ../diurn-rs/openapi.json
cargo run -p xtask -- generate          # CI runs it with --check
```

The tests replay responses recorded from a local API server
(`cargo run -p xtask -- record http://127.0.0.1:8099`), so they need no
network or key.

## License

MIT or Apache-2.0, at your option.
