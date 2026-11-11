//! The calendar's answers, from recorded XNYS, XTKS, XHKG, XCEC and XNSE
//! responses: the same questions diurn-py's tests ask, with the same answers.

mod common;

use common::{client, date, instant, server};
use diurn::models::DayKindDto;
use diurn::{Calendar, CalendarOptions, Direction, Error};
use wiremock::matchers::path;
use wiremock::{Mock, ResponseTemplate};

async fn cal(mic: &str) -> Calendar {
    let s = server().await;
    client(&s)
        .calendar(mic, date("2026-01-01")..=date("2026-12-31"))
        .await
        .unwrap()
}

#[tokio::test]
async fn sessions_around_thanksgiving() {
    let x = cal("XNYS").await;
    assert!(!x.is_session(date("2026-11-26")).unwrap(), "Thanksgiving");
    assert!(
        x.is_session(date("2026-11-27")).unwrap(),
        "an early close is a session"
    );
    assert_eq!(
        x.next_session(date("2026-11-25")).unwrap(),
        date("2026-11-27")
    );
    assert_eq!(
        x.previous_session(date("2026-11-27")).unwrap(),
        date("2026-11-25")
    );
    assert_eq!(
        x.next_session(date("2026-11-26")).unwrap(),
        date("2026-11-27"),
        "need not be a session"
    );
    assert_eq!(
        x.sessions_in_range(date("2026-11-24")..=date("2026-11-30"))
            .unwrap(),
        [
            date("2026-11-24"),
            date("2026-11-25"),
            date("2026-11-27"),
            date("2026-11-30")
        ]
    );
}

#[tokio::test]
async fn date_to_session_and_window() {
    let x = cal("XNYS").await;
    assert_eq!(
        x.date_to_session(date("2026-11-27"), Direction::None)
            .unwrap(),
        date("2026-11-27")
    );
    assert_eq!(
        x.date_to_session(date("2026-11-26"), Direction::Next)
            .unwrap(),
        date("2026-11-27")
    );
    assert_eq!(
        x.date_to_session(date("2026-11-26"), Direction::Previous)
            .unwrap(),
        date("2026-11-25")
    );
    assert!(matches!(
        x.date_to_session(date("2026-11-26"), Direction::None),
        Err(Error::NotASession(_))
    ));
    assert_eq!(
        x.sessions_window(date("2026-11-25"), 3).unwrap(),
        [date("2026-11-25"), date("2026-11-27"), date("2026-11-30")]
    );
    assert_eq!(
        x.sessions_window(date("2026-11-27"), -2).unwrap(),
        [date("2026-11-25"), date("2026-11-27")]
    );
}

#[tokio::test]
async fn a_walk_crosses_the_year_when_both_are_loaded() {
    let s = server().await;
    let c = client(&s);
    let two = c
        .calendar("XNYS", date("2026-01-01")..=date("2027-12-31"))
        .await
        .unwrap();
    assert_eq!(
        two.next_session(date("2026-12-31")).unwrap(),
        date("2027-01-04"),
        "New Year's Day, then a weekend"
    );
    let one = c
        .calendar("XNYS", date("2026-01-01")..=date("2026-12-31"))
        .await
        .unwrap();
    match one.next_session(date("2026-12-31")) {
        Err(Error::OutsideLoaded { date: d, to, .. }) => {
            assert_eq!((d, to), (date("2027-01-01"), date("2026-12-31")));
        }
        other => panic!("no silent fetch: {other:?}"),
    }
}

#[tokio::test]
async fn instants_keep_the_venues_zone() {
    let x = cal("XNYS").await;
    let close = x.next_close(instant("2026-11-27T15:00:00Z")).unwrap();
    assert_eq!(
        close.to_string(),
        "2026-11-27T13:00:00-05:00[America/New_York]"
    );
    let opens = x.next_open(instant("2026-11-26T12:00:00Z")).unwrap();
    assert_eq!(
        opens.date(),
        date("2026-11-27"),
        "Thanksgiving: the next open is Friday's"
    );
    assert!(x.is_open_at_time(instant("2026-11-27T17:30:00Z")).unwrap());
    assert!(
        !x.is_open_at_time(instant("2026-11-27T18:30:00Z")).unwrap(),
        "after the 13:00 close"
    );
}

#[tokio::test]
async fn a_schedule_lays_out_breaks_like_exchange_calendars() {
    let tokyo = cal("XTKS").await;
    let rows = tokyo
        .schedule(date("2026-11-02")..=date("2026-11-02"), false)
        .unwrap();
    let r = &rows[0];
    let hour = |z: &Option<jiff::Zoned>| z.as_ref().unwrap().hour();
    assert_eq!(
        (
            hour(&r.open),
            hour(&r.break_start),
            hour(&r.break_end),
            hour(&r.close)
        ),
        (9, 11, 12, 15)
    );
    assert!(
        tokyo
            .schedule(date("2026-11-03")..=date("2026-11-03"), false)
            .unwrap()
            .is_empty(),
        "Culture Day"
    );
    let all = tokyo
        .schedule(date("2026-11-03")..=date("2026-11-03"), true)
        .unwrap();
    assert_eq!(all[0].kind, DayKindDto::Holiday);
    assert!(all[0].open.is_none() && all[0].reason.is_some());
}

