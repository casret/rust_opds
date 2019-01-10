use super::ComicInfo;
use chrono::prelude::*;
use failure::Error;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Row;
use std::path::Path;
use walkdir::DirEntry;

#[derive(Clone)]
pub struct DB {
    pool: Pool<SqliteConnectionManager>,
}

impl DB {
    pub fn new(db: &Path) -> Result<DB, Error> {
        let manager = SqliteConnectionManager::file(db);
        let pool = ::r2d2::Pool::new(manager)?;
        let conn = pool.get()?;
        conn.execute(
            "
          CREATE TABLE IF NOT EXISTS issue (
            filepath TEXT PRIMARY KEY,
            modified_at TEXT NOT NULL,
            size INTEGER NOT NULL,
            comicvine_id INTEGER,
            comicvine_url TEXT,
            series TEXT,
            issue_number INTEGER,
            volume INTEGER,
            title TEXT,
            summary TEXT,
            released_at TEXT,
            writer TEXT,
            penciller TEXT,
            inker TEXT,
            colorist TEXT,
            cover_artist TEXT,
            publisher TEXT,
            page_count INTEGER,
            cover_page TEXT
          )",
            &[],
        )?;

        conn.execute(
            "
          CREATE TABLE IF NOT EXISTS user (
            username TEXT PRIMARY KEY,
            salt blob,
            ciphertext blob
          )",
            &[],
        )?;

        conn.execute(
            "
          CREATE TABLE IF NOT EXISTS read (
            user_id INTEGER NOT NULL,
            issue_id INTEGER NOT NULL,
            read_at TEXT NOT NULL
          )",
            &[],
        )?;

        conn.execute(
            "
          CREATE TABLE IF NOT EXISTS page (
            issue_id INTEGER NOT NULL,
            entry TEXT NOT NULL
          )",
            &[],
        )?;

        conn.execute(
            "
          CREATE UNIQUE INDEX IF NOT EXISTS read_user_issue on read(user_id, issue_id)
          ",
            &[],
        )?;

        conn.execute(
            "
          CREATE UNIQUE INDEX IF NOT EXISTS page_issue on page(issue_id, entry)
          ",
            &[],
        )?;

        conn.execute(
            "
          CREATE INDEX IF NOT EXISTS issue_modified_at on issue(modified_at)
          ",
            &[],
        )?;

        conn.execute(
            "
          CREATE INDEX IF NOT EXISTS issue_publisher_series on issue(publisher, series)
          ",
            &[],
        )?;

        conn.execute(
            "
          CREATE INDEX IF NOT EXISTS issue_released_at on issue(released_at)
          ",
            &[],
        )?;

        conn.execute(
            "
          CREATE VIRTUAL TABLE IF NOT EXISTS issue_fts USING FTS4(issue_id, comicinfo)
          ",
            &[],
        )?;

        conn.execute(
            "
          ANALYZE
          ",
            &[],
        )?;

        Ok(DB { pool })
    }

    pub fn analyze(&self) -> Result<(), Error> {
        let conn = self.pool.get()?;
        conn.execute("ANALYZE", &[])?;
        Ok(())
    }

