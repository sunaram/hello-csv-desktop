//! Small CSV engine: record parsing, delimiter sniffing, sparse row index, paged reads, search and delta save.
//! Everything works on raw bytes so multi-gigabyte files are never held in memory.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Files larger than this are opened in streaming mode (indexed, paged from disk).
pub const STREAM_OVER: u64 = 5 * 1024 * 1024;
/// One row offset is stored per STRIDE rows.
const STRIDE: u64 = 32;
/// Bytes read per `scan` step when rewriting a file; searches use the smaller SEARCH_WINDOW so each request stays quick.
const WINDOW: usize = 16 << 20;
const SEARCH_WINDOW: usize = 2 << 20;

/// Index of the byte offset of every STRIDE-th non-blank record (the header counts as record 0).
pub struct Index {
    offs: Mutex<Vec<u64>>,
    pub count: AtomicU64,
    pub done: AtomicBool,
    pub epoch: u64,
}

impl Index {
    pub fn new(epoch: u64) -> Arc<Index> {
        Arc::new(Index { offs: Mutex::new(vec![]), count: AtomicU64::new(0), done: AtomicBool::new(false), epoch })
    }
}

/// A line made only of blanks is skipped everywhere (a tab only counts as blank when it is not the delimiter).
fn rec_blank(rec: &[u8], d: u8) -> bool {
    rec.iter().all(|&b| b == b'\r' || b == b'\n' || b == b' ' || (b == b'\t' && d != b'\t'))
}

/// End of the record starting at `i` (just past its newline) and whether a newline outside quotes was found.
fn record_end(b: &[u8], i: usize) -> (usize, bool) {
    let mut inq = false;
    for (k, &c) in b[i..].iter().enumerate() {
        if c == b'"' {
            inq = !inq;
        } else if c == b'\n' && !inq {
            return (i + k + 1, true);
        }
    }
    (b.len(), false)
}

/// Parse one record (RFC 4180 with lenient handling of stray quotes).
pub fn parse_record(b: &[u8], d: u8) -> Vec<String> {
    let mut fields = vec![];
    let mut i = 0;
    loop {
        let mut cur: Vec<u8> = vec![];
        if i < b.len() && b[i] == b'"' {
            i += 1;
            while i < b.len() {
                if b[i] == b'"' {
                    if b.get(i + 1) == Some(&b'"') {
                        cur.push(b'"');
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    cur.push(b[i]);
                    i += 1;
                }
            }
        }
        while i < b.len() && b[i] != d && b[i] != b'\n' {
            cur.push(b[i]);
            i += 1;
        }
        if i < b.len() && b[i] == d {
            fields.push(String::from_utf8_lossy(&cur).into_owned());
            i += 1;
            continue;
        }
        if cur.last() == Some(&b'\r') {
            cur.pop();
        }
        fields.push(String::from_utf8_lossy(&cur).into_owned());
        return fields;
    }
}

/// Parse up to `limit` non-blank records.
pub fn parse_all(b: &[u8], d: u8, limit: usize) -> Vec<Vec<String>> {
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() && out.len() < limit {
        let (e, _) = record_end(b, i);
        if !rec_blank(&b[i..e], d) {
            out.push(parse_record(&b[i..e], d));
        }
        i = e;
    }
    out
}

pub fn detect_delim(b: &[u8]) -> u8 {
    let sample = &b[..b.len().min(64 * 1024)];
    let mut best = (0usize, b',');
    for d in [b',', b';', b'\t', b'|'] {
        let recs = parse_all(sample, d, 20);
        let Some(first) = recs.first() else { continue };
        let c0 = first.len();
        if c0 < 2 {
            continue;
        }
        let score = recs.iter().filter(|r| r.len() == c0).count() * 100 + c0;
        if score > best.0 {
            best = (score, d);
        }
    }
    best.1
}

pub fn strip_bom(b: &[u8]) -> (&[u8], bool) {
    match b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(r) => (r, true),
        None => (b, false),
    }
}

/// Read up to `n` bytes from the start of the file, cut back to the last complete line when truncated.
pub fn read_head(path: &Path, n: usize) -> std::io::Result<Vec<u8>> {
    let mut buf = vec![];
    File::open(path)?.take(n as u64).read_to_end(&mut buf)?;
    if buf.len() == n {
        if let Some(p) = buf.iter().rposition(|&c| c == b'\n') {
            buf.truncate(p + 1);
        }
    }
    Ok(buf)
}

