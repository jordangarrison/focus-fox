use std::collections::HashSet;
use std::time::Duration;

use chrono::{DateTime, Days, FixedOffset, NaiveDate, Weekday};
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

/// Aggregates over the history, computed as a pure fold — no IO here.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub today_sessions: u32,
    pub today_focus: Duration,
    pub week_sessions: u32,
    pub week_focus: Duration,
    /// Consecutive days with >=1 completed work session, counting back from
    /// today — or from yesterday when today has none yet.
    pub streak_days: u32,
    pub lifetime_sessions: u32,
    pub lifetime_focus: Duration,
    /// Last 10 records, newest first.
    pub recent: Vec<Record>,
}

impl Summary {
    pub fn compute(records: &[Record], today: NaiveDate) -> Self {
        let week_start = today.week(Weekday::Mon).first_day();
        let mut s = Summary {
            today_sessions: 0,
            today_focus: Duration::ZERO,
            week_sessions: 0,
            week_focus: Duration::ZERO,
            streak_days: 0,
            lifetime_sessions: 0,
            lifetime_focus: Duration::ZERO,
            recent: records.iter().rev().take(10).cloned().collect(),
        };

        let mut completed_days: HashSet<NaiveDate> = HashSet::new();
        for r in records {
            if r.phase != Phase::Work {
                continue;
            }
            let date = r.at.date_naive();
            let focus = Duration::from_secs(r.actual_secs);
            let in_week = (week_start..=today).contains(&date);
            s.lifetime_focus += focus;
            if date == today {
                s.today_focus += focus;
            }
            if in_week {
                s.week_focus += focus;
            }
            if r.completed {
                s.lifetime_sessions += 1;
                completed_days.insert(date);
                if date == today {
                    s.today_sessions += 1;
                }
                if in_week {
                    s.week_sessions += 1;
                }
            }
        }

        let mut day = if completed_days.contains(&today) {
            today
        } else {
            today - Days::new(1)
        };
        while completed_days.contains(&day) {
            s.streak_days += 1;
            day = day - Days::new(1);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::time::Duration;

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

    fn rec(day: &str, time: &str, phase: Phase, actual_mins: u64, completed: bool) -> Record {
        Record {
            at: DateTime::parse_from_rfc3339(&format!("{day}T{time}:00-05:00")).unwrap(),
            phase,
            planned_secs: 1500,
            actual_secs: actual_mins * 60,
            completed,
        }
    }

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    // 2024-01-01 was a Monday; "today" is Wednesday 2024-01-03 throughout.
    const TODAY: &str = "2024-01-03";

    #[test]
    fn today_counts_completed_sessions_but_credits_partial_focus() {
        let records = vec![
            rec(TODAY, "09:00", Phase::Work, 25, true),
            rec(TODAY, "09:40", Phase::Work, 10, false), // skip: focus counts, session doesn't
            rec(TODAY, "10:10", Phase::ShortBreak, 5, true), // breaks never count as focus
            rec("2024-01-02", "10:00", Phase::Work, 25, true), // yesterday
        ];
        let s = Summary::compute(&records, d(TODAY));
        assert_eq!(s.today_sessions, 1);
        assert_eq!(s.today_focus, Duration::from_secs(35 * 60));
    }

    #[test]
    fn week_starts_monday() {
        let records = vec![
            rec("2023-12-31", "10:00", Phase::Work, 25, true), // Sunday: last week
            rec("2024-01-01", "10:00", Phase::Work, 25, true), // Monday: this week
            rec(TODAY, "10:00", Phase::Work, 25, true),
        ];
        let s = Summary::compute(&records, d(TODAY));
        assert_eq!(s.week_sessions, 2);
        assert_eq!(s.week_focus, Duration::from_secs(50 * 60));
        assert_eq!(s.lifetime_sessions, 3);
        assert_eq!(s.lifetime_focus, Duration::from_secs(75 * 60));
    }

    #[test]
    fn streak_counts_consecutive_days() {
        let records = vec![
            rec("2024-01-01", "10:00", Phase::Work, 25, true),
            rec("2024-01-02", "10:00", Phase::Work, 25, true),
            rec(TODAY, "00:10", Phase::Work, 25, true), // just after midnight still today
        ];
        assert_eq!(Summary::compute(&records, d(TODAY)).streak_days, 3);
    }

    #[test]
    fn streak_breaks_on_a_gap() {
        let records = vec![
            rec("2024-01-01", "10:00", Phase::Work, 25, true),
            rec(TODAY, "10:00", Phase::Work, 25, true), // nothing on the 2nd
        ];
        assert_eq!(Summary::compute(&records, d(TODAY)).streak_days, 1);
    }

    #[test]
    fn empty_morning_does_not_break_the_streak() {
        let records = vec![
            rec("2024-01-01", "10:00", Phase::Work, 25, true),
            rec("2024-01-02", "10:00", Phase::Work, 25, true),
        ];
        // No session yet today: streak counts back from yesterday.
        assert_eq!(Summary::compute(&records, d(TODAY)).streak_days, 2);
    }

    #[test]
    fn skips_and_breaks_do_not_extend_a_streak() {
        let records = vec![
            rec("2024-01-02", "10:00", Phase::Work, 10, false),
            rec(TODAY, "10:00", Phase::ShortBreak, 5, true),
        ];
        assert_eq!(Summary::compute(&records, d(TODAY)).streak_days, 0);
    }

    #[test]
    fn recent_is_last_ten_newest_first() {
        let records: Vec<Record> = (0..12)
            .map(|i| rec(TODAY, &format!("{:02}:00", 8 + i), Phase::Work, 25, true))
            .collect();
        let s = Summary::compute(&records, d(TODAY));
        assert_eq!(s.recent.len(), 10);
        assert_eq!(s.recent[0], records[11]);
        assert_eq!(s.recent[9], records[2]);
    }
}