    pub fn store_comic(&self, info: &ComicInfo, entries: &[String]) -> Result<i64, Error> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare_cached("insert into issue(filepath, modified_at, size, comicvine_id,
            comicvine_url, series, issue_number, volume, title, summary, released_at, writer, penciller,
            inker, colorist, cover_artist, publisher, page_count) values (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
            ON CONFLICT(filepath) DO UPDATE SET
            modified_at = excluded.modified_at, size = excluded.size, comicvine_id = excluded.comicvine_id,
            comicvine_url = excluded.comicvine_url, series = excluded.series, issue_number = excluded.issue_number,
            volume = excluded.volume, title = excluded.title, summary = excluded.summary, released_at = excluded.released_at,
            writer = excluded.writer, penciller = excluded.penciller, inker = excluded.inker, colorist = excluded.colorist,
            cover_artist = excluded.cover_artist, publisher = excluded.publisher, page_count = excluded.page_count
                                           ")?;

        stmt.insert(&[
            &info.filepath,
            &info.modified_at,
            &info.size,
            &info.comicvine_id,
            &info.comicvine_url,
            &info.series,
            &info.issue_number,
            &info.volume,
            &info.title,
            &info.summary,
            &info.released_at,
            &info.writer,
            &info.penciller,
            &info.inker,
            &info.colorist,
            &info.cover_artist,
            &info.publisher,
            &info.page_count,
        ])?;

        // On upserts, the last rowid thing doesn't work
        let issue_id: i64 = conn.query_row(
            "select rowid from issue where filepath=?",
            &[&info.filepath],
            |row| row.get(0),
        )?;

        if let Some(ref comic_info) = info.comic_info {
            stmt =
                conn.prepare_cached("replace into issue_fts(issue_id, comicinfo) values (?1, ?2)")?;
            stmt.insert(&[&issue_id, comic_info])?;
        }

        if !entries.is_empty() {
            conn.execute("delete from page where issue_id = ?", &[&issue_id])?;
            stmt = conn.prepare_cached("insert into page(issue_id, entry) values (?, ?)")?;
            for entry in entries {
                stmt.insert(&[&issue_id, &entry.as_str()])?;
            }
        }
        Ok(issue_id)
    }

    // Basically the only time we shouldn't update is if we know
    // that path hasn't be modified since the last mod_time
    pub fn should_update(&self, entry: &DirEntry) -> bool {
        let path: String = entry.path().to_string_lossy().into();
        let modified = super::entry_modified(entry.path());

        if let Ok(conn) = self.pool.get() {
            conn.query_row(
                "select modified_at from issue where filepath=?",
                &[&path],
                |row| {
                    let modified_at: DateTime<Local> = row.get(0);
                    modified_at < modified
                },
            ).unwrap_or(true)
        } else {
            true
        }
    }

