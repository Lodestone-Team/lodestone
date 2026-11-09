//! A small read-only reader of SQLite database files, enough to read the Modrinth App's
//! `app.db` without an SQLite library (none is in the launcher's lockfile).
//!
//! It follows the documented file format, <https://www.sqlite.org/fileformat2.html>:
//! the 100-byte header, table b-trees (interior and leaf pages), records with their
//! serial types, overflow pages, and the write-ahead log (`<db>-wal`), whose committed
//! frames replace pages of the main file. The Modrinth App keeps its database in WAL
//! mode, so recent changes are often only in the log.
//!
//! It never writes, takes no locks and creates no `-shm` file. Only rowid tables are
//! read; `WITHOUT ROWID` tables and indexes are not needed.

use std::{collections::HashMap, fs, path::Path};

/// The largest database read, in bytes.
const MAX_BYTES: u64 = 256 * 1024 * 1024;
/// The deepest b-tree followed; real trees are a few levels deep.
const MAX_DEPTH: usize = 32;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SqliteError {
    #[error("{0}")]
    Io(String),
    #[error("not an SQLite database")]
    NotSqlite,
    #[error("the database is damaged: {0}")]
    Corrupt(String),
    #[error("{0}")]
    Unsupported(String),
}

fn corrupt(what: impl Into<String>) -> SqliteError {
    SqliteError::Corrupt(what.into())
}

/// A value in a row.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(value) => Some(*value),
            Value::Real(value) => Some(*value as i64),
            Value::Text(text) => text.trim().parse().ok(),
            _ => None,
        }
    }

    /// A JSON column: text JSON, or a JSONB blob (<https://sqlite.org/jsonb.html>).
    pub fn as_json(&self) -> Option<serde_json::Value> {
        match self {
            Value::Text(text) => serde_json::from_str(text).ok(),
            Value::Blob(bytes) => super::jsonb::decode(bytes).ok(),
            _ => None,
        }
    }
}

/// A table's column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    /// `INTEGER PRIMARY KEY`: the column is the rowid, stored as NULL in the record.
    pub rowid_alias: bool,
}

/// A table from the schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub root_page: u32,
    pub columns: Vec<Column>,
    pub without_rowid: bool,
}

/// A row: its values by column name.
pub type Row = HashMap<String, Value>;

pub struct Database {
    main: Vec<u8>,
    /// Pages from committed WAL frames, by page number.
    wal: HashMap<u32, Vec<u8>>,
    page_size: usize,
    usable_size: usize,
    /// The number of pages, after the WAL's last commit.
    page_count: u32,
}

impl Database {
    /// Reads `path` and its `-wal` file, if there is one.
    pub fn open(path: &Path) -> Result<Self, SqliteError> {
        let read = |path: &Path| -> Result<Vec<u8>, SqliteError> {
            let size = fs::metadata(path)
                .map_err(|err| SqliteError::Io(format!("{}: {err}", path.display())))?
                .len();
            if size > MAX_BYTES {
                return Err(SqliteError::Unsupported(format!(
                    "{} is larger than {} MB",
                    path.display(),
                    MAX_BYTES / 1024 / 1024
                )));
            }
            fs::read(path).map_err(|err| SqliteError::Io(format!("{}: {err}", path.display())))
        };
        let main = read(path)?;
        let mut wal_path = path.as_os_str().to_owned();
        wal_path.push("-wal");
        let wal_path = std::path::PathBuf::from(wal_path);
        let wal = if wal_path.is_file() {
            Some(read(&wal_path)?)
        } else {
            None
        };
        Self::from_bytes(main, wal)
    }