#[tokio::test]
async fn a_close_inside_the_lunch_break_has_no_break() {
    let hk = cal("XHKG").await;
    let eve = &hk
        .schedule(date("2026-12-24")..=date("2026-12-24"), false)
        .unwrap()[0];
    assert_eq!(eve.kind, DayKindDto::EarlyClose);
    assert!(eve.break_start.is_none());
    assert_eq!(
        eve.close.as_ref().unwrap().strftime("%H:%M").to_string(),
        "12:10"
    );
}

#[tokio::test]
async fn one_sessions_times_and_the_distance_between_two() {
    let tokyo = cal("XTKS").await;
    let at = |z: Option<jiff::Zoned>| z.unwrap().strftime("%H:%M").to_string();
    assert_eq!(at(tokyo.session_open(date("2026-11-02")).unwrap()), "09:00");
    assert_eq!(
        at(tokyo.session_break_start(date("2026-11-02")).unwrap()),
        "11:30"
    );
    assert_eq!(
        at(tokyo.session_break_end(date("2026-11-02")).unwrap()),
        "12:30"
    );
    assert_eq!(
        at(tokyo.session_close(date("2026-11-02")).unwrap()),
        "15:30"
    );
    let nyse = cal("XNYS").await;
    assert!(
        nyse.session_break_start(date("2026-11-27"))
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        nyse.session_open(date("2026-11-26")),
        Err(Error::NotASession(_))
    ));
    assert_eq!(
        nyse.sessions_distance(date("2026-11-01"), date("2026-11-30"))
            .unwrap(),
        20
    );
    assert_eq!(
        nyse.sessions_distance(date("2026-11-30"), date("2026-11-01"))
            .unwrap(),
        -20
    );
    assert_eq!(
        nyse.sessions_distance(date("2026-11-26"), date("2026-11-26"))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn a_product_picks_its_own_sessions() {
    let s = server().await;
    let c = client(&s);
    let range = date("2026-01-01")..=date("2026-12-31");
    let whole = c.calendar("XCEC", range.clone()).await.unwrap();
    assert!(whole.products().iter().any(|p| p == "Equity Index"));
    assert_eq!(
        whole.day(date("2026-11-03")).unwrap().sessions[0]
            .products
            .as_deref(),
        Some("Equity Index")
    );
    let pick = |p: &str| CalendarOptions {
        product: Some(p.into()),
        ..Default::default()
    };
    let equity = c
        .calendar_with("XCEC", range.clone(), pick("Equity Index"))
        .await
        .unwrap();
    assert!(
        equity
            .is_open_at_time(instant("2026-11-03T15:00:00Z"))
            .unwrap()
    );
    let metals = c
        .calendar_with("XCEC", range.clone(), pick("Energy and Metals"))
        .await
        .unwrap();
    assert!(
        metals.session_open(date("2026-11-03")).unwrap().is_none(),
        "never another product's hours"
    );
    assert!(matches!(
        c.calendar_with("XCEC", range, pick("no such product"))
            .await,
        Err(Error::InvalidArgument(_))
    ));
}

#[tokio::test]
async fn a_holiday_that_trades_is_a_session() {
    let nse = cal("XNSE").await;
    assert_eq!(
        nse.day(date("2026-11-08")).unwrap().kind,
        DayKindDto::Holiday
    );
    assert!(nse.is_session(date("2026-11-08")).unwrap(), "Muhurat");
    assert!(
        nse.is_open_at_time(instant("2026-11-08T09:30:00Z"))
            .unwrap()
    );
    assert!(
        !nse.is_session(date("2026-11-10")).unwrap(),
        "a holiday without one"
    );
    assert_eq!(
        nse.next_session(date("2026-11-07")).unwrap(),
        date("2026-11-08")
    );
}

#[tokio::test]
async fn an_unknown_day_is_neither_open_nor_closed() {
    let s = server().await;
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/v1_venues_XNYS_sessions__from-2026-01-01_to-2026-12-31.json"
    );
    let mut year: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    for day in year["days"].as_array_mut().unwrap() {
        if day["date"] == "2026-11-26" {
            *day = serde_json::json!({"date": "2026-11-26", "kind": "unknown", "verification": "inferredadvisory"});
        }
    }
    Mock::given(path("/v1/venues/XNYS/sessions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(year))
        .with_priority(1)
        .mount(&s)
        .await;
    let x = client(&s)
        .calendar("XNYS", date("2026-01-01")..=date("2026-12-31"))
        .await
        .unwrap();
    for outcome in [
        x.is_session(date("2026-11-26")).map(|_| ()),
        x.next_session(date("2026-11-25")).map(|_| ()),
        x.sessions_in_range(date("2026-11-20")..=date("2026-11-30"))
            .map(|_| ()),
        x.sessions_distance(date("2026-11-20"), date("2026-11-30"))
            .map(|_| ()),
    ] {
        assert!(
            matches!(outcome, Err(Error::Indeterminate { date: Some(d), .. }) if d == date("2026-11-26"))
        );
    }
}

#[tokio::test]
async fn a_calendar_is_named_by_its_code() {
    let x = cal("XNYS").await;
    assert_eq!(x.name(), "XNYS");
    assert_eq!(x.market_name(), "NEW YORK STOCK EXCHANGE, INC.");
    assert_eq!(x.tz_name(), "America/New_York");
    assert_eq!(x.range(), date("2026-01-01")..=date("2026-12-31"));
}

#[tokio::test]
async fn an_unknown_mic_is_refused() {
    let s = server().await;
    let outcome = client(&s)
        .calendar("ZZZZ", date("2026-01-01")..=date("2026-12-31"))
        .await;
    assert!(matches!(outcome, Err(Error::UnknownVenue(_))));
}
