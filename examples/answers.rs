//! A few answers from a live API, to compare with another client's:
//!
//! ```sh
//! DIURN_API_URL=http://127.0.0.1:8099 cargo run --example answers
//! ```

use diurn::{Client, JointOptions, StepOptions};
use jiff::civil::date;

#[tokio::main]
async fn main() -> diurn::Result<()> {
    let client = Client::new()?;
    let nov = |d: i8| date(2026, 11, d);
    let xnys = client
        .calendar("XNYS", date(2026, 1, 1)..=date(2027, 12, 31))
        .await?;
    println!(
        "XNYS sessions in Nov 2026: {}",
        xnys.sessions_distance(nov(1), nov(30))?
    );
    println!(
        "XNYS next session after 2026-12-31: {}",
        xnys.next_session(date(2026, 12, 31))?
    );
    println!(
        "XNYS close on 2026-11-27: {}",
        xnys.session_close(nov(27))?.expect("held")
    );
    let xtks = client.calendar("XTKS", nov(1)..=nov(30)).await?;
    let lunch = xtks.session_break_start(nov(2))?.expect("a lunch");
    println!(
        "XTKS lunch on 2026-11-02 starts: {}",
        lunch.strftime("%H:%M")
    );
    let xnse = client.calendar("XNSE", nov(1)..=nov(30)).await?;
    println!("XNSE 2026-11-08 is a session: {}", xnse.is_session(nov(8))?);
    let step = client
        .next_trading_day("XNYS", nov(25), StepOptions::default())
        .await?;
    println!("XNYS next trading day after 2026-11-25: {}", step.date);
    let joint = client
        .joint_next_trading_day(
            ["XNYS", "XLON"],
            date(2026, 12, 23),
            JointOptions {
                n: Some(2),
                ..Default::default()
            },
        )
        .await?;
    println!("XNYS+XLON 2nd joint day after 2026-12-23: {}", joint.date);
    Ok(())
}
