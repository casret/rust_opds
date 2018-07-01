extern crate chrono;
extern crate failure;
extern crate rusqlite;
extern crate walkdir;

use super::ComicInfo;
use chrono::prelude::*;
use failure::Error;
use rusqlite::Connection;
use walkdir::DirEntry;

pub struct DB {
    conn: Connection,
}

impl DB {
    pub fn new(db: &str) -> Result<DB, Error> {
        Ok(DB {
            conn: Connection::open(db)?,
        })
    }

    pub fn store_comic(&mut self, info: &ComicInfo) -> Result<(), Error> {
        let mut stmt = self.conn.prepare_cached("replace into issue(filepath, modified_at, comicvine_id,
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
            stmt = self.conn
                .prepare_cached("replace into issue_fts(issue_id, comicinfo) values (?1, ?2)")?;
            stmt.insert(&[&issue_id, comic_info])?;
        }
        Ok(())
    }

    // Basically the only time we shouldn't update is if we know
    // that path hasn't be modified since the last mod_time
    pub fn should_update(&mut self, entry: &DirEntry) -> bool {
        let path: String = entry.path().to_string_lossy().into();
        let modified = match super::entry_modified(entry) {
            Some(modified) => modified,
            _ => return true,
        };

        self.conn
            .query_row(
                "select modified_at from issue where filepath=?",
                &[&path],
                |row| {
                    let modified_at: DateTime<Local> = row.get(0);
                    modified_at < modified
                },
            )
            .unwrap_or(true)
    }
}