    pub fn get_all(&self) -> Result<Vec<ComicInfo>, Error> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare_cached(SELECT_CLAUSE)?;
        let iter = stmt.query_map(&[], row_to_entry)?;
        let mut retval = Vec::new();
        for comic in iter {
            retval.push(comic?)
        }
        Ok(retval)
    }

    pub fn get_recent(&self) -> Result<Vec<ComicInfo>, Error> {
        let conn = self.pool.get()?;
        let mut stmt =
            conn.prepare_cached(&format!("{} order by released_at desc", SELECT_CLAUSE))?;
        let iter = stmt.query_map(&[], row_to_entry)?;
        let mut retval = Vec::new();
        for comic in iter {
            retval.push(comic?)
        }
        Ok(retval)
    }

    pub fn get_unread(&self, user_id: i64) -> Result<Vec<ComicInfo>, Error> {
        let conn = self.pool.get()?;
        let mut stmt =
            conn.prepare_cached(
                &format!("{} left join (select issue_id from read where user_id = ?) r on i.rowid = r.issue_id where r.issue_id is null order by released_at", SELECT_CLAUSE)
                )?;
        let iter = stmt.query_map(&[&user_id], row_to_entry)?;
        let mut retval = Vec::new();
        for comic in iter {
            retval.push(comic?)
        }
        Ok(retval)
    }

    pub fn get_unread_series(&self, user_id: i64) -> Result<Vec<(String, DateTime<Utc>)>, Error> {
        let conn = self.pool.get()?;
        let mut stmt =
            conn.prepare_cached("select series, max(modified_at) from issue i left join (select issue_id from read where user_id = ?) r on i.rowid = r.issue_id where r.issue_id is null group by 1 order by series")?;
        let mut rows = stmt.query(&[&user_id])?;
        let mut series = Vec::new();

        while let Some(row) = rows.next() {
            let row = row?;
            let s: Option<String> = row.get(0);
            series.push((s.unwrap_or_else(|| "None".to_owned()), row.get(1)));
        }
        Ok(series)
    }

    pub fn get_recent_unread_series(&self, user_id: i64) -> Result<Vec<(String, DateTime<Utc>)>, Error> {
        let conn = self.pool.get()?;
        let mut stmt =
            conn.prepare_cached("select series, max(modified_at) from issue i left join (select issue_id from read where user_id = ?) r on i.rowid = r.issue_id where r.issue_id is null and date('now','-6 months') < i.released_at and i.series is not null group by 1 order by series")?;
        let mut rows = stmt.query(&[&user_id])?;
        let mut series = Vec::new();

        while let Some(row) = rows.next() {
            let row = row?;
            series.push((row.get(0), row.get(1)));
        }
        Ok(series)
    }

    pub fn get_unread_for_series(
        &self,
        user_id: i64,
        series: &str,
    ) -> Result<Vec<ComicInfo>, Error> {
        let mut query = format!("{} left join (select issue_id from read where user_id = ?) r on i.rowid = r.issue_id where r.issue_id is null ", SELECT_CLAUSE);

        let conn = self.pool.get()?;
        // The ? in the None case will pick up both the unlikely event that there really
        // is a series named None as well as making the binds happy
        match series {
            "None" => query.push_str("and (series is null or series = ?) "),
            _ => query.push_str("and series = ? "),
        };

        query.push_str(" order by released_at");
        let mut stmt = conn.prepare_cached(&query)?;
        let iter = stmt.query_map(&[&user_id, &series], row_to_entry)?;
        let mut retval = Vec::new();
        for comic in iter {
            retval.push(comic?)
        }
        Ok(retval)
    }

    pub fn get_publishers(&self) -> Result<Vec<(String, DateTime<Utc>)>, Error> {
        let conn = self.pool.get()?;
        let mut stmt =
            conn.prepare_cached("select publisher, max(modified_at) from issue group by 1")?;
        let mut rows = stmt.query(&[])?;

        let mut pubs = Vec::new();

        while let Some(row) = rows.next() {
            let row = row?;
            let publisher: Option<String> = row.get(0);
            pubs.push((publisher.unwrap_or_else(|| "None".to_owned()), row.get(1)));
        }
        Ok(pubs)
    }

    pub fn get_series_for_publisher(
        &self,
        publisher: &str,
    ) -> Result<Vec<(String, DateTime<Utc>)>, Error> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare_cached(
            "select series, max(modified_at) from issue where publisher = ? group by 1",
        )?;
        let mut rows = stmt.query(&[&publisher])?;

        let mut series = Vec::new();

        while let Some(row) = rows.next() {
            let row = row?;
            let publisher: Option<String> = row.get(0);
            series.push((publisher.unwrap_or_else(|| "None".to_owned()), row.get(1)));
        }
        Ok(series)
    }

    pub fn get_for_publisher_series(
        &self,
        publisher: &str,
        series: &str,
    ) -> Result<Vec<ComicInfo>, Error> {
        let conn = self.pool.get()?;
        // The ? in the None case will pick up both the unlikely event that there really
        // is a publisher named None as well as making the binds happy
        let mut where_clause = match publisher {
            "None" => "where publisher is null or publisher = ?".to_owned(),
            _ => "where publisher = ?".to_owned(),
        };

        match series {
            "None" => where_clause.push_str(" and series is null or series = ?"),
            _ => where_clause.push_str("and series = ?"),
        }
        let mut stmt = conn.prepare_cached(&format!("{} {}", SELECT_CLAUSE, where_clause))?;
        let iter = stmt.query_map(&[&publisher, &series], row_to_entry)?;
        let mut retval = Vec::new();
        for comic in iter {
            retval.push(comic?)
        }
        Ok(retval)
    }

    pub fn get(&self, id: i64) -> Result<ComicInfo, Error> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare_cached(&format!("{} where rowid = ?", SELECT_CLAUSE))?;
        Ok(stmt.query_row(&[&id], row_to_entry)?)
    }

    pub fn get_page(
        &self,
        issue_id: i64,
        page_id: i32,
        user_id: i64,
    ) -> Result<(String, Vec<u8>), Error> {
        #[derive(Default)]
        struct Entry {
            issue: String,
            entry: String,
        };

        let page_id: usize = page_id as usize;

        let conn = self.pool.get()?;
        let mut stmt = conn.prepare_cached("select i.filepath, p.entry from issue i, page p where i.rowid = p.issue_id and i.rowid = ? order by p.entry")?;
        let iter = stmt.query_map(&[&issue_id], |r| Entry {
            issue: r.get(0),
            entry: r.get(1),
        })?;

        let entries: Vec<Entry> = iter.map(|e| e.unwrap_or_default())
            .filter(|e| {
                e.entry.ends_with("jpg") || e.entry.ends_with("gif") || e.entry.ends_with("png")
            })
            .collect();

        if page_id < entries.len() {
            if page_id + 3 > entries.len() {
                self.mark_read(issue_id, user_id).ok(); // Ignore the error
            }
            Ok((
                entries[page_id].entry.clone(),
                super::get_bytes_for_entry(&entries[page_id].issue, &entries[page_id].entry)?,
            ))
        } else {
            Err(::failure::err_msg("No such page"))
        }
    }

    pub fn mark_read(&self, issue_id: i64, user_id: i64) -> Result<usize, Error> {
        let conn = self.pool.get()?;
        let mut stmt =
            conn.prepare_cached("replace into read(user_id, issue_id, read_at) values(?,?,?)")?;
        Ok(stmt.execute(&[&user_id, &issue_id, &Local::now()])?)
    }

    /// grabs as user_id given a name.  If you want to check the password use
    /// check_or_provision_user, this version will raise an error if not found
    pub fn get_user(&self, username: &str) -> Result<i64, Error> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare_cached("select rowid from user where username = ?")?;
        Ok(stmt.query_row(&[&username], |row| row.get(0))?)
    }

    /// either check if the password is correct or make a user with the password
    /// if auth fails, return 0, otherwise the user_id
    pub fn check_or_provision_user(&self, username: &str, password: &str) -> Result<i64, Error> {
        use argon2rs::Argon2;
        let a2 = Argon2::default(::argon2rs::Variant::Argon2i);
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare_cached(
            "select rowid, username, salt, ciphertext from user where username = ?",
        )?;
        let mut rows = stmt.query(&[&username])?;
        if let Some(Ok(row)) = rows.next() {
            let mut ciphertext = [0; 32];
            let salt: Vec<u8> = row.get(2);
            let db_cipher: Vec<u8> = row.get(3);
            a2.hash(
                &mut ciphertext,
                password.as_bytes(),
                salt.as_slice(),
                &[],
                &[],
            );
            if ciphertext.to_vec() == db_cipher {
                Ok(row.get(0))
            } else {
                Ok(0)
            }
        } else {
            use rand::os::OsRng;
            use rand::RngCore;
            let mut osrng = OsRng::new().unwrap(); // supposed to not really fail
            let mut salt = [0; 32];
            osrng.fill_bytes(&mut salt[..]);
            let mut ciphertext = [0; 32];
            a2.hash(&mut ciphertext, password.as_bytes(), &salt, &[], &[]);

            let mut stmt = conn.prepare_cached(
                "insert into user(username, salt, ciphertext) values (?, ?, ?)",
            )?;
            let user_id = stmt.insert(&[&username, &salt.to_vec(), &ciphertext.to_vec()])?;
            Ok(user_id)
        }
    }
}

const SELECT_CLAUSE: &str = "select i.rowid, i.filepath, i.modified_at, i.size, i.comicvine_id, i.comicvine_url, i.series, i.issue_number, i.volume, i.title, i.summary, i.released_at, i.writer, i.penciller, i.inker, i.colorist, i.cover_artist, i.publisher, i.page_count from issue i";

fn row_to_entry(row: &Row) -> ComicInfo {
    ComicInfo {
        comic_info: None,
        id: row.get(0),
        filepath: row.get(1),
        modified_at: row.get(2),
        size: row.get(3),
        comicvine_id: row.get(4),
        comicvine_url: row.get(5),
        series: row.get(6),
        issue_number: row.get(7),
        volume: row.get(8),
        title: row.get(9),
        summary: row.get(10),
        released_at: row.get(11),
        writer: row.get(12),
        penciller: row.get(13),
        inker: row.get(14),
        colorist: row.get(15),
        cover_artist: row.get(16),
        publisher: row.get(17),
        page_count: row.get(18),
    }
}