/// Background thread: record the offset of every STRIDE-th non-blank record and the total count.
pub fn spawn_index(path: PathBuf, delim: u8, idx: Arc<Index>, cur: Arc<AtomicU64>) {
    std::thread::spawn(move || {
        let Ok(mut f) = File::open(path) else { return };
        let mut buf = vec![0u8; 1 << 20];
        let (mut pos, mut n, mut rec_start) = (0u64, 0u64, 0u64);
        let (mut inq, mut content) = (false, false);
        loop {
            let Ok(r) = f.read(&mut buf) else { return };
            if r == 0 {
                break;
            }
            let mut local = vec![];
            for (k, &b) in buf[..r].iter().enumerate() {
                if !inq && b == b'\n' {
                    if content {
                        if n % STRIDE == 0 {
                            local.push(rec_start);
                        }
                        n += 1;
                    }
                    rec_start = pos + k as u64 + 1;
                    content = false;
                } else {
                    if b == b'"' {
                        inq = !inq;
                    }
                    if !(b == b'\r' || b == b' ' || (b == b'\t' && delim != b'\t')) {
                        content = true;
                    }
                }
            }
            pos += r as u64;
            idx.offs.lock().unwrap().extend(local);
            idx.count.store(n, Ordering::Relaxed);
            if cur.load(Ordering::Relaxed) != idx.epoch {
                return;
            }
        }
        if content {
            if n % STRIDE == 0 {
                idx.offs.lock().unwrap().push(rec_start);
            }
            n += 1;
        }
        idx.count.store(n, Ordering::Relaxed);
        idx.done.store(true, Ordering::Release);
    });
}

/// Rows `[page-1)*size, page*size)` of the data (the header is not counted).
pub fn page(path: &Path, idx: &Index, delim: u8, hdr: bool, ncols: usize, page: u64, size: u64) -> std::io::Result<Vec<Vec<String>>> {
    let start = (page.max(1) - 1) * size + hdr as u64;
    if start >= idx.count.load(Ordering::Relaxed) {
        return Ok(vec![]);
    }
    let si = (start / STRIDE) as usize;
    let (base, end) = {
        let o = idx.offs.lock().unwrap();
        let Some(&base) = o.get(si) else { return Ok(vec![]) };
        (base, o.get(((start + size) / STRIDE + 1) as usize).copied())
    };
    let mut f = File::open(path)?;
    let fsize = f.metadata()?.len();
    let end = end.unwrap_or(fsize).min(base + 32 * 1024 * 1024).min(fsize);
    f.seek(SeekFrom::Start(base))?;
    let mut buf = vec![];
    f.take(end - base).read_to_end(&mut buf)?;
    let skip = (start - si as u64 * STRIDE) as usize;
    let mut rows = parse_all(&buf, delim, skip + size as usize);
    rows.drain(..skip.min(rows.len()));
    for r in &mut rows {
        if r.len() < ncols {
            r.resize(ncols, String::new());
        }
    }
    Ok(rows)
}

/// Feed whole records starting at byte `off` (at most one WINDOW of the file) to `cb` (return false to stop).
/// Returns the offset to continue from and whether the end of the file was reached.
fn scan<F: FnMut(&[u8]) -> bool>(f: &mut File, off: u64, window: usize, mut cb: F) -> std::io::Result<(u64, bool)> {
    f.seek(SeekFrom::Start(off))?;
    let mut buf = vec![];
    f.take(window as u64).read_to_end(&mut buf)?;
    let eof = buf.len() < window;
    let mut i = 0;
    let mut stopped = false;
    while i < buf.len() {
        let (e, term) = record_end(&buf, i);
        if !term && !eof && i > 0 {
            break;
        }
        let s = i;
        i = e;
        if !cb(&buf[s..e]) {
            stopped = true;
            break;
        }
    }
    Ok((off + i as u64, eof && !stopped && i >= buf.len()))
}

fn contains_ci(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if hay.len() < needle.len() {
        return false;
    }
    hay.windows(needle.len()).any(|w| w.iter().zip(needle).all(|(a, b)| a.to_ascii_lowercase() == *b))
}

pub struct SearchOut {
    /// `(data row number, cells)`
    pub matches: Vec<(u64, Vec<String>)>,
    pub off: u64,
    pub row: u64,
    pub done: bool,
}

/// Scan from byte `off` (data row number `row`) until `max` matches or one window has been read.
pub fn search(path: &Path, delim: u8, hdr: bool, q: &str, off: u64, row: u64, max: usize) -> std::io::Result<SearchOut> {
    let mut f = File::open(path)?;
    let ql = q.trim().to_lowercase();
    let ascii = ql.is_ascii();
    let mut row = row;
    let mut skip_header = hdr && off == 0;
    let mut matches = vec![];
    let (off, done) = scan(&mut f, off, SEARCH_WINDOW, |rec| {
        if rec_blank(rec, delim) {
            return true;
        }
        if skip_header {
            skip_header = false;
            return true;
        }
        let r = row;
        row += 1;
        let hit = if ascii { contains_ci(rec, ql.as_bytes()) } else { String::from_utf8_lossy(rec).to_lowercase().contains(&ql) };
        if hit {
            let cells = parse_record(rec, delim);
            if cells.iter().any(|c| c.to_lowercase().contains(&ql)) {
                matches.push((r, cells));
            }
        }
        matches.len() < max
    })?;
    Ok(SearchOut { matches, off, row, done })
}

