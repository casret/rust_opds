#[macro_use]
extern crate serde_derive;

extern crate argon2rs;
extern crate base64;
extern crate chrono;
extern crate env_logger;
extern crate serde;
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
extern crate tokio_threadpool;
extern crate unrar;
extern crate url;
extern crate uuid;
extern crate walkdir;
extern crate xml;
extern crate zip;

use chrono::prelude::*;
use failure::Error;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::sync::Arc;
use std::thread;
use walkdir::{DirEntry, WalkDir};
use xml::reader::{EventReader, XmlEvent};

pub mod db;
mod opds;
pub mod web;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    addr: SocketAddr,
    pub comics_path: PathBuf,
    pub database_path: PathBuf,
    pub tag_authority: String,
    pub import_comicrack: Option<ImportConfig>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ImportConfig {
    pub strip_prefix: Option<String>,
    pub read_user: Option<String>,
}

pub struct ComicInfo {
    pub id: Option<i64>,
    pub comic_info: Option<String>,
    pub filepath: String,
    pub size: i32,
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
    pub fn new(entry: &Path, comic_info: Option<String>) -> Result<ComicInfo, Error> {
        let mut info = ComicInfo {
            id: None, // You don't get an Id until you are in the DB
            comic_info,
            filepath: entry.to_string_lossy().to_string(),
            modified_at: entry_modified(entry),
            size: entry_size(entry) as i32,
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

pub fn run(config: Config) -> Result<(), Error> {
    let db = Arc::new(db::DB::new(config.database_path.as_path())?);
    let config = Arc::new(config);
    let scan_db = Arc::clone(&db);
    let comics_path = config.comics_path.clone();
    thread::spawn(move || match scan_dir(comics_path.as_path(), &scan_db) {
        Err(e) => error!("Error scanning: {}, {}", e, e.backtrace()),
        _ => info!("Done scanning directory"),
    });
    web::start_web_service(Arc::clone(&db), config)?;
    Ok(())
}

// TODO: probably move all the compression stuff to another module
pub fn get_bytes_for_entry(filepath: &str, entry: &str) -> Result<Vec<u8>, Error> {
    if filepath.ends_with("cbr") {
        let archive = unrar::Archive::new(filepath.to_owned());
        match archive.read_bytes(entry) {
            Err(e) => Err(format_err!("Rar error {}", e)),
            Ok(e) => Ok(e),
        }
    } else if filepath.ends_with("cbz") {
        let fname = std::path::Path::new(filepath);
        let zipfile = std::fs::File::open(&fname)?;
        let mut archive = zip::ZipArchive::new(zipfile)?;
        let mut file = archive.by_name(entry)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        Ok(contents)
    } else {
        Err(failure::err_msg("Unsupported archive"))
    }
}

fn scan_dir(dir: &Path, db: &Arc<db::DB>) -> Result<(), Error> {
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
            Ok((comic_info, entries)) => {
                db.store_comic(&ComicInfo::new(&entry.path(), comic_info)?, &entries)?;
            }
            Err(e) => error!("Skipping {}: {}", entry.path().display(), e),
        }
    }
    db.analyze()?;
    Ok(())
}

fn process_rar(file: &DirEntry) -> Result<(Option<String>, Vec<String>), Error> {
    info!("Processing {}", file.path().display());

    let archive = unrar::Archive::new(file.path().to_string_lossy().into());

    let mut entries: Vec<String> = Vec::new();

    match archive.list() {
        Ok(archive) => for entry in archive {
            match entry {
                Ok(e) => entries.push(e.filename),
                Err(e) => return Err(format_err!("{}", e)),
            }
        },
        Err(e) => return Err(format_err!("{}", e)),
    };

    if entries.iter().any(|e| e == "ComicInfo.xml") {
        let archive = unrar::Archive::new(file.path().to_string_lossy().into());
        match archive.read_bytes("ComicInfo.xml") {
            Ok(bytes) => Ok((Some(str::from_utf8(&bytes)?.to_owned()), entries)),
            Err(e) => Err(format_err!("{}", e)),
        }
    } else {
        Ok((None, entries))
    }
}

fn process_zip(entry: &DirEntry) -> Result<(Option<String>, Vec<String>), Error> {
    info!("Processing {}", entry.path().display());
    let zipfile = std::fs::File::open(&entry.path())?;
    let mut archive = zip::ZipArchive::new(zipfile)?;

    let mut comic_info = None;
    let mut entries = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        entries.push(file.name().to_owned());
        if file.name() == "ComicInfo.xml" {
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            comic_info = Some(contents);
        }
    }

    Ok((comic_info, entries))
}

fn entry_modified(entry: &Path) -> DateTime<Local> {
    match entry.metadata() {
        Ok(metadata) => match metadata.modified() {
            Ok(modified) => DateTime::from(modified),
            _ => Local::now(),
        },
        _ => Local::now(),
    }
}

fn entry_size(entry: &Path) -> u64 {
    match entry.metadata() {
        Ok(metadata) => metadata.len(),
        _ => 0,
    }
}
