//! `materialize --from <canonical> --root <dir>`: copy a canonical tree and swap the
//! `@CROSSHOOK_BENCH_ROOT@` token for the real `<root>`, inside SQLite rows and text files
//! (profile TOMLs, manifests, taps). Binary content (JPEGs, manifest DB file structure aside)
//! is copied untouched. SQLite rewrite goes through rusqlite so journal/WAL state stays valid.

use std::path::{Path, PathBuf};

use crate::guard;
use crate::layout::TOKEN;

pub fn materialize(canonical: &Path, root: &Path) -> Result<(), String> {
    let canonical = if canonical.is_absolute() {
        canonical.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("cwd: {e}"))?
            .join(canonical)
    };
    if !canonical.is_dir() {
        return Err(format!("--from {} is not a directory", canonical.display()));
    }
    let root = guard::check_output_dir_real_env(root).map_err(|e| e.to_string())?;
    if root.exists()
        && std::fs::read_dir(&root)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        return Err(format!(
            "materialize root {} exists and is not empty; refusing to mix trees",
            root.display()
        ));
    }
    copy_tree(&canonical, &root)?;
    rewrite_token(&root, &root.to_string_lossy())?;
    eprintln!(
        "bench_fixtures: materialized {} -> {}",
        canonical.display(),
        root.display()
    );
    Ok(())
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(src)
        .map_err(|e| format!("read_dir {}: {e}", src.display()))?
        .map(|e| e.map_err(|e| e.to_string()).map(|x| x.path()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for path in entries {
        let name = path.file_name().expect("file name");
        let out = dst.join(name);
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("stat {}: {e}", path.display()))?;
        if meta.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
            copy_tree(&path, &out)?;
        } else if meta.is_file() {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
            }
            std::fs::copy(&path, &out).map_err(|e| format!("copy {}: {e}", path.display()))?;
        } else {
            return Err(format!("refusing to copy symlink {}", path.display()));
        }
    }
    Ok(())
}

/// Is this plausibly a text file the token could appear in?
fn is_text(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("toml" | "json" | "log" | "txt" | "md" | "conf")
    )
}

fn all_files(root: &Path, acc: &mut Vec<PathBuf>) -> Result<(), String> {
    for e in std::fs::read_dir(root).map_err(|e| format!("read_dir {}: {e}", root.display()))? {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.is_dir() {
            all_files(&p, acc)?;
        } else {
            acc.push(p);
        }
    }
    Ok(())
}

fn rewrite_token(root: &Path, real: &str) -> Result<(), String> {
    let mut files = Vec::new();
    all_files(root, &mut files)?;
    let real = real.replace('\\', "/");
    for file in files {
        if file.file_name().is_some_and(|n| n == "metadata.db") {
            rewrite_db(&file, &real)?;
        } else if is_text(&file) {
            let bytes =
                std::fs::read(&file).map_err(|e| format!("read {}: {e}", file.display()))?;
            if !bytes.contains(&b'@') {
                continue;
            }
            let s = String::from_utf8(bytes).map_err(|_| format!("non-UTF8 {}", file.display()))?;
            if s.contains(TOKEN) {
                std::fs::write(&file, s.replace(TOKEN, &real))
                    .map_err(|e| format!("rewrite {}: {e}", file.display()))?;
            }
        }
    }
    Ok(())
}

fn rewrite_db(db: &Path, real: &str) -> Result<(), String> {
    // Open read/write through rusqlite; replace TOKEN in every TEXT cell.
    let conn = rusqlite::Connection::open(db).map_err(|e| format!("open {}: {e}", db.display()))?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")
        .map_err(|e| e.to_string())?;
    let tables: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .map_err(|e| e.to_string())?;
        let names = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        names
    };
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|e| e.to_string())?;
    for table in &tables {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .map_err(|e| e.to_string())?;
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        let quoted = cols
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let mut stmt = conn
            .prepare(&format!(
                "SELECT rowid, {quoted} FROM \"{table}\" ORDER BY rowid"
            ))
            .map_err(|e| e.to_string())?;
        let mut hits: Vec<(i64, usize, String)> = Vec::new();
        let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let rowid: i64 = row.get(0).map_err(|e| e.to_string())?;
            for (i, col) in cols.iter().enumerate() {
                let val: rusqlite::types::Value = row.get(i + 1).map_err(|e| e.to_string())?;
                if let rusqlite::types::Value::Text(s) = val {
                    if s.contains(TOKEN) {
                        hits.push((rowid, i, s.replace(TOKEN, real)));
                        let _ = col;
                    }
                }
            }
        }
        drop(rows);
        drop(stmt);
        for (rowid, col_i, new) in hits {
            conn.execute(
                &format!(
                    "UPDATE \"{}\" SET \"{}\" = ?1 WHERE rowid = ?2",
                    table, cols[col_i]
                ),
                rusqlite::params![new, rowid],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    conn.execute_batch(
        "COMMIT; PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE; VACUUM;",
    )
    .map_err(|e| format!("finalize: {e}"))?;
    Ok(())
}