/// Rewrite the file, replacing only the edited records (and optionally the header) and copying everything
/// else byte for byte. Streams through a temp file so memory stays constant.
pub fn save_delta(path: &Path, delim: u8, hdr: bool, header: Option<&str>, mods: &HashMap<u64, String>, crlf: bool) -> std::io::Result<()> {
    let tmp = PathBuf::from(format!("{}.hcsv.tmp", path.display()));
    let result = (|| {
        let mut out = BufWriter::new(File::create(&tmp)?);
        let mut f = File::open(path)?;
        let (mut off, mut idx, mut header_pending) = (0u64, 0u64, hdr);
        let mut err: Option<std::io::Error> = None;
        loop {
            let first = off == 0;
            let mut first_rec = first;
            let (next, eof) = scan(&mut f, off, WINDOW, |rec| {
                let bom = first_rec && rec.starts_with(&[0xEF, 0xBB, 0xBF]);
                first_rec = false;
                let mut w = |bytes: &[u8]| {
                    if err.is_none() {
                        if let Err(e) = out.write_all(bytes) {
                            err = Some(e);
                        }
                    }
                };
                if rec_blank(rec, delim) {
                    w(rec);
                    return true;
                }
                // replacement text keeps the original line ending of that record (none for an unterminated last line)
                let term: &[u8] = if rec.ends_with(b"\r\n") { b"\r\n" } else if rec.ends_with(b"\n") { b"\n" } else { b"" };
                let repl = |text: &str, w: &mut dyn FnMut(&[u8])| {
                    if bom {
                        w(&[0xEF, 0xBB, 0xBF]);
                    }
                    let t = if crlf { text.replace('\n', "\r\n") } else { text.to_string() };
                    w(t.as_bytes());
                    w(term);
                };
                if header_pending {
                    header_pending = false;
                    match header {
                        Some(h) => repl(h, &mut w),
                        None => w(rec),
                    }
                    return true;
                }
                match mods.get(&idx) {
                    Some(m) => repl(m, &mut w),
                    None => w(rec),
                }
                idx += 1;
                true
            })?;
            if let Some(e) = err.take() {
                return Err(e);
            }
            if eof {
                break;
            }
            off = next;
        }
        out.flush()?;
        drop(out);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

pub fn json_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn json_row(r: &[String], out: &mut String) {
    out.push('[');
    for (i, c) in r.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        json_str(c, out);
    }
    out.push(']');
}

pub fn json_rows(rows: &[Vec<String>], out: &mut String) {
    out.push('[');
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        json_row(r, out);
    }
    out.push(']');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str, data: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("hcsv-test-{name}.csv"));
        std::fs::write(&p, data).unwrap();
        p
    }

    #[test]
    fn parses_quotes_and_blank_lines() {
        let r = parse_all(b"a,b\r\n\r\n\"x,\"\"y\"\"\nz\",2\r\n  \n3,4", b',', 99);
        assert_eq!(r, vec![vec!["a", "b"], vec!["x,\"y\"\nz", "2"], vec!["3", "4"]]);
        assert_eq!(detect_delim(b"a;b;c\n1;2;3\n"), b';');
        assert_eq!(detect_delim(b"a\tb\n1\t2\n"), b'\t');
    }

    #[test]
    fn index_page_search_delta() {
        let mut s = String::from("id,name\n");
        for i in 0..1000 {
            if i == 500 {
                s.push_str("500,\"multi\nline Needle\"\n\n");
            } else {
                s.push_str(&format!("{i},name{i}\r\n").replace('\r', ""));
            }
        }
        let p = tmp("a", &s);
        let idx = Index::new(1);
        let cur = Arc::new(AtomicU64::new(1));
        spawn_index(p.clone(), b',', idx.clone(), cur);
        while !idx.done.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(idx.count.load(Ordering::Relaxed), 1001);
        let pg = page(&p, &idx, b',', true, 2, 11, 50).unwrap(); // rows 500..549
        assert_eq!(pg[0], vec!["500", "multi\nline Needle"]);
        assert_eq!(pg[1], vec!["501", "name501"]);
        assert_eq!(pg.len(), 50);
        let last = page(&p, &idx, b',', true, 2, 20, 50).unwrap();
        assert_eq!(last.last().unwrap()[0], "999");
        let r = search(&p, b',', true, "needle", 0, 0, 200).unwrap();
        assert_eq!(r.matches.len(), 1);
        assert_eq!(r.matches[0].0, 500);
        assert!(r.done);
        let mut mods = HashMap::new();
        mods.insert(2u64, "2,\"edited, value\"".to_string());
        save_delta(&p, b',', true, Some("ID,NAME"), &mods, false).unwrap();
        let out = std::fs::read_to_string(&p).unwrap();
        assert!(out.starts_with("ID,NAME\n0,name0\n1,name1\n2,\"edited, value\"\n3,name3\n"));
        assert!(out.contains("500,\"multi\nline Needle\"\n\n"));
        assert!(out.ends_with("999,name999\n"));
    }
}
