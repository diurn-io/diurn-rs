#![doc = include_str!("../README.md")]

mod calendar;
mod client;
mod error;
pub mod generated;
mod ops;
mod request;

#[cfg(feature = "blocking")]
pub mod blocking;

pub use calendar::{
    Calendar, CalendarOptions, Day, DayKind, Direction, ScheduleRow, Session, Verification,
};
pub use client::Client;
pub use diurn_mic::Mic;
pub use error::{ApiErrorBody, Error, Result};
pub use generated::models;
pub use ops::{
    ExceptionsOptions, IntoMic, JointOptions, Mode, OnUnknown, SessionsOptions, StepOptions,
    VenuesQuery,
};
pub use request::ClientBuilder;
