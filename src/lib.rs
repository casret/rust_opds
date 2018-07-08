extern crate argon2rs;
extern crate base64;
extern crate chrono;
extern crate env_logger;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate r2d2;
extern crate r2d2_sqlite;
extern crate rand;
extern crate regex;
extern crate rusqlite;
extern crate tokio_fs;
extern crate tokio_io;
extern crate unrar;
extern crate url;
extern crate uuid;
extern crate walkdir;
extern crate xml;
extern crate zip;

use chrono::prelude::*;
use failure::Error;
use std::io::prelude::*;
use std::path::Path;
use std::str;
use std::sync::Arc;
use std::thread;
use walkdir::{DirEntry, WalkDir};
use xml::reader::{EventReader, XmlEvent};

mod db;
mod opds;
pub mod web;

pub struct ComicInfo {
    pub id: Option<i64>,
    pub comic_info: Option<String>,
    pub filepath: String,
    pub modified_at: DateTime<Local>,
    pub comicvine_id: Option<i64>,
    pub comicvine_url: Option<String>,
    pub series: Option<String>,
    pub issue_number: Option<i32>,
    pub volume: Option<i32>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub released_at: Option<NaiveDate>,
    pub writer: Option<String>,
    pub penciller: Option<String>,
    pub inker: Option<String>,
    pub colorist: Option<String>,
    pub cover_artist: Option<String>,
    pub publisher: Option<String>,
    pub page_count: Option<i32>,
}

impl ComicInfo {
    pub fn new(entry: &DirEntry, comic_info: Option<String>) -> Result<ComicInfo, Error> {
        let mut info = ComicInfo {
            id: None, // You don't get an Id until you are in the DB
            comic_info,
            filepath: entry.path().to_string_lossy().to_string(),
            modified_at: entry_modified(entry).unwrap_or_else(Local::now),
            comicvine_id: None,
            comicvine_url: None,
            series: None,
            issue_number: None,
            volume: None,
            title: None,
            summary: None,
            released_at: None,
            writer: None,
            penciller: None,
            inker: None,
            colorist: None,
            cover_artist: None,
            publisher: None,
            page_count: None,
        };
        if let Some(ref comic_info) = info.comic_info {
            let parser = EventReader::from_str(comic_info);
            let mut current_string: String = String::from("");
            let mut year: Option<i32> = None;
            let mut month: Option<u32> = None;
            let mut day: Option<u32> = None;
            for e in parser {
                match e {
                    Ok(XmlEvent::Characters(s)) => current_string = s,
                    Ok(XmlEvent::EndElement { name }) => {
                        match name.local_name.as_ref() {
                            "Title" => info.title = Some(current_string.clone()),
                            "Series" => info.series = Some(current_string.clone()),
                            "Number" => info.issue_number = current_string.parse().ok(),
                            "Web" => info.comicvine_url = Some(current_string.clone()),
                            "Notes" => (), // TODO: Parse out the comicvine id
                            "Volume" => info.volume = current_string.parse().ok(),
                            "Summary" => info.summary = Some(current_string.clone()),
                            "Year" => year = current_string.parse().ok(),
                            "Month" => month = current_string.parse().ok(),
                            "Day" => day = current_string.parse().ok(),
                            "Writer" => info.writer = Some(current_string.clone()),
                            "Penciller" => info.penciller = Some(current_string.clone()),
                            "Inker" => info.inker = Some(current_string.clone()),
                            "Colorist" => info.colorist = Some(current_string.clone()),
                            "CoverArtist" => info.cover_artist = Some(current_string.clone()),
                            "Publisher" => info.publisher = Some(current_string.clone()),
                            "PageCount" => info.page_count = current_string.parse().ok(),
                            _ => (),
                        }
                    }
                    Err(e) => Err(e)?,
                    _ => (),
                }
            }
            if year.is_some() {
                info.released_at =
                    NaiveDate::from_ymd_opt(year.unwrap(), month.unwrap_or(1), day.unwrap_or(1));
            }
        }
        Ok(info)
    }

    pub fn get_filename(&self) -> String {
        Path::new(&self.filepath)
            .file_name()
            .map(|s| s.to_string_lossy().into())
            .unwrap_or_default()
    }
}

pub fn run() -> Result<(), Error> {
    let db = Arc::new(db::DB::new("comics.db")?);
    let scan_db = Arc::clone(&db);
    thread::spawn(move || scan_dir("/Users/casret/comics", &scan_db) );
    let addr = ([127, 0, 0, 1], 3000);
    web::start_web_service(Arc::clone(&db), addr.into())?;
    Ok(())
}

fn scan_dir(dir: &str, db: &Arc<db::DB>) -> Result<(), Error> {
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if !db.should_update(&entry) {
            info!("Skipping unchanged {}", entry.path().display());
            continue;
        }

        let comic_info = match entry.file_name().to_str() {
            Some(name) if name.ends_with("cbr") => process_rar(&entry),
            Some(name) if name.ends_with("cbz") => process_zip(&entry),
            _ => {
                info!("Skipping {}", entry.path().display());
                continue;
            }
        };
        match comic_info {
            Ok(comic_info) => db.store_comic(&ComicInfo::new(&entry, comic_info)?)?,
            Err(e) => error!("Skipping {}: {}", entry.path().display(), e),
        }
    }
    Ok(())
}

fn process_rar(file: &DirEntry) -> Result<Option<String>, Error> {
    use unrar::error::{Code, UnrarError};
    info!("Processing {}", file.path().display());
    match unrar::Archive::new(file.path().to_string_lossy().into()).read_bytes("ComicInfo.xml") {
        Ok(bytes) => Ok(Some(str::from_utf8(&bytes)?.to_owned())),
        Err(UnrarError {
            code: Code::EndArchive,
            ..
        }) => Ok(None),
        Err(e) => Err(format_err!("{}", e)),
    }
}

fn process_zip(entry: &DirEntry) -> Result<Option<String>, Error> {
    info!("Processing {}", entry.path().display());
    let zipfile = std::fs::File::open(&entry.path())?;
    let mut archive = zip::ZipArchive::new(zipfile)?;

    let mut file = match archive.by_name("ComicInfo.xml") {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => Err(e)?,
    };

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(Some(contents))
}

fn entry_modified(entry: &DirEntry) -> Option<DateTime<Local>> {
    match entry.metadata() {
        Ok(metadata) => match metadata.modified() {
            Ok(modified) => Some(DateTime::from(modified)),
            _ => None,
        },
        _ => None,
    }
}
