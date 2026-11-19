use rusqlite::Connection;

use crate::StoreError;

const SCHEMA_VERSION: u32 = 2;

pub fn migrate(conn: &Connection) -> Result<(), StoreError> {
    let found: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if found > SCHEMA_VERSION {
        return Err(StoreError::SchemaTooNew {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    if found < 1 {
        conn.execute_batch(
            "CREATE TABLE recording (
               fingerprint TEXT PRIMARY KEY,
               path TEXT NOT NULL,
               duration_s REAL NOT NULL,
               width INTEGER NOT NULL,
               height INTEGER NOT NULL,
               fps REAL NOT NULL
             );
             CREATE TABLE audio_track (
               fingerprint TEXT NOT NULL REFERENCES recording(fingerprint) ON DELETE CASCADE,
               idx INTEGER NOT NULL,
               title TEXT,
               role TEXT NOT NULL,
               PRIMARY KEY (fingerprint, idx)
             );",
        )?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    if found < 2 {
        conn.execute_batch(
            "CREATE TABLE webcam (
               fingerprint TEXT PRIMARY KEY REFERENCES recording(fingerprint) ON DELETE CASCADE,
               x INTEGER NOT NULL,
               y INTEGER NOT NULL,
               w INTEGER NOT NULL,
               h INTEGER NOT NULL,
               confidence REAL NOT NULL,
               source TEXT NOT NULL,
               width INTEGER NOT NULL,
               height INTEGER NOT NULL
             );",
        )?;
        conn.pragma_update(None, "user_version", 2)?;
    }
    Ok(())
}