    pub fn from_bytes(main: Vec<u8>, wal: Option<Vec<u8>>) -> Result<Self, SqliteError> {
        let header_page_size = if main.is_empty() {
            None
        } else {
            if main.len() < 100 || &main[..16] != b"SQLite format 3\0" {
                return Err(SqliteError::NotSqlite);
            }
            Some(match u16::from_be_bytes([main[16], main[17]]) {
                1 => 65536,
                size => size as usize,
            })
        };
        let wal = match wal {
            Some(bytes) if bytes.len() >= 32 => read_wal(&bytes, header_page_size)?,
            _ => None,
        };
        let page_size = match (header_page_size, &wal) {
            (Some(size), _) => size,
            (None, Some(wal)) => wal.page_size,
            (None, None) => return Err(SqliteError::NotSqlite),
        };
        if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(corrupt(format!("page size {page_size}")));
        }
        let mut db = Self {
            page_count: (main.len() / page_size) as u32,
            main,
            wal: HashMap::new(),
            page_size,
            usable_size: page_size,
        };
        if let Some(wal) = wal {
            db.wal = wal.pages;
            db.page_count = wal.page_count;
        }
        let reserved = db.page(1)?[20] as usize;
        db.usable_size = page_size
            .checked_sub(reserved)
            .filter(|usable| *usable >= 480)
            .ok_or_else(|| corrupt("reserved space"))?;
        Ok(db)
    }

    fn page(&self, number: u32) -> Result<&[u8], SqliteError> {
        if number == 0 || number > self.page_count {
            return Err(corrupt(format!("page {number} is out of range")));
        }
        if let Some(page) = self.wal.get(&number) {
            return Ok(page);
        }
        let start = (number as usize - 1) * self.page_size;
        self.main
            .get(start..start + self.page_size)
            .ok_or_else(|| corrupt(format!("page {number} is missing")))
    }

    /// The tables in the schema.
    pub fn tables(&self) -> Result<Vec<Table>, SqliteError> {
        let mut tables = Vec::new();
        for (_, values) in self.rows_at(1)? {
            let text = |i: usize| match values.get(i) {
                Some(Value::Text(text)) => Some(text.clone()),
                _ => None,
            };
            if text(0).as_deref() != Some("table") {
                continue;
            }
            let (Some(name), Some(sql)) = (text(1), text(4)) else {
                continue;
            };
            let Some(root_page) = values.get(3).and_then(Value::as_i64) else {
                continue;
            };
            let (columns, without_rowid) = parse_create_table(&sql);
            tables.push(Table {
                name,
                root_page: u32::try_from(root_page).map_err(|_| corrupt("root page"))?,
                columns,
                without_rowid,
            });
        }
        Ok(tables)
    }

    pub fn table(&self, name: &str) -> Result<Option<Table>, SqliteError> {
        Ok(self
            .tables()?
            .into_iter()
            .find(|t| t.name.eq_ignore_ascii_case(name)))
    }

    /// Every row of the table `name`, or `None` if there is no such table.
    pub fn rows(&self, name: &str) -> Result<Option<Vec<Row>>, SqliteError> {
        let Some(table) = self.table(name)? else {
            return Ok(None);
        };
        if table.without_rowid {
            return Err(SqliteError::Unsupported(format!(
                "{name} is a WITHOUT ROWID table"
            )));
        }
        let rows = self
            .rows_at(table.root_page)?
            .into_iter()
            .map(|(rowid, values)| {
                table
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(i, column)| {
                        let value = if column.rowid_alias {
                            Value::Integer(rowid)
                        } else {
                            // Rows written before an ALTER TABLE ADD COLUMN are shorter.
                            values.get(i).cloned().unwrap_or(Value::Null)
                        };
                        (column.name.clone(), value)
                    })
                    .collect()
            })
            .collect();
        Ok(Some(rows))
    }

    /// The (rowid, values) of the table b-tree rooted at `root`.
    fn rows_at(&self, root: u32) -> Result<Vec<(i64, Vec<Value>)>, SqliteError> {
        let mut rows = Vec::new();
        let mut visited = std::collections::HashSet::new();
        self.walk(root, 0, &mut visited, &mut rows)?;
        Ok(rows)
    }

    fn walk(
        &self,
        number: u32,
        depth: usize,
        visited: &mut std::collections::HashSet<u32>,
        rows: &mut Vec<(i64, Vec<Value>)>,
    ) -> Result<(), SqliteError> {
        if depth > MAX_DEPTH || !visited.insert(number) {
            return Err(corrupt("the b-tree loops"));
        }
        let page = self.page(number)?;
        let offset = if number == 1 { 100 } else { 0 };
        let header = page
            .get(offset..offset + 8)
            .ok_or_else(|| corrupt("page header"))?;
        let kind = header[0];
        let cells = u16::from_be_bytes([header[3], header[4]]) as usize;
        let header_len = match kind {
            0x0d => 8,
            0x05 => 12,
            _ => {
                return Err(corrupt(format!(
                    "page {number} is not a table page ({kind})"
                )));
            }
        };
        let pointer = |i: usize| -> Result<usize, SqliteError> {
            let at = offset + header_len + i * 2;
            let bytes = page
                .get(at..at + 2)
                .ok_or_else(|| corrupt("cell pointer"))?;
            Ok(u16::from_be_bytes([bytes[0], bytes[1]]) as usize)
        };
        if kind == 0x05 {
            for i in 0..cells {
                let at = pointer(i)?;
                let child = page.get(at..at + 4).ok_or_else(|| corrupt("child page"))?;
                let child = u32::from_be_bytes([child[0], child[1], child[2], child[3]]);
                self.walk(child, depth + 1, visited, rows)?;
            }
            let right = &page[offset + 8..offset + 12];
            let right = u32::from_be_bytes([right[0], right[1], right[2], right[3]]);
            return self.walk(right, depth + 1, visited, rows);
        }
        for i in 0..cells {
            let at = pointer(i)?;
            let cell = page.get(at..).ok_or_else(|| corrupt("cell"))?;
            let (payload_len, a) = varint(cell)?;
            let (rowid, b) = varint(&cell[a..])?;
            let payload = self.payload(&cell[a + b..], payload_len)?;
            rows.push((rowid as i64, record(&payload)?));
        }
        Ok(())
    }

    /// A cell's payload of `len` bytes, starting at `local`, with its overflow pages.
    fn payload(&self, local: &[u8], len: u64) -> Result<Vec<u8>, SqliteError> {
        let len = usize::try_from(len).map_err(|_| corrupt("payload size"))?;
        if len as u64 > MAX_BYTES {
            return Err(corrupt("payload size"));
        }
        let usable = self.usable_size;
        let max_local = usable - 35;
        if len <= max_local {
            return local
                .get(..len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| corrupt("payload"));
        }
        let min_local = (usable - 12) * 32 / 255 - 23;
        let k = min_local + (len - min_local) % (usable - 4);
        let stored = if k <= max_local { k } else { min_local };
        let mut payload = local
            .get(..stored)
            .ok_or_else(|| corrupt("payload"))?
            .to_vec();
        let next = local
            .get(stored..stored + 4)
            .ok_or_else(|| corrupt("overflow pointer"))?;
        let mut next = u32::from_be_bytes([next[0], next[1], next[2], next[3]]);
        let mut hops = 0;
        while payload.len() < len {
            hops += 1;
            if next == 0 || hops > self.page_count {
                return Err(corrupt("overflow chain"));
            }
            let page = self.page(next)?;
            let want = (len - payload.len()).min(usable - 4);
            payload.extend_from_slice(&page[4..4 + want]);
            next = u32::from_be_bytes([page[0], page[1], page[2], page[3]]);
        }
        Ok(payload)
    }
}

