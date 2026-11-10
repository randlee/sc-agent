//! [`ReleaseDate`], the ordering key for "newest release wins".

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::TypesError;

/// A model's release date, `YYYY-MM-DD`: an ordering key for "newest release wins".
///
/// It is not a calendar date and does not validate the calendar: day 1..=31 is accepted for
/// every month (for example `2026-02-31`). The derived `Ord` compares year, then month, then day.
///
/// # Examples
///
/// ```
/// use sc_agent_types::ReleaseDate;
///
/// let early: ReleaseDate = "2026-09-15".parse().unwrap();
/// let late: ReleaseDate = "2026-10-14".parse().unwrap();
/// assert!(early < late);
/// assert_eq!(late.to_string(), "2026-10-14");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReleaseDate {
    year: u16,
    month: u8,
    day: u8,
}

impl ReleaseDate {
    /// A date from its parts; callers pass month 1..=12 and day 1..=31.
    pub(crate) const fn ymd(year: u16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }
}

impl FromStr for ReleaseDate {
    type Err = TypesError;

    /// Parses `YYYY-MM-DD` with month 1..=12 and day 1..=31.
    ///
    /// # Errors
    ///
    /// [`TypesError::InvalidReleaseDate`] for any other text.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bytes = text.as_bytes();
        let digits = |range: std::ops::Range<usize>| {
            bytes
                .get(range)
                .filter(|part| part.iter().all(u8::is_ascii_digit))
                .and_then(|part| std::str::from_utf8(part).ok())
                .and_then(|part| part.parse::<u16>().ok())
        };
        let parsed =
            if bytes.len() == 10 && bytes.get(4) == Some(&b'-') && bytes.get(7) == Some(&b'-') {
                let year = digits(0..4);
                let month = digits(5..7)
                    .filter(|month| (1..=12).contains(month))
                    .and_then(|month| u8::try_from(month).ok());
                let day = digits(8..10)
                    .filter(|day| (1..=31).contains(day))
                    .and_then(|day| u8::try_from(day).ok());
                year.zip(month)
                    .zip(day)
                    .map(|((year, month), day)| Self::ymd(year, month, day))
            } else {
                None
            };
        parsed.ok_or_else(|| TypesError::InvalidReleaseDate {
            text: text.to_owned(),
        })
    }
}

impl fmt::Display for ReleaseDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}

impl Serialize for ReleaseDate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ReleaseDate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}
