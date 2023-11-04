use std::fmt::Display;

use typed_builder::TypedBuilder;

use crate::field::Field;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    value: time::OffsetDateTime,
}

impl Field for DateTime {}

impl DateTime {
    pub fn now_local() -> Result<Self, CannotDetermineTimezone> {
        let value: time::OffsetDateTime = match time::OffsetDateTime::now_local() {
            Ok(value) => value,
            Err(err) => {
                let cannot_determine_timezone = CannotDetermineTimezone::builder().build();
                #[cfg(feature = "logging")]
                log::debug!("{}", cannot_determine_timezone);
                return Err(cannot_determine_timezone);
            }
        };

        Ok(Self { value })
    }

    pub fn now_local_or_utc() -> Self {
        match time::OffsetDateTime::now_local() {
            Ok(value) => Self { value },
            Err(err) => Self {
                value: time::OffsetDateTime::now_utc(),
            },
        }
    }
}

#[derive(TypedBuilder, Debug)]
pub struct CannotDetermineTimezone {}

impl Display for CannotDetermineTimezone {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(formatter, "Cannot determine the timezeone")
    }
}