/// A WAL's committed pages.
struct Wal {
    page_size: usize,
    pages: HashMap<u32, Vec<u8>>,
    page_count: u32,
}

/// The pages of the committed transactions in a WAL file: frames whose salts match the
/// header and whose cumulative checksums are right, up to the last commit frame.
fn read_wal(bytes: &[u8], main_page_size: Option<usize>) -> Result<Option<Wal>, SqliteError> {
    let u32_at =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let magic = u32_at(0);
    let big_endian = match magic {
        0x377f_0682 => false,
        0x377f_0683 => true,
        _ => return Ok(None),
    };
    let page_size = match u32_at(8) {
        1 => 65536,
        size => size as usize,
    };
    if main_page_size.is_some_and(|size| size != page_size)
        || !(512..=65536).contains(&page_size)
        || !page_size.is_power_of_two()
    {
        return Ok(None);
    }
    let (salt1, salt2) = (u32_at(16), u32_at(20));
    let mut sum = checksum(&bytes[..24], (0, 0), big_endian);
    if sum != (u32_at(24), u32_at(28)) {
        return Ok(None);
    }
    let mut pending: Vec<(u32, &[u8])> = Vec::new();
    let mut pages = HashMap::new();
    let mut page_count = None;
    let mut at = 32;
    while at + 24 + page_size <= bytes.len() {
        let frame = &bytes[at..at + 24];
        let page = &bytes[at + 24..at + 24 + page_size];
        let number = u32_at(at);
        let commit = u32_at(at + 4);
        if (u32_at(at + 8), u32_at(at + 12)) != (salt1, salt2) {
            break;
        }
        sum = checksum(&frame[..8], sum, big_endian);
        sum = checksum(page, sum, big_endian);
        if sum != (u32_at(at + 16), u32_at(at + 20)) {
            break;
        }
        pending.push((number, page));
        if commit != 0 {
            for (number, page) in pending.drain(..) {
                pages.insert(number, page.to_vec());
            }
            page_count = Some(commit);
        }
        at += 24 + page_size;
    }
    Ok(page_count.map(|page_count| Wal {
        page_size,
        pages,
        page_count,
    }))
}

