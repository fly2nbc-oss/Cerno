//! The `images` and `faces` tables: what the analysis found, keyed by fingerprint.

use super::*;

impl Db {
    pub fn put_sharpness(&self, fingerprint: u64, value: f32, version: i64) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, sharpness, sharpness_version) VALUES (?1, ?2, ?3)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET sharpness = excluded.sharpness, sharpness_version = excluded.sharpness_version",
            )?
            .execute(params![fingerprint as i64, f64::from(value), version])?;
        Ok(())
    }

    pub fn put_aesthetic(
        &self,
        fingerprint: u64,
        value: f32,
        model: &str,
        embedding: &[f32],
    ) -> Result<()> {
        let embedding: Vec<u8> = embedding.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, aesthetic, aesthetic_model, embedding)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET aesthetic = excluded.aesthetic, aesthetic_model = excluded.aesthetic_model,
                     embedding = excluded.embedding",
            )?
            .execute(params![
                fingerprint as i64,
                f64::from(value),
                model,
                embedding
            ])?;
        Ok(())
    }

    pub fn put_aesthetic25(&self, fingerprint: u64, value: f32, model: &str) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, aesthetic25, aesthetic25_model) VALUES (?1, ?2, ?3)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET aesthetic25 = excluded.aesthetic25,
                     aesthetic25_model = excluded.aesthetic25_model",
            )?
            .execute(params![fingerprint as i64, f64::from(value), model])?;
        Ok(())
    }

    pub fn put_exposure(
        &self,
        fingerprint: u64,
        highlights: f32,
        shadows: f32,
        version: i64,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, highlights, shadows, exposure_version)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET highlights = excluded.highlights, shadows = excluded.shadows,
                     exposure_version = excluded.exposure_version",
            )?
            .execute(params![
                fingerprint as i64,
                f64::from(highlights),
                f64::from(shadows),
                version
            ])?;
        Ok(())
    }

    pub fn put_faces(
        &self,
        fingerprint: u64,
        eyes: Option<f32>,
        faces: u8,
        version: i64,
    ) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, eyes, faces, faces_version) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(fingerprint) DO UPDATE
                 SET eyes = excluded.eyes, faces = excluded.faces,
                     faces_version = excluded.faces_version",
            )?
            .execute(params![
                fingerprint as i64,
                eyes.map(f64::from),
                faces,
                version
            ])?;
        Ok(())
    }

    /// The faces found on a photo, in place of those stored before.
    pub fn put_face_rows(&self, fingerprint: u64, faces: &[FaceRow]) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM faces WHERE fingerprint = ?1",
            [fingerprint as i64],
        )?;
        for (idx, face) in faces.iter().enumerate() {
            let landmarks: Vec<u8> = face
                .landmarks
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            let [x, y, w, h] = face.bbox.map(f64::from);
            tx.execute(
                "INSERT INTO faces (fingerprint, idx, score, x, y, w, h, landmarks, eyes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    fingerprint as i64,
                    idx as i64,
                    f64::from(face.score),
                    x,
                    y,
                    w,
                    h,
                    landmarks,
                    face.eyes.map(f64::from)
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The faces stored for a photo, in the order they were found; `None` when none were
    /// stored – the photo was analysed before 1.7, or has no face (`Scores::faces` says which).
    pub fn faces_of(&self, fingerprint: u64) -> Result<Option<Vec<FaceRow>>> {
        let conn = self.conn();
        let mut stmt = conn.prepare_cached(
            "SELECT score, x, y, w, h, landmarks, eyes FROM faces WHERE fingerprint = ?1
             ORDER BY idx",
        )?;
        let rows: Vec<FaceRow> = stmt
            .query_map([fingerprint as i64], |row| {
                let points = blob_to_f32(&row.get::<_, Vec<u8>>(5)?);
                let mut landmarks = [[0.0; 2]; 5];
                for (i, point) in landmarks.iter_mut().enumerate() {
                    *point = [
                        points.get(2 * i).copied().unwrap_or(0.0),
                        points.get(2 * i + 1).copied().unwrap_or(0.0),
                    ];
                }
                Ok(FaceRow {
                    score: row.get::<_, f64>(0)? as f32,
                    bbox: [
                        row.get::<_, f64>(1)? as f32,
                        row.get::<_, f64>(2)? as f32,
                        row.get::<_, f64>(3)? as f32,
                        row.get::<_, f64>(4)? as f32,
                    ],
                    landmarks,
                    eyes: opt_f32(row, 6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok((!rows.is_empty()).then_some(rows))
    }

    pub fn put_thumbnail(&self, fingerprint: u64, jpeg: &[u8]) -> Result<()> {
        self.conn()
            .prepare_cached(
                "INSERT INTO images (fingerprint, thumbnail) VALUES (?1, ?2)
                 ON CONFLICT(fingerprint) DO UPDATE SET thumbnail = excluded.thumbnail",
            )?
            .execute(params![fingerprint as i64, jpeg])?;
        Ok(())
    }

    pub fn thumbnail(&self, fingerprint: u64) -> Result<Option<Vec<u8>>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare_cached("SELECT thumbnail FROM images WHERE fingerprint = ?1")?;
        let thumb = stmt
            .query_row(params![fingerprint as i64], |row| {
                row.get::<_, Option<Vec<u8>>>(0)
            })
            .optional()?;
        Ok(thumb.flatten())
    }
}
