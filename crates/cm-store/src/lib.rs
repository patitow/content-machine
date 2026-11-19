//! Armazenamento SQLite (WAL) de metadados do content-machine.

mod migrations;

use std::path::{Path, PathBuf};

use cm_domain::{AudioTrack, Recording, TrackRole};
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("erro no banco de dados: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("o banco tem esquema v{found}, mais novo que o suportado (v{supported})")]
    SchemaTooNew { found: u32, supported: u32 },
    #[error("papel de faixa desconhecido no banco: {0}")]
    UnknownRole(String),
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
}
