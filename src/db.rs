extern crate chrono;
extern crate failure;
extern crate rusqlite;
extern crate walkdir;

use super::ComicInfo;
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

    pub fn store_comic(&mut self, info: &ComicInfo, comic_info: &str) -> Result<(), Error> {
        let mut stmt = self.conn.prepare_cached("insert into issue(filepath, imported_at, read_at, comicvine_id,
            series, issue_number, volume, title, summary, released_at, writer, penciller,
            inker, colorist, cover_artist, publisher, page_count) values (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)")?;

        let issue_id = stmt.insert(&[
            &info.filepath,
            &info.imported_at,
            &info.read_at,
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

        stmt = self.conn
            .prepare_cached("insert into issue_fts(issue_id, comicinfo) values (?1, ?2)")?;
        stmt.insert(&[&issue_id, &comic_info])?;
        Ok(())
    }

    pub fn check(&self, _entry: &DirEntry) -> bool {
        true
    }
}
