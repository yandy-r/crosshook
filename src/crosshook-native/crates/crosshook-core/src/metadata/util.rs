/// Build a comma-joined list of `?` placeholders for SQL `IN (...)` clauses.
pub(super) fn in_clause_placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Atomic standalone and nested writes for callers borrowing `&Connection`.
pub(super) struct WriteSavepoint<'a> {
    conn: &'a rusqlite::Connection,
    name: String,
    finished: bool,
}

pub(super) fn write_savepoint(conn: &rusqlite::Connection) -> rusqlite::Result<WriteSavepoint<'_>> {
    let name = format!("crosshook_{}", uuid::Uuid::new_v4().simple());
    conn.execute_batch(&format!("SAVEPOINT {name}"))?;
    Ok(WriteSavepoint {
        conn,
        name,
        finished: false,
    })
}

impl std::ops::Deref for WriteSavepoint<'_> {
    type Target = rusqlite::Connection;
    fn deref(&self) -> &Self::Target {
        self.conn
    }
}

impl WriteSavepoint<'_> {
    pub(super) fn commit(mut self) -> rusqlite::Result<()> {
        self.conn.execute_batch(&format!("RELEASE {}", self.name))?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for WriteSavepoint<'_> {
    fn drop(&mut self) {
        if !self.finished {
            if let Err(error) = self
                .conn
                .execute_batch(&format!("ROLLBACK TO {}; RELEASE {}", self.name, self.name))
            {
                tracing::error!(%error, "Failed to roll back metadata savepoint");
            }
        }
    }
}

/// Immediate write transaction that permits mutable connection access to nested savepoints.
pub(super) struct WriteTransaction<'a> {
    conn: &'a mut rusqlite::Connection,
    finished: bool,
}

impl<'a> WriteTransaction<'a> {
    pub(super) fn begin(conn: &'a mut rusqlite::Connection) -> rusqlite::Result<Self> {
        conn.execute_batch("BEGIN IMMEDIATE")?;
        Ok(Self {
            conn,
            finished: false,
        })
    }

    pub(super) fn commit(mut self) -> rusqlite::Result<()> {
        self.conn.execute_batch("COMMIT")?;
        self.finished = true;
        Ok(())
    }
}

impl std::ops::Deref for WriteTransaction<'_> {
    type Target = rusqlite::Connection;
    fn deref(&self) -> &Self::Target {
        self.conn
    }
}

impl std::ops::DerefMut for WriteTransaction<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn
    }
}

impl Drop for WriteTransaction<'_> {
    fn drop(&mut self) {
        if !self.finished && !self.conn.is_autocommit() {
            if let Err(error) = self.conn.execute_batch("ROLLBACK") {
                tracing::error!(%error, "Failed to roll back metadata transaction");
            }
        }
    }
}
