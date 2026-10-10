//! Saved settings: one text value per key.

use super::*;

impl Db {
    pub fn setting(&self, key: &str) -> Option<String> {
        self.conn()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }

    /// A switch, saved as `"1"` / `"0"`.
    pub fn put_flag(&self, key: &str, on: bool) {
        self.put_setting(key, if on { "1" } else { "0" });
    }

    pub fn put_setting(&self, key: &str, value: &str) {
        let result = self.conn().execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            [key, value],
        );
        if let Err(err) = result {
            log::warn!("cannot save setting {key}: {err}");
        }
    }
}
