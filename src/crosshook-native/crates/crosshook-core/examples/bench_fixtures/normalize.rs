//! Post-generation SQLite normalisation so the DB file is byte-identical for a given seed
//! and independent of the `--out` path.
//!
//! Core APIs stamp UUID v4 ids, `Utc::now()`/`datetime('now')` timestamps, `randomblob` ids
//! and absolute paths. We rewrite every row generically:
//! - timestamp-looking TEXT -> deterministic seeded-epoch value in the same format
//! - UUID / 32-hex ids (TEXT or BLOB) -> one global old->new map in first-encounter order
//! - real `--out` prefix -> `@CROSSHOOK_BENCH_ROOT@`
//!
//! Then checkpoints WAL, switches to `journal_mode=DELETE` and `VACUUM`s.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::types::{Value, ValueRef};
use rusqlite::Connection;

use crate::layout::{Layout, TOKEN};
use crate::rng::Rng;

/// 2024-01-01T00:00:00Z
const BASE_EPOCH: i64 = 1_704_067_200;
const TS_ROW_STRIDE: i64 = 128;
/// Generator-chosen fixed timestamp (see generate.rs); never rewritten.
pub const EPOCH_ANCHOR: &str = "2024-01-01T00:00:00+00:00";
/// Generator-chosen expiry year prefix; must stay in the future so no refresh is triggered.
pub const FAR_FUTURE_PREFIX: &str = "2099-";

#[derive(PartialEq, Eq, Clone, Copy)]
enum TsFormat {
    Rfc3339,
    Sqlite,
}

fn detect_ts(s: &str) -> Option<TsFormat> {
    let b = s.as_bytes();
    let digits =
        |r: std::ops::Range<usize>| b.get(r).is_some_and(|x| x.iter().all(u8::is_ascii_digit));
    if !(digits(0..4)
        && b.get(4) == Some(&b'-')
        && digits(5..7)
        && b.get(7) == Some(&b'-')
        && digits(8..10))
    {
        return None;
    }
    match b.get(10) {
        Some(b'T') if chrono::DateTime::parse_from_rfc3339(s).is_ok() => Some(TsFormat::Rfc3339),
        Some(b' ') if chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").is_ok() => {
            Some(TsFormat::Sqlite)
        }
        _ => None,
    }
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit() && !c.is_ascii_uppercase(),
        })
}

fn is_hex32(s: &str) -> bool {
    s.len() == 32
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

struct Ctx<'a> {
    real_prefix: String,
    ids: HashMap<String, String>,
    id_rng: Rng,
    row_seq: i64,
    col_idx: i64,
    _layout: &'a Layout,
}

impl Ctx<'_> {
    fn map_id(&mut self, old: &str, uuid_style: bool) -> String {
        if let Some(v) = self.ids.get(old) {
            return v.clone();
        }
        let new = if uuid_style {
            self.id_rng.uuid()
        } else {
            format!(
                "{:016x}{:016x}",
                self.id_rng.next_u64(),
                self.id_rng.next_u64()
            )
        };
        self.ids.insert(old.to_string(), new.clone());
        new
    }

    fn map_ts(&mut self, fmt: TsFormat) -> String {
        // Position-based, never value-based: original wall-clock values collide differently
        // from run to run. Row sequence * stride + column index keeps column order within a
        // row (e.g. started_at < finished_at) and is identical for a given seed and schema.
        let secs = BASE_EPOCH + self.row_seq * TS_ROW_STRIDE + self.col_idx;
        let dt = chrono::DateTime::from_timestamp(secs, 0).expect("valid epoch");
        match fmt {
            TsFormat::Rfc3339 => dt.to_rfc3339(),
            TsFormat::Sqlite => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }

    fn map_text(&mut self, s: &str) -> String {
        if is_uuid(s) {
            return self.map_id(s, true);
        }
        if is_hex32(s) {
            return self.map_id(s, false);
        }
        if let Some(fmt) = detect_ts(s) {
            // Generator-set anchors (far-future expiry, fixed epoch) are already deterministic
            // and semantically load-bearing (expiry must stay in the future): keep verbatim.
            if s.starts_with(FAR_FUTURE_PREFIX) || s == EPOCH_ANCHOR {
                return s.to_string();
            }
            return self.map_ts(fmt);
        }
        // Embedded UUIDs inside larger strings (e.g. JSON payloads) are left alone: core APIs
        // only mint them as whole-column ids.
        if !s.contains(&self.real_prefix) {
            return s.to_string();
        }
        s.replace(&self.real_prefix, TOKEN)
    }
}

