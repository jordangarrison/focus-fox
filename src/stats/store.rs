use anyhow::{Context, Result};
use chrono::Datelike;
use directories::ProjectDirs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use super::Record;

/// Appends history records to, and loads them from, year-partitioned JSONL
/// files (`history-<year>.jsonl`) in the XDG data directory.
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// XDG data dir (~/.local/share/focus-fox on Linux) — same ProjectDirs
    /// triple as Config, but the data dir rather than the config dir.
    pub fn default_dir() -> Option<PathBuf> {
        ProjectDirs::from("dev", "jordangarrison", "focus-fox")
            .map(|dirs| dirs.data_dir().to_path_buf())
    }

    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Append one record to the file for the record's own year.
    pub fn append(&self, record: &Record) -> Result<()> {
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("creating {}", self.dir.display()))?;
        let path = self.dir.join(format!("history-{}.jsonl", record.at.year()));
        let line = serde_json::to_string(record).context("serializing history record")?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        writeln!(file, "{line}").with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Load `year - 1` and `year` — enough for every stat we compute
    /// (streaks and weeks can span New Year's). Missing files are empty
    /// history; unparseable lines are skipped.
    pub fn load_recent(&self, year: i32) -> Vec<Record> {
        let mut records = Vec::new();
        for y in [year - 1, year] {
            let path = self.dir.join(format!("history-{y}.jsonl"));
            let Ok(contents) = std::fs::read_to_string(&path) else {
                continue;
            };
            records.extend(
                contents
                    .lines()
                    .filter_map(|line| serde_json::from_str::<Record>(line).ok()),
            );
        }
        records
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::Phase;
    use chrono::DateTime;

    fn rec(rfc3339: &str, completed: bool) -> Record {
        Record {
            at: DateTime::parse_from_rfc3339(rfc3339).unwrap(),
            phase: Phase::Work,
            planned_secs: 1500,
            actual_secs: 1500,
            completed,
        }
    }

    #[test]
    fn append_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let a = rec("2024-01-02T10:00:00-05:00", true);
        let b = rec("2024-01-02T11:00:00-05:00", false);
        store.append(&a).unwrap();
        store.append(&b).unwrap();
        assert_eq!(store.load_recent(2024), vec![a, b]);
    }

    #[test]
    fn records_land_in_their_years_file_and_both_years_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let old = rec("2023-12-31T23:00:00-05:00", true);
        let new = rec("2024-01-01T10:00:00-05:00", true);
        store.append(&old).unwrap();
        store.append(&new).unwrap();
        assert!(dir.path().join("history-2023.jsonl").exists());
        assert!(dir.path().join("history-2024.jsonl").exists());
        assert_eq!(store.load_recent(2024), vec![old.clone(), new]);
        // A load from 2023's perspective doesn't see the future file.
        assert_eq!(store.load_recent(2023), vec![old]);
    }

    #[test]
    fn corrupt_lines_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        let good = rec("2024-01-02T10:00:00-05:00", true);
        store.append(&good).unwrap();
        std::fs::write(
            dir.path().join("history-2024.jsonl"),
            format!("not json\n{}\n", serde_json::to_string(&good).unwrap()),
        )
        .unwrap();
        assert_eq!(store.load_recent(2024), vec![good]);
    }

    #[test]
    fn missing_files_mean_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().to_path_buf());
        assert_eq!(store.load_recent(2024), Vec::<Record>::new());
    }
}