/// SQLite's WAL checksum over `data` (a multiple of 8 bytes), continuing from `start`.
fn checksum(data: &[u8], start: (u32, u32), big_endian: bool) -> (u32, u32) {
    let (mut s0, mut s1) = start;
    for chunk in data.as_chunks::<8>().0 {
        let word = |bytes: &[u8]| {
            let bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
            if big_endian {
                u32::from_be_bytes(bytes)
            } else {
                u32::from_le_bytes(bytes)
            }
        };
        s0 = s0.wrapping_add(word(&chunk[..4])).wrapping_add(s1);
        s1 = s1.wrapping_add(word(&chunk[4..])).wrapping_add(s0);
    }
    (s0, s1)
}

/// A varint and its length in bytes.
fn varint(bytes: &[u8]) -> Result<(u64, usize), SqliteError> {
    let mut value = 0u64;
    for i in 0..9 {
        let byte = *bytes.get(i).ok_or_else(|| corrupt("varint"))?;
        if i == 8 {
            return Ok(((value << 8) | byte as u64, 9));
        }
        value = (value << 7) | (byte & 0x7f) as u64;
        if byte & 0x80 == 0 {
            return Ok((value, i + 1));
        }
    }
    unreachable!()
}

/// The values of a record.
fn record(payload: &[u8]) -> Result<Vec<Value>, SqliteError> {
    let (header_len, mut at) = varint(payload)?;
    let header_len = header_len as usize;
    if header_len > payload.len() {
        return Err(corrupt("record header"));
    }
    let mut types = Vec::new();
    while at < header_len {
        let (serial, n) = varint(&payload[at..header_len])?;
        types.push(serial);
        at += n;
    }
    let mut body = header_len;
    let mut values = Vec::with_capacity(types.len());
    for serial in types {
        let take = |len: usize, body: &mut usize| -> Result<&[u8], SqliteError> {
            let bytes = payload
                .get(*body..*body + len)
                .ok_or_else(|| corrupt("record body"))?;
            *body += len;
            Ok(bytes)
        };
        let int = |bytes: &[u8]| {
            let mut value: i64 = if bytes[0] & 0x80 != 0 { -1 } else { 0 };
            for byte in bytes {
                value = (value << 8) | *byte as i64;
            }
            value
        };
        values.push(match serial {
            0 => Value::Null,
            1..=4 => Value::Integer(int(take(serial as usize, &mut body)?)),
            5 => Value::Integer(int(take(6, &mut body)?)),
            6 => Value::Integer(int(take(8, &mut body)?)),
            7 => {
                let bytes = take(8, &mut body)?;
                Value::Real(f64::from_be_bytes(bytes.try_into().expect("8 bytes")))
            }
            8 => Value::Integer(0),
            9 => Value::Integer(1),
            10 | 11 => return Err(corrupt("reserved serial type")),
            n if n % 2 == 0 => Value::Blob(take(((n - 12) / 2) as usize, &mut body)?.to_vec()),
            n => Value::Text(
                String::from_utf8_lossy(take(((n - 13) / 2) as usize, &mut body)?).into_owned(),
            ),
        });
    }
    Ok(values)
}

