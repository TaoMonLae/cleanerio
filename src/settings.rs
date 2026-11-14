use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const GB: u64 = 1_000_000_000;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub smart_cleanup: bool,
    pub smart_performance: bool,
    pub trash_alert_gb: u64,
    pub low_disk_gb: u64,
    pub scan_reminder_days: u64,
    pub ignore: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            smart_cleanup: true,
            smart_performance: true,
            trash_alert_gb: 2,
            low_disk_gb: 0,
            scan_reminder_days: 7,
            ignore: vec![],
        }
    }
}

fn file() -> PathBuf {
    crate::scan::app_data_dir().join("settings.json")
}

pub fn expand(entry: &str) -> PathBuf {
    match entry.strip_prefix("~/") {
        Some(rest) => crate::scan::home().join(rest),
        None if entry == "~" => crate::scan::home(),
        None => PathBuf::from(entry),
    }
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(file())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let f = file();
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(f, t);
        }
    }

    pub fn trash_alert_bytes(&self) -> u64 {
        self.trash_alert_gb.max(1) * GB
    }

    pub fn low_disk_bytes(&self, disk_total: u64) -> u64 {
        if self.low_disk_gb == 0 {
            crate::scan::low_disk_limit(disk_total)
        } else {
            self.low_disk_gb * GB
        }
    }

    pub fn ignored(&self, path: &Path) -> bool {
        self.ignore
            .iter()
            .any(|e| !e.trim().is_empty() && path.starts_with(expand(e.trim())))
    }

    pub fn add_ignore(&mut self, entry: &str) -> bool {
        let e = entry.trim().trim_end_matches(['/', '\\']).to_string();
        if e.is_empty() || e == "~" || self.ignore.contains(&e) {
            return false;
        }
        self.ignore.push(e);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_fill_missing_keys() {
        let s: Settings = serde_json::from_str(r#"{"trash_alert_gb":5}"#).unwrap();
        assert_eq!(s.trash_alert_bytes(), 5 * GB);
        assert!(s.smart_cleanup && s.smart_performance);
        assert_eq!(s.scan_reminder_days, 7);
    }

    #[test]
    fn ignore_list_matches_whole_folders_only() {
        let mut s = Settings::default();
        assert!(s.add_ignore("~/Projects/keep/"));
        assert!(!s.add_ignore("~/Projects/keep"));
        assert!(!s.add_ignore("  "));
        assert!(!s.add_ignore("~"));
        let h = crate::scan::home();
        assert!(s.ignored(&h.join("Projects/keep/cache/a.bin")));
        assert!(!s.ignored(&h.join("Projects/keeper/a.bin")));
        assert!(!s.ignored(&h.join("Projects")));
    }

    #[test]
    fn low_disk_limit_is_automatic_until_set() {
        let mut s = Settings::default();
        assert_eq!(
            s.low_disk_bytes(494 * GB),
            crate::scan::low_disk_limit(494 * GB)
        );
        s.low_disk_gb = 20;
        assert_eq!(s.low_disk_bytes(494 * GB), 20 * GB);
    }
}
