//! The examples the prediction learns from.

use super::*;

impl Db {
    /// Forgets taste training data while keeping star ratings in the photo files.
    pub fn reset_taste_learning(&self) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO taste_skip (fingerprint)
             SELECT fingerprint FROM files WHERE rating IS NOT NULL",
            [],
        )?;
        tx.execute("DELETE FROM feedback", [])?;
        tx.commit()?;
        Ok(())
    }

    /// A new rating or rejection counts again for the taste model.
    pub fn allow_taste_for(&self, fingerprint: u64) -> Result<()> {
        self.conn().execute(
            "DELETE FROM taste_skip WHERE fingerprint = ?1",
            [fingerprint as i64],
        )?;
        Ok(())
    }

    /// Training data for the taste model: (CLIP embedding, label 0–5), and where the examples
    /// come from. Stars count with their number, rejected photos as 0. Deleted photos (rows in
    /// `.originals`) don't count at all, whatever stars they had: a good photo is often deleted
    /// only because there are too many alike (the user's correction of 2026-10-05).
    pub fn taste_examples(&self) -> Result<(Vec<TasteExample>, TasteSources)> {
        let conn = self.conn();
        // The third column: 1 = stars, 2 = rejected.
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT i.embedding, CAST(MAX(MAX(f.rating, 0)) AS REAL),
                    CASE WHEN MAX(f.rating) >= 1 THEN 1 ELSE 2 END
             FROM files f JOIN images i ON i.fingerprint = f.fingerprint
             WHERE (f.rating BETWEEN 1 AND 5 OR f.rating = -1) AND i.embedding IS NOT NULL
               AND {}
               AND f.fingerprint NOT IN (SELECT fingerprint FROM taste_skip)
             GROUP BY f.fingerprint",
            live("f.path"),
        ))?;
        let mut sources = TasteSources::default();
        let mut examples = Vec::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            match row.get::<_, i64>(2)? {
                1 => sources.stars += 1,
                _ => sources.rejected += 1,
            }
            examples.push((
                blob_to_f32(&row.get::<_, Vec<u8>>(0)?),
                row.get::<_, f64>(1)? as f32,
            ));
        }
        Ok((examples, sources))
    }
}
