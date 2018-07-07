use super::ComicInfo;
use chrono::prelude::*;
use failure::Error;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Row;
use walkdir::DirEntry;

#[derive(Clone)]
pub struct DB {
    pool: Pool<SqliteConnectionManager>,
}

impl DB {
    pub fn new(db: &str) -> Result<DB, Error> {
        let manager = SqliteConnectionManager::file(db);
        let pool = ::r2d2::Pool::new(manager)?;
        Ok(DB { pool })
    }

    pub fn store_comic(&self, info: &ComicInfo) -> Result<(), Error> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare_cached("replace into issue(filepath, modified_at, comicvine_id,
            comicvine_url, series, issue_number, volume, title, summary, released_at, writer, penciller,
            inker, colorist, cover_artist, publisher, page_count) values (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)")?;

        let issue_id = stmt.insert(&[
            &info.filepath,
            &info.modified_at,
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

        if let Some(ref comic_info) = info.comic_info {
            stmt =
                conn.prepare_cached("replace into issue_fts(issue_id, comicinfo) values (?1, ?2)")?;
            stmt.insert(&[&issue_id, comic_info])?;
        }
        Ok(())
    }

    // Basically the only time we shouldn't update is if we know
    // that path hasn't be modified since the last mod_time
    pub fn should_update(&self, entry: &DirEntry) -> bool {
        let path: String = entry.path().to_string_lossy().into();
        let modified = match super::entry_modified(entry) {
            Some(modified) => modified,
            _ => return true,
        };

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

    pub fn get(&self, id: i64) -> Result<ComicInfo, Error> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare_cached(&format!("{} where rowid = ?", SELECT_CLAUSE))?;
        Ok(stmt.query_row(&[&id], row_to_entry)?)
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

const SELECT_CLAUSE: &str = "select rowid, filepath, modified_at, comicvine_id, comicvine_url, series, issue_number, volume, title, summary, released_at, writer, penciller, inker, colorist, cover_artist, publisher, page_count from issue";

fn row_to_entry(row: &Row) -> ComicInfo {
    ComicInfo {
        comic_info: None,
        id: row.get(0),
        filepath: row.get(1),
        modified_at: row.get(2),
        comicvine_id: row.get(3),
        comicvine_url: row.get(4),
        series: row.get(5),
        issue_number: row.get(6),
        volume: row.get(7),
        title: row.get(8),
        summary: row.get(9),
        released_at: row.get(10),
        writer: row.get(11),
        penciller: row.get(12),
        inker: row.get(13),
        colorist: row.get(14),
        cover_artist: row.get(15),
        publisher: row.get(16),
        page_count: row.get(17),
    }
}
