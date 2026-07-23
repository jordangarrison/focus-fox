use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use crate::timer::Phase;

/// One finished or skipped phase, as written to the history log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// End of the phase, local time.
    pub at: DateTime<FixedOffset>,
    pub phase: Phase,
    /// Configured phase length when it ran.
    pub planned_secs: u64,
    /// Timer time actually elapsed (pauses excluded).
    pub actual_secs: u64,
    /// True for a natural finish, false for a skip.
    pub completed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trips_through_json() {
        let record = Record {
            at: DateTime::parse_from_rfc3339("2026-07-23T14:30:00-05:00").unwrap(),
            phase: Phase::Work,
            planned_secs: 1500,
            actual_secs: 1400,
            completed: true,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"phase\":\"work\""), "json was: {json}");
        assert!(json.contains("\"completed\":true"));
        let back: Record = serde_json::from_str(&json).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn break_phases_use_snake_case() {
        assert_eq!(
            serde_json::to_string(&Phase::ShortBreak).unwrap(),
            "\"short_break\""
        );
    }
}