struct TableInfo {
    name: String,
    cols: Vec<String>,
}

fn tables(conn: &Connection) -> Result<Vec<TableInfo>, String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .map_err(|e| e.to_string())?;
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for name in names {
        let mut info = conn
            .prepare(&format!("PRAGMA table_info(\"{name}\")"))
            .map_err(|e| e.to_string())?;
        let cols = info
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        out.push(TableInfo { name, cols });
    }
    Ok(out)
}

pub fn normalize_db(db: &Path, layout: &Layout, seed: u64) -> Result<(), String> {
    let conn = Connection::open(db).map_err(|e| format!("open {}: {e}", db.display()))?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")
        .map_err(|e| e.to_string())?;

    let mut ctx = Ctx {
        real_prefix: layout.out.to_string_lossy().into_owned(),
        ids: HashMap::new(),
        id_rng: Rng::new(seed ^ 0x1D1D_1D1D_1D1D_1D1D),
        row_seq: 0,
        col_idx: 0,
        _layout: layout,
    };

    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| e.to_string())?;
    for table in tables(&conn)? {
        normalize_table(&conn, &table, &mut ctx)
            .map_err(|e| format!("normalize {}: {e}", table.name))?;
    }
    // AUTOINCREMENT counters are rowid-derived and already deterministic; just rewrite
    // nothing there. Commit.
    conn.execute_batch("COMMIT;").map_err(|e| e.to_string())?;

    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; VACUUM;")
        .map_err(|e| format!("finalize db: {e}"))?;
    drop(conn);
    for suffix in ["-wal", "-shm"] {
        let p = db.with_file_name(format!(
            "{}{suffix}",
            db.file_name().expect("db name").to_string_lossy()
        ));
        if p.exists() {
            std::fs::remove_file(&p).map_err(|e| format!("remove {}: {e}", p.display()))?;
        }
    }
    Ok(())
}

fn normalize_table(
    conn: &Connection,
    table: &TableInfo,
    ctx: &mut Ctx<'_>,
) -> rusqlite::Result<()> {
    let select = format!(
        "SELECT rowid, {} FROM \"{}\" ORDER BY rowid",
        quoted(&table.cols),
        table.name
    );
    let mut stmt = conn.prepare(&select)?;
    let mut rows = stmt.query([])?;
    let mut updates: Vec<(i64, Vec<(usize, Value)>)> = Vec::new();
    while let Some(row) = rows.next()? {
        let rowid: i64 = row.get(0)?;
        ctx.row_seq += 1;
        let mut changed = Vec::new();
        for i in 0..table.cols.len() {
            ctx.col_idx = i64::try_from(i).expect("SQLite column index fits i64");
            let new = match row.get_ref(i + 1)? {
                ValueRef::Text(t) => {
                    let s = String::from_utf8_lossy(t);
                    let n = ctx.map_text(&s);
                    (n != s).then_some(Value::Text(n))
                }
                ValueRef::Blob(b) => {
                    // 16-byte randomblob ids: map through same table keyed by hex.
                    (b.len() == 16 || b.len() == 32).then(|| {
                        let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
                        let mapped = ctx.map_id(&hex, false);
                        Value::Blob(decode_hex(&mapped, b.len()))
                    })
                }
                _ => None,
            };
            if let Some(v) = new {
                changed.push((i, v));
            }
        }
        if !changed.is_empty() {
            updates.push((rowid, changed));
        }
    }
    drop(rows);
    drop(stmt);

    for (rowid, changed) in updates {
        let sets: Vec<String> = changed
            .iter()
            .map(|(i, _)| format!("\"{}\" = ?", table.cols[*i]))
            .collect();
        let sql = format!(
            "UPDATE \"{}\" SET {} WHERE rowid = ?",
            table.name,
            sets.join(", ")
        );
        let mut params: Vec<Value> = changed.into_iter().map(|(_, v)| v).collect();
        params.push(Value::Integer(rowid));
        conn.execute(&sql, rusqlite::params_from_iter(params))?;
    }
    Ok(())
}

fn quoted(cols: &[String]) -> String {
    cols.iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn decode_hex(hex: &str, len: usize) -> Vec<u8> {
    // Expand/trim mapped 32-hex to requested blob length deterministically.
    let mut bytes: Vec<u8> = (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap_or(0))
        .collect();
    bytes.resize(len, 0);
    bytes
}
