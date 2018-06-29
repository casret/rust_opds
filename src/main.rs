extern crate rust_opds;

extern crate chrono;
extern crate failure;
extern crate rusqlite;
extern crate walkdir;
extern crate unrar;
extern crate zip;

use std::io::prelude::*;
use failure::Error;
use rusqlite::Connection;
use walkdir::{DirEntry, WalkDir};
use std::path::PathBuf;
use chrono::prelude::*;
use rust_opds::ComicInfo;


fn main() {
    let conn = Connection::open("comics.db").unwrap();
    for entry in WalkDir::new("/Users/casret/comics").into_iter()
    .filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) { 
        match entry.file_name().to_str() {
            Some(name) if name.ends_with("cbr") => process_rar(&entry, &conn),
            Some(name) if name.ends_with("cbz") => process_zip(&entry, &conn),
            _ => { println!("Skipping {}", entry.path().display()); Ok(()) },
        }.unwrap();
    }
}


fn store_comic_info(entry: &DirEntry, comic_info: &String, conn: &Connection) -> Result<(), Error> {
    println!("Will store {}", entry.path().display());
    let info = ComicInfo::new(&entry.path().to_string_lossy(), comic_info)?;
    let mut stmt = conn.prepare_cached("insert into issue(filepath, imported_at, read_at, comicvine_id,
        series, issue_number, volume, title, summary, released_at, writer, penciller,
        inker, colorist, cover_artist, publisher, page_count) values (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)")?;

    stmt.insert(&[&info.filepath, &info.imported_at, &info.read_at, &info.comicvine_id, &info.series, &info.issue_number,
                &info.volume, &info.title, &info.summary, &info.released_at, &info.writer, &info.penciller,
                &info.inker, &info.colorist, &info.cover_artist, &info.publisher, &info.page_count]);


    stmt = conn.prepare_cached("insert into issue_fts(comicinfo) values (?)")?;
    stmt.insert(&[comic_info])?;
    Ok(())
}

fn process_rar(file: &DirEntry, _conn: &Connection) -> Result<(), Error> {
    //println!("Processing {}", file.path().display());
    for entry in unrar::Archive::new(file.path().to_string_lossy().into()).list().unwrap() {
        if let Ok(entry) = entry {
            if entry.filename != "ComicInfo.xml" { continue; }
            // TODO: extract and send
        }
    }
    Ok(())
}

fn process_zip(entry: &DirEntry, conn: &Connection) -> Result<(), Error> {
    //println!("Processing {}", entry.path().display());
    let zipfile = std::fs::File::open(&entry.path()).unwrap();
    let mut archive = zip::ZipArchive::new(zipfile).unwrap();

    let mut file = match archive.by_name("ComicInfo.xml") {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(()),
        Err(e) => Err(e)?
    };

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    store_comic_info(entry, &contents, conn)
}
