//! Armazenamento SQLite (WAL) de metadados do content-machine.

mod migrations;

use std::path::{Path, PathBuf};

use cm_domain::{AudioTrack, Recording, Rect, TrackRole, WebcamDetection, WebcamSource};
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("erro no banco de dados: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("o banco tem esquema v{found}, mais novo que o suportado (v{supported})")]
    SchemaTooNew { found: u32, supported: u32 },
    #[error("papel de faixa desconhecido no banco: {0}")]
    UnknownRole(String),
    #[error("origem de webcam desconhecida no banco: {0}")]
    UnknownSource(String),
}

fn source_name(s: WebcamSource) -> &'static str {
    match s {
        WebcamSource::Override => "override",
        WebcamSource::Detected => "detected",
        WebcamSource::Saved => "saved",
    }
}

fn parse_source(s: &str) -> Result<WebcamSource, StoreError> {
    match s {
        "override" => Ok(WebcamSource::Override),
        "detected" => Ok(WebcamSource::Detected),
        "saved" => Ok(WebcamSource::Saved),
        other => Err(StoreError::UnknownSource(other.to_string())),
    }
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::migrate(&conn)?;
        Ok(Store { conn })
    }

    pub fn upsert_recording(&self, fingerprint: &str, rec: &Recording) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO recording (fingerprint, path, duration_s, width, height, fps)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(fingerprint) DO UPDATE SET
               path = excluded.path, duration_s = excluded.duration_s,
               width = excluded.width, height = excluded.height, fps = excluded.fps",
            params![
                fingerprint,
                rec.path.to_string_lossy(),
                rec.duration_s,
                rec.width,
                rec.height,
                rec.fps
            ],
        )?;
        tx.execute(
            "DELETE FROM audio_track WHERE fingerprint = ?1",
            params![fingerprint],
        )?;
        for t in &rec.audio_tracks {
            tx.execute(
                "INSERT INTO audio_track (fingerprint, idx, title, role) VALUES (?1, ?2, ?3, ?4)",
                params![fingerprint, t.index, t.title, t.role.as_str()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn find_recording(&self, fingerprint: &str) -> Result<Option<Recording>, StoreError> {
        let head = self
            .conn
            .query_row(
                "SELECT path, duration_s, width, height, fps FROM recording WHERE fingerprint = ?1",
                params![fingerprint],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, f64>(1)?,
                        r.get::<_, u32>(2)?,
                        r.get::<_, u32>(3)?,
                        r.get::<_, f64>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((path, duration_s, width, height, fps)) = head else {
            return Ok(None);
        };
        let mut stmt = self.conn.prepare(
            "SELECT idx, title, role FROM audio_track WHERE fingerprint = ?1 ORDER BY idx",
        )?;
        let rows = stmt.query_map(params![fingerprint], |r| {
            Ok((
                r.get::<_, u32>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut audio_tracks = Vec::new();
        for row in rows {
            let (index, title, role) = row?;
            let role = TrackRole::parse(&role).ok_or(StoreError::UnknownRole(role))?;
            audio_tracks.push(AudioTrack { index, title, role });
        }
        Ok(Some(Recording {
            path: PathBuf::from(path),
            duration_s,
            width,
            height,
            fps,
            audio_tracks,
        }))
    }

    /// Guarda (ou troca) o retângulo da webcam da gravação, com a resolução da fonte.
    pub fn save_webcam(
        &self,
        fingerprint: &str,
        d: &WebcamDetection,
        width: u32,
        height: u32,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO webcam
               (fingerprint, x, y, w, h, confidence, source, width, height)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                fingerprint,
                d.rect.x,
                d.rect.y,
                d.rect.w,
                d.rect.h,
                d.confidence,
                source_name(d.source),
                width,
                height
            ],
        )?;
        Ok(())
    }

    pub fn find_webcam(&self, fingerprint: &str) -> Result<Option<WebcamDetection>, StoreError> {
        let row = self
            .conn
            .query_row(
                "SELECT x, y, w, h, confidence, source FROM webcam WHERE fingerprint = ?1",
                params![fingerprint],
                |r| {
                    Ok((
                        r.get::<_, u32>(0)?,
                        r.get::<_, u32>(1)?,
                        r.get::<_, u32>(2)?,
                        r.get::<_, u32>(3)?,
                        r.get::<_, f64>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()?;
        row.map(|(x, y, w, h, confidence, source)| {
            Ok(WebcamDetection {
                rect: Rect { x, y, w, h },
                confidence,
                source: parse_source(&source)?,
            })
        })
        .transpose()
    }

    /// Retângulo detectado mais recente numa gravação da mesma resolução
    /// (só detecções de verdade: substituições manuais e reaproveitamentos não valem).
    pub fn last_webcam_for_resolution(
        &self,
        width: u32,
        height: u32,
    ) -> Result<Option<Rect>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT x, y, w, h FROM webcam
                 WHERE width = ?1 AND height = ?2 AND source = 'detected'
                 ORDER BY rowid DESC LIMIT 1",
                params![width, height],
                |r| {
                    Ok(Rect {
                        x: r.get(0)?,
                        y: r.get(1)?,
                        w: r.get(2)?,
                        h: r.get(3)?,
                    })
                },
            )
            .optional()?)
    }
}
