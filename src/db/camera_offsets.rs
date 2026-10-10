//! Camera clocks set right, per folder and camera model.

use super::*;

impl Db {
    /// The camera clocks set right in `folder`: camera id → milliseconds added.
    pub fn camera_offsets(&self, folder: &str) -> Result<HashMap<u64, i64>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare_cached("SELECT camera, offset_ms FROM camera_offsets WHERE folder = ?1")?;
        let rows = stmt.query_map([folder], |row| {
            Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Sets a camera's offset in `folder`; 0 removes it.
    pub fn set_camera_offset(&self, folder: &str, camera: u64, offset_ms: i64) -> Result<()> {
        let conn = self.conn();
        if offset_ms == 0 {
            conn.execute(
                "DELETE FROM camera_offsets WHERE folder = ?1 AND camera = ?2",
                params![folder, camera as i64],
            )?;
        } else {
            conn.execute(
                "INSERT OR REPLACE INTO camera_offsets (folder, camera, offset_ms)
                 VALUES (?1, ?2, ?3)",
                params![folder, camera as i64, offset_ms],
            )?;
        }
        Ok(())
    }
}
