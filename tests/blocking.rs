//! The blocking client answers as the async one does: the same operations,
//! generated from the same list, over the same core.
#![cfg(feature = "blocking")]

mod common;

use common::{client, date, server};
use diurn::{Error, StepOptions};

#[tokio::test(flavor = "multi_thread")]
async fn the_blocking_client_agrees_with_the_async_one() {
    let s = server().await;
    let expected = client(&s)
        .next_trading_day("XNYS", date("2026-11-25"), StepOptions::default())
        .await
        .unwrap();
    let uri = s.uri();
    // reqwest's blocking client runs its own runtime: off this one's threads.
    let (step, distance, refused) = tokio::task::spawn_blocking(move || {
        let c = diurn::blocking::Client::builder()
            .api_key("k")
            .base_url(uri)
            .build_blocking()
            .unwrap();
        let step = c
            .next_trading_day("XNYS", date("2026-11-25"), StepOptions::default())
            .unwrap();
        let cal = c
            .calendar("XNYS", date("2026-01-01")..=date("2026-12-31"))
            .unwrap();
        let distance = cal
            .sessions_distance(date("2026-11-01"), date("2026-11-30"))
            .unwrap();
        let refused = c.venue("ZZZZ").unwrap_err();
        (step, distance, refused)
    })
    .await
    .unwrap();
    assert_eq!(step, expected);
    assert_eq!(distance, 20);
    assert!(matches!(refused, Error::UnknownVenue(_)));
}
