//! The client against recorded responses: every operation decodes, the
//! answers are the API's, errors are typed, and retries wait as they should.

mod common;

use std::time::Duration;

use common::{client, date, instant, server};
use diurn::models::{DayKindDto, StateDto};
use diurn::{Error, ExceptionsOptions, JointOptions, SessionsOptions, StepOptions, VenuesQuery};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn every_operation_decodes_its_recorded_answer() {
    let s = server().await;
    let c = client(&s);
    c.health().await.unwrap();
    let venue = c.venue("xnys").await.unwrap();
    assert_eq!(
        (venue.mic.as_str(), venue.timezone.as_deref()),
        ("XNYS", Some("America/New_York"))
    );
    let page = c
        .venues(VenuesQuery {
            country: Some("JP".into()),
            limit: Some(5),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page.venues.len(), 5);
    let year = c
        .sessions(
            "XNYS",
            date("2026-01-01"),
            date("2026-12-31"),
            SessionsOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(year.days.len(), 365);
    let day = c.day("XNYS", date("2026-11-26"), None).await.unwrap();
    assert_eq!(day.kind, DayKindDto::Holiday);
    c.sessions_bulk(
        ["XNYS", "XLON"],
        date("2026-11-20"),
        date("2026-11-30"),
        None,
    )
    .await
    .unwrap();
    c.trading_days(
        ["XNYS", "XLON"],
        date("2026-11-20"),
        date("2026-11-30"),
        None,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn instants_are_jiff_types_in_the_venues_zone() {
    let s = server().await;
    let c = client(&s);
    let status = c
        .status("XNYS", Some(instant("2026-11-27T15:00:00Z")), None)
        .await
        .unwrap();
    assert_eq!(status.state, StateDto::Running);
    assert_eq!(status.at, instant("2026-11-27T15:00:00Z"));
    let close = c
        .next_close("XNYS", instant("2026-11-27T15:00:00Z"), None)
        .await
        .unwrap()
        .at
        .unwrap();
    assert_eq!(close.time_zone().iana_name(), Some("America/New_York"));
    assert_eq!(
        close.to_string(),
        "2026-11-27T13:00:00-05:00[America/New_York]"
    );
    let session = &status.day.sessions[0];
    assert_eq!(session.close, close, "the early close, as a zoned instant");
}

#[tokio::test]
async fn trading_day_arithmetic_and_many_venues() {
    let s = server().await;
    let c = client(&s);
    let next = c
        .next_trading_day("XNYS", date("2026-11-25"), StepOptions::default())
        .await
        .unwrap();
    assert_eq!(next.date, date("2026-11-27"));
    assert_eq!(next.skipped[0].reason.as_deref(), Some("Thanksgiving Day"));
    let count = c
        .count_trading_days("XNYS", date("2026-11-01"), date("2026-11-30"), None)
        .await
        .unwrap();
    assert_eq!(count.count, 20);
    let joint = c
        .joint_next_trading_day(
            ["XNYS", "XLON"],
            date("2026-12-23"),
            JointOptions {
                n: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(joint.date, date("2026-12-29"));
    let statuses = c
        .statuses(
            ["XNYS", "XLON"],
            Some(instant("2026-11-26T15:00:00Z")),
            None,
        )
        .await
        .unwrap();
    assert_eq!(statuses.venues["XNYS"].state, StateDto::Closed);
    assert_eq!(statuses.venues["XLON"].state, StateDto::Running);
    let rows = c
        .exceptions(
            ["XNYS", "XLON"],
            date("2026-11-20"),
            date("2026-12-31"),
            ExceptionsOptions::default(),
        )
        .await
        .unwrap()
        .days;
    let keys: Vec<(String, String)> = rows
        .iter()
        .map(|r| (r.date.to_string(), r.mic.clone()))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "by date, then MIC");
}

#[tokio::test]
async fn a_long_mic_list_is_posted() {
    let s = server().await;
    Mock::given(method("POST"))
        .and(path("/v1/status"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"at": "2026-11-26T15:00:00Z", "venues": {}})),
        )
        .mount(&s)
        .await;
    let many: Vec<String> = (0..41).map(|i| format!("X{i:03}")).collect();
    client(&s).statuses(many.iter(), None, None).await.unwrap();
    let posted = s.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&posted.last().unwrap().body).unwrap();
    assert_eq!(body["mics"].as_array().unwrap().len(), 41);
}

#[tokio::test]
async fn a_mic_is_checked_before_any_request() {
    let s = server().await;
    assert!(matches!(
        client(&s).venue("NYSE!").await,
        Err(Error::InvalidMic(_))
    ));
    assert!(s.received_requests().await.unwrap().is_empty());
}

// --- errors -------------------------------------------------------------------

async fn refusal(status: u16, body: serde_json::Value) -> Error {
    let s = server().await;
    Mock::given(path("/v1/venues/XNYS"))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .with_priority(1)
        .mount(&s)
        .await;
    client(&s).venue("XNYS").await.unwrap_err()
}

#[tokio::test]
async fn errors_carry_their_numbers() {
    match refusal(
        429,
        json!({"error": "mic_cap_exceeded", "message": "", "mics_in_use": ["XLON", "XTKS"],
               "resets_at": "2026-11-28T09:00:00Z", "limit": {"kind": "mics_day", "used": 2, "max": 2}}),
    )
    .await
    {
        Error::MicCapExceeded { mics_in_use, resets_at, body } => {
            assert_eq!(mics_in_use, ["XLON", "XTKS"]);
            assert_eq!(resets_at, Some(instant("2026-11-28T09:00:00Z")));
            assert_eq!(body.raw["limit"]["max"], 2);
        }
        other => panic!("{other:?}"),
    }
    match refusal(403, json!({"error": "out_of_window", "message": "", "allowed": {"from": "2026-01-01", "to": null}})).await {
        Error::OutOfWindow { allowed_from, allowed_to, .. } => {
            assert_eq!((allowed_from, allowed_to), (Some(date("2026-01-01")), None));
        }
        other => panic!("{other:?}"),
    }
    match refusal(
        422,
        json!({"error": "indeterminate", "message": "", "date": "2100-01-01"}),
    )
    .await
    {
        Error::Indeterminate { date: d, body } => {
            assert_eq!(d, Some(date("2100-01-01")));
            assert_eq!(body.unwrap().status, 422);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        refusal(401, json!({"error": "needs_key", "message": ""})).await,
        Error::Auth(_)
    ));
    assert!(matches!(
        refusal(404, json!({"error": "unknown_mic", "message": ""})).await,
        Error::UnknownVenue(_)
    ));
    assert!(matches!(
        refusal(429, json!({"error": "venue_days_exceeded", "message": ""})).await,
        Error::OverLimit(_)
    ));
    assert!(matches!(
        refusal(503, json!({"error": "as_of_unavailable", "message": ""})).await,
        Error::Unavailable(_)
    ));
}

// --- retries --------------------------------------------------------------------

async fn retrying(
    first: ResponseTemplate,
    times: u64,
    retries: u32,
    max_wait: Duration,
) -> (diurn::Result<()>, usize) {
    let s = server().await;
    Mock::given(path("/v1/venues/XNYS"))
        .respond_with(first)
        .up_to_n_times(times)
        .with_priority(1)
        .mount(&s)
        .await;
    let c = diurn::Client::builder()
        .api_key("k")
        .base_url(s.uri())
        .retries(retries)
        .max_wait(max_wait)
        .backoff(Duration::from_millis(1))
        .build()
        .unwrap();
    let outcome = c.venue("XNYS").await.map(|_| ());
    let sent = s
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.url.path() == "/v1/venues/XNYS")
        .count();
    (outcome, sent)
}

fn rate_limited(after: &str) -> ResponseTemplate {
    ResponseTemplate::new(429)
        .insert_header("retry-after", after)
        .set_body_json(json!({"error": "rate_limited", "message": "slow down"}))
}

#[tokio::test]
async fn a_rate_limit_is_waited_out_then_answered() {
    let (outcome, sent) = retrying(rate_limited("0"), 1, 3, Duration::from_secs(60)).await;
    assert!(outcome.is_ok());
    assert_eq!(sent, 2);
}

#[tokio::test]
async fn failures_in_transit_back_off_and_run_out() {
    let down = ResponseTemplate::new(503)
        .set_body_json(json!({"error": "as_of_unavailable", "message": ""}));
    let (outcome, sent) = retrying(down.clone(), 2, 3, Duration::from_secs(60)).await;
    assert!(outcome.is_ok());
    assert_eq!(sent, 3);
    let (outcome, sent) = retrying(down, 10, 2, Duration::from_secs(60)).await;
    assert!(matches!(outcome, Err(Error::Unavailable(_))));
    assert_eq!(sent, 3, "the first try and two retries");
}

#[tokio::test]
async fn what_waiting_cannot_fix_is_not_retried() {
    let cap = ResponseTemplate::new(429)
        .set_body_json(json!({"error": "mic_cap_exceeded", "message": ""}));
    let (outcome, sent) = retrying(cap, 1, 3, Duration::from_secs(60)).await;
    assert!(matches!(outcome, Err(Error::MicCapExceeded { .. })));
    assert_eq!(sent, 1);
    let (outcome, sent) = retrying(rate_limited("120"), 1, 3, Duration::from_secs(60)).await;
    match outcome {
        Err(Error::RateLimited { retry_after, .. }) => {
            assert_eq!(retry_after, Some(Duration::from_secs(120)))
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        sent, 1,
        "a wait longer than max_wait is returned, not slept"
    );
    let (outcome, sent) = retrying(rate_limited("0"), 1, 0, Duration::from_secs(60)).await;
    assert!(matches!(outcome, Err(Error::RateLimited { .. })));
    assert_eq!(sent, 1, "no retry unless asked");
}

#[tokio::test]
async fn the_last_responses_headers_are_kept() {
    let s = server().await;
    Mock::given(path("/v1/venues/XNYS"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("diurn-mics-day", "1/5")
                .set_body_json(
                    serde_json::from_str::<serde_json::Value>(
                        &std::fs::read_to_string(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/tests/fixtures/v1_venues_XNYS.json"
                        ))
                        .unwrap(),
                    )
                    .unwrap(),
                ),
        )
        .with_priority(1)
        .mount(&s)
        .await;
    let c = client(&s);
    c.venue("XNYS").await.unwrap();
    assert!(
        c.last_headers()
            .iter()
            .any(|(k, v)| k == "diurn-mics-day" && v == "1/5")
    );
}
