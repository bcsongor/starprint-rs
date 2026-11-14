//! The relay's SQLite file: every line record it holds, and the faxes
//! waiting for their lines. A fax is deleted once its line confirms it
//! printed, or once it has waited past the hold, so the file holds only
//! what is still on its way. Numbers are kept as they are
//! written, `*star1en2su3z68yscvky0n3j3l2qwny4dkq7s`, so the file reads well in any
//! SQLite browser.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension as _, params};
use starprint_fax::{Fax, HOLD_SECONDS, Held, LineRecord, Number};

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS lines (
        number TEXT PRIMARY KEY,
        record TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS faxes (
        id TEXT PRIMARY KEY,
        recipient TEXT NOT NULL,
        sender TEXT NOT NULL,
        size INTEGER NOT NULL,
        arrived INTEGER NOT NULL,
        fax TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS waiting ON faxes (recipient, arrived);
    CREATE INDEX IF NOT EXISTS expiring ON faxes (arrived);
";

pub struct Store {
    db: Mutex<Connection>,
}

/// What became of a record handed to [`Store::put_line`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Put {
    /// Kept, for a number the relay did not have.
    Published,
    /// Kept in place of the record held before.
    Updated,
    /// Already held, so nothing was written.
    Unchanged,
    /// Turned away, since a newer record is held.
    Older,
    /// Turned away, since the relay holds as many lines as it may.
    Full,
}

/// How much is waiting for a line, and for every line together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Waiting {
    pub faxes: usize,
    pub bytes: usize,
    pub stored: u64,
}

impl Store {
    /// Opens the file at `path`, creating it if need be. `:memory:` is a
    /// store that lasts as long as the relay, for tests.
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = Connection::open(path)
            .map_err(|e| format!("{} could not be opened: {e}", path.display()))?;
        db.pragma_update(None, "journal_mode", "WAL")
            .and_then(|()| db.execute_batch(SCHEMA))
            .map_err(|e| format!("{} could not be set up: {e}", path.display()))?;
        Ok(Self { db: Mutex::new(db) })
    }

    /// The record held for `number`, if the relay has that line.
    pub fn line(&self, number: Number) -> rusqlite::Result<Option<LineRecord>> {
        line(&self.db.lock().unwrap(), number)
    }

    /// Keeps `record` unless a newer one is held, or it is for a new
    /// line and the relay already holds `limit` of them. Of two records
    /// for a number the one updated last wins. In one turn of the lock,
    /// so two new lines at once cannot both take the last place.
    pub fn put_line(&self, record: &LineRecord, limit: usize) -> rusqlite::Result<Put> {
        let db = self.db.lock().unwrap();
        let put = match line(&db, record.number)? {
            Some(held) if held.updated > record.updated => return Ok(Put::Older),
            Some(held) if held == *record => return Ok(Put::Unchanged),
            Some(_) => Put::Updated,
            None => {
                let lines: usize =
                    db.query_row("SELECT count(*) FROM lines", [], |row| row.get(0))?;
                if lines >= limit {
                    return Ok(Put::Full);
                }
                Put::Published
            }
        };
        db.execute(
            "INSERT INTO lines (number, record) VALUES (?1, ?2)
             ON CONFLICT (number) DO UPDATE SET record = excluded.record",
            params![record.number.to_string(), to_json(record)],
        )?;
        Ok(put)
    }

    /// Takes `held` at `now` unless `full` says the line already has
    /// too much waiting, once faxes past the hold are deleted. In one
    /// turn of the lock, so two faxes at once cannot both squeeze in.
    pub fn hold(
        &self,
        held: &Held,
        size: usize,
        now: u64,
        full: impl FnOnce(Waiting) -> bool,
    ) -> rusqlite::Result<bool> {
        let db = self.db.lock().unwrap();
        expire(&db, now)?;
        let waiting = waiting(&db, held.fax.to)?;
        if full(waiting) {
            return Ok(false);
        }
        db.execute(
            "INSERT INTO faxes (id, recipient, sender, size, arrived, fax)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                held.id,
                held.fax.to.to_string(),
                held.fax.from.to_string(),
                size,
                now,
                to_json(&held.fax)
            ],
        )?;
        Ok(true)
    }

    /// Deletes every line's faxes past the hold at `now`, then answers
    /// with those waiting for `number`, oldest first. Hiding the old
    /// ones instead would keep them until someone next faxed.
    pub fn waiting_for(&self, number: Number, now: u64) -> rusqlite::Result<Vec<Held>> {
        let db = self.db.lock().unwrap();
        expire(&db, now)?;
        let mut query = db.prepare(
            "SELECT id, fax FROM faxes
             WHERE recipient = ?1
             ORDER BY arrived, rowid",
        )?;
        query
            .query_map(params![number.to_string()], |row| {
                Ok(Held {
                    id: row.get(0)?,
                    fax: json::<Fax>(row.get(1)?)?,
                })
            })?
            .collect()
    }

    /// Deletes those of `ids` waiting for `number`, since they printed,
    /// and answers with each one's id and sender. In one transaction,
    /// so a round of faxes costs the disk one commit.
    pub fn deliver(
        &self,
        number: Number,
        ids: &[String],
    ) -> rusqlite::Result<Vec<(String, String)>> {
        let mut db = self.db.lock().unwrap();
        let deleting = db.transaction()?;
        let mut delivered = Vec::new();
        for id in ids {
            let sender: Option<String> = deleting
                .query_row(
                    "DELETE FROM faxes WHERE id = ?1 AND recipient = ?2 RETURNING sender",
                    params![id, number.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(sender) = sender {
                delivered.push((id.clone(), sender));
            }
        }
        deleting.commit()?;
        Ok(delivered)
    }

    /// Deletes the faxes past the hold at `now`, then counts the lines
    /// and every line's waiting faxes together, for the log at startup.
    pub fn expire_and_count(&self, now: u64) -> rusqlite::Result<(usize, usize)> {
        let db = self.db.lock().unwrap();
        expire(&db, now)?;
        db.query_row(
            "SELECT (SELECT count(*) FROM lines), (SELECT count(*) FROM faxes)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    }
}

fn line(db: &Connection, number: Number) -> rusqlite::Result<Option<LineRecord>> {
    db.query_row(
        "SELECT record FROM lines WHERE number = ?1",
        params![number.to_string()],
        |row| json(row.get(0)?),
    )
    .optional()
}

/// Deletes the faxes that have waited [`HOLD_SECONDS`] at `now`: nobody
/// polled for them within the hold.
fn expire(db: &Connection, now: u64) -> rusqlite::Result<usize> {
    db.execute(
        "DELETE FROM faxes WHERE arrived <= ?1",
        [now.saturating_sub(HOLD_SECONDS)],
    )
}

fn waiting(db: &Connection, number: Number) -> rusqlite::Result<Waiting> {
    db.query_row(
        "SELECT count(*), coalesce(sum(size), 0),
                (SELECT coalesce(sum(size), 0) FROM faxes)
         FROM faxes WHERE recipient = ?1",
        params![number.to_string()],
        |row| {
            Ok(Waiting {
                faxes: row.get(0)?,
                bytes: row.get(1)?,
                stored: row.get(2)?,
            })
        },
    )
}

fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("records and faxes serialise")
}

fn json<T: serde::de::DeserializeOwned>(text: String) -> rusqlite::Result<T> {
    serde_json::from_str(&text).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}
