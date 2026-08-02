//! Local-first persistence: SQLite WAL, immutable snapshots, current-pointer flip.
//!
//! Doctrine §5: *Results are immutable snapshots with a current-pointer flip.
//! Never mutate a computed result in place.* A snapshot row is written once and
//! never updated; making a result current is a single-row update to a pointer
//! table inside the same transaction. Recomputing therefore never destroys the
//! prior underwrite, and "what did this deal look like last Tuesday" is a query
//! rather than an archaeology project.
//!
//! Doctrine §5: *Local-first is a security decision.* The database lives on the
//! operator's disk. Nothing in this crate opens a network socket.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use rusqlite::{Connection, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

/// Errors from the store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// SQLite failure.
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Serialization failure.
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    /// No snapshot is current for the requested subject.
    #[error("no current snapshot for subject {0}")]
    NoCurrent(Uuid),
}

/// Result alias for store operations.
pub type Result<T> = std::result::Result<T, StoreError>;

/// Schema applied on open. Additive migrations only.
const SCHEMA: &str = r"
CREATE TABLE IF NOT EXISTS snapshot (
    id            TEXT PRIMARY KEY NOT NULL,
    subject_id    TEXT NOT NULL,
    kind          TEXT NOT NULL,
    payload       TEXT NOT NULL,
    engine_version TEXT NOT NULL,
    created_at    TEXT NOT NULL
) STRICT;

CREATE INDEX IF NOT EXISTS snapshot_subject_idx ON snapshot(subject_id, created_at DESC);

CREATE TABLE IF NOT EXISTS current_snapshot (
    subject_id  TEXT PRIMARY KEY NOT NULL,
    snapshot_id TEXT NOT NULL REFERENCES snapshot(id),
    flipped_at  TEXT NOT NULL
) STRICT;
";

/// A handle to the local VELT database.
#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (creating if absent) the database at `path` in WAL mode.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] if the file cannot be opened or the schema fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::configure(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory database, for tests.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`].
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure(&conn)?;
        Ok(Self { conn })
    }

    fn configure(conn: &Connection) -> Result<()> {
        // WAL gives readers a consistent view while the daemon writes, which is
        // what makes a redraw-on-every-keystroke terminal UI viable.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    /// Write an immutable snapshot and flip the current pointer to it, atomically.
    ///
    /// `created_at` is supplied by the caller rather than read from a clock, so
    /// the store stays as testable as the engine.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`] or [`StoreError::Serde`].
    pub fn put_snapshot<T: serde::Serialize>(
        &mut self,
        subject_id: Uuid,
        kind: &str,
        payload: &T,
        engine_version: &str,
        created_at: &str,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let json = serde_json::to_string(payload)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO snapshot (id, subject_id, kind, payload, engine_version, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id.to_string(),
                subject_id.to_string(),
                kind,
                json,
                engine_version,
                created_at
            ],
        )?;
        tx.execute(
            "INSERT INTO current_snapshot (subject_id, snapshot_id, flipped_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(subject_id) DO UPDATE SET snapshot_id = ?2, flipped_at = ?3",
            rusqlite::params![subject_id.to_string(), id.to_string(), created_at],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Read the snapshot currently pointed at for a subject.
    ///
    /// # Errors
    /// [`StoreError::NoCurrent`] if nothing is current, or a sqlite/serde error.
    pub fn current<T: serde::de::DeserializeOwned>(&self, subject_id: Uuid) -> Result<T> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT s.payload FROM current_snapshot c
                 JOIN snapshot s ON s.id = c.snapshot_id
                 WHERE c.subject_id = ?1",
                rusqlite::params![subject_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let json = json.ok_or(StoreError::NoCurrent(subject_id))?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Count snapshots retained for a subject.
    ///
    /// # Errors
    /// [`StoreError::Sqlite`].
    pub fn history_len(&self, subject_id: Uuid) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM snapshot WHERE subject_id = ?1",
            rusqlite::params![subject_id.to_string()],
            |row| row.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Fixture {
        noi_minor: i64,
    }

    #[test]
    fn a_flip_replaces_the_pointer_and_retains_the_history() {
        let mut store = Store::open_in_memory().unwrap();
        let subject = Uuid::new_v4();

        store
            .put_snapshot(
                subject,
                "underwrite",
                &Fixture {
                    noi_minor: 1_227_972,
                },
                "0.1.0",
                "2026-08-01T00:00:00Z",
            )
            .unwrap();
        store
            .put_snapshot(
                subject,
                "underwrite",
                &Fixture {
                    noi_minor: 1_300_000,
                },
                "0.1.0",
                "2026-08-02T00:00:00Z",
            )
            .unwrap();

        let current: Fixture = store.current(subject).unwrap();
        assert_eq!(
            current,
            Fixture {
                noi_minor: 1_300_000
            },
            "pointer flipped"
        );
        assert_eq!(
            store.history_len(subject).unwrap(),
            2,
            "prior snapshot retained"
        );
    }

    #[test]
    fn an_unknown_subject_has_no_current_snapshot() {
        let store = Store::open_in_memory().unwrap();
        let missing = store.current::<Fixture>(Uuid::new_v4());
        assert!(matches!(missing, Err(StoreError::NoCurrent(_))));
    }
}