/// The columns of a `CREATE TABLE` statement, and whether it is `WITHOUT ROWID`.
pub fn parse_create_table(sql: &str) -> (Vec<Column>, bool) {
    let Some(open) = sql.find('(') else {
        return (Vec::new(), false);
    };
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut current = String::new();
    let mut close = None;
    for (i, c) in sql[open + 1..].char_indices() {
        if let Some(q) = quote {
            current.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                quote = Some(c);
                current.push(c);
            }
            '[' => {
                quote = Some(']');
                current.push(c);
            }
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' if depth == 0 => {
                close = Some(open + 1 + i);
                break;
            }
            ')' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    let without_rowid = close.is_some_and(|close| {
        sql[close + 1..]
            .to_ascii_uppercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|w| w == ["WITHOUT", "ROWID"])
    });
    let columns = parts
        .iter()
        .map(|part| strip_comments(part))
        .filter_map(|part| {
            let part = part.trim();
            let upper = part.to_ascii_uppercase();
            let first = upper
                .split(|c: char| c.is_whitespace() || c == '(')
                .next()?;
            if first.is_empty()
                || matches!(
                    first,
                    "CONSTRAINT" | "PRIMARY" | "UNIQUE" | "CHECK" | "FOREIGN"
                )
            {
                return None;
            }
            let (name, rest) = split_name(part);
            let rest_upper = rest.to_ascii_uppercase();
            let mut words = rest_upper.split_whitespace();
            let rowid_alias = words.next() == Some("INTEGER")
                && rest_upper
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .windows(2)
                    .any(|w| w == ["PRIMARY", "KEY"]);
            Some(Column { name, rowid_alias })
        })
        .collect();
    (columns, without_rowid)
}

fn strip_comments(part: &str) -> String {
    part.lines()
        .map(|line| match line.find("--") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A column definition's name (unquoted) and the rest.
fn split_name(part: &str) -> (String, &str) {
    let close = match part.chars().next() {
        Some('"') => Some('"'),
        Some('`') => Some('`'),
        Some('[') => Some(']'),
        Some('\'') => Some('\''),
        _ => None,
    };
    match close {
        Some(close) => match part[1..].find(close) {
            Some(end) => (part[1..1 + end].to_string(), &part[2 + end..]),
            None => (part.to_string(), ""),
        },
        None => {
            let end = part.find(char::is_whitespace).unwrap_or(part.len());
            (part[..end].to_string(), &part[end..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_and_records() {
        assert_eq!(varint(&[0x05]).unwrap(), (5, 1));
        assert_eq!(varint(&[0x81, 0x00]).unwrap(), (128, 2));
        assert_eq!(varint(&[0xff; 9]).unwrap(), (u64::MAX, 9));
        // header: len 4, NULL, int8, text "hi"
        let payload = [4, 0, 1, 17, 0xfe, b'h', b'i'];
        assert_eq!(
            record(&payload).unwrap(),
            [Value::Null, Value::Integer(-2), Value::Text("hi".into())]
        );
        assert!(record(&[9, 0]).is_err());
    }

    #[test]
    fn create_table_columns() {
        let (columns, without_rowid) = parse_create_table(
            "CREATE TABLE \"profiles\" (\n  path TEXT NOT NULL PRIMARY KEY, -- the folder\n  name TEXT NOT NULL,\n  [odd name] TEXT DEFAULT 'a,b',\n  groups JSONB NOT NULL DEFAULT (jsonb_array()),\n  PRIMARY KEY (path),\n  UNIQUE (name)\n, override_mc_memory_max INTEGER NULL)",
        );
        let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "path",
                "name",
                "odd name",
                "groups",
                "override_mc_memory_max"
            ]
        );
        assert!(!without_rowid);
        assert!(columns.iter().all(|c| !c.rowid_alias));
        let (columns, without_rowid) =
            parse_create_table("CREATE TABLE t (id INTEGER PRIMARY KEY, v) WITHOUT ROWID");
        assert!(columns[0].rowid_alias);
        assert!(without_rowid);
    }

    #[test]
    fn rejects_other_files() {
        assert!(matches!(
            Database::from_bytes(b"not a database at all".repeat(10), None),
            Err(SqliteError::NotSqlite)
        ));
        assert!(Database::from_bytes(Vec::new(), None).is_err());
    }
}
