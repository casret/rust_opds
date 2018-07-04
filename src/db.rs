use super::ComicInfo;
use chrono::prelude::*;
use failure::Error;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
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
            series, issue_number, volume, title, summary, released_at, writer, penciller,
            inker, colorist, cover_artist, publisher, page_count) values (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)")?;

        let issue_id = stmt.insert(&[
            &info.filepath,
            &info.modified_at,
            &info.comicvine_id,
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
}
