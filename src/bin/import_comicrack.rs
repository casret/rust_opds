#![allow(non_snake_case)]

extern crate env_logger;
extern crate failure;
extern crate rust_opds;
extern crate toml;
#[macro_use] extern crate serde_derive;
extern crate serde_xml_rs;
#[macro_use]
extern crate lazy_static;
extern crate regex;

use serde_xml_rs::deserialize;


use failure::Error;
use rust_opds::{Config, db, ComicInfo};
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use regex::Regex;
use std::env;
use std::path::PathBuf;


#[derive(Debug, Deserialize)]
struct Book {
    pub File: String,
    pub PageCount: Option<String>,
    pub LastPageRead: Option<String>
}

/// Utility to import in the comicrack database.  As currently written
/// it is meant to run once, right after the initial import of the comic database,
/// overwriting any metadata in the DB.
#[allow(trivial_regex)]
fn main() -> Result<(), Error> {
    env_logger::init();

    lazy_static! {
        static ref BOOK_START_RE: Regex = Regex::new(r#"<Book Id.*File="(.*)""#).unwrap();
        static ref BOOK_END_RE: Regex = Regex::new(r"</Book>").unwrap();
    }

    let args: Vec<String> = env::args().collect();

    let mut file = File::open("config.toml")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let config: Config = toml::from_str(&contents)?;
    let db = db::DB::new(config.database_path.as_path())?;
    let user_id: Option<i64> = match config.import_comicrack {
        Some(ref import_comicrack) => if let Some(ref read_user) = import_comicrack.read_user {
            Some(db.get_user(&read_user)?)
        } else {
            None
        }
        _ => None
    };

    println!("config: {:#?}", config);

    // I could parse the whole thing as an XML, but since
    // the comicinfo constructor already does it I'll just
    // slice out the ComicInfo blocks after parsing and transforming the filepath 
    let file = File::open(&args[1])?;
    let mut file = BufReader::new(file);
    let mut book_buf = String::new();
    let mut in_book = false;
    loop {
        if 0 == file.read_line(&mut book_buf)? {
            break;
        }
        if in_book && BOOK_END_RE.is_match(&book_buf) {
            in_book = false;
            let book: Book = deserialize(book_buf.as_bytes())?;
            process_book(&config, &db, book, &book_buf, user_id)?;
            book_buf.clear();
        } else if !in_book && BOOK_START_RE.is_match(&book_buf) {
            in_book = true;
        } else if !in_book {
            book_buf.clear();
        }
    }
    Ok(())
}

fn process_book(config: &Config, db: &db::DB, book: Book, comic_info: &str, user_id: Option<i64>) -> Result<(), Error> {
    let path = if cfg!(unix) {
        // kludge conversion of windows path to unix style
        
        let mut file = book.File;
        if let Some(ref config) = config.import_comicrack {
            if let Some(ref strip) = config.strip_prefix {
                file = file.trim_left_matches(strip).to_string();
            }
        }
        let mut path = config.comics_path.clone();
        path.push(file.replace("\\", "/"));
        path
    } else {
        PathBuf::from(book.File)
    };

    if !path.exists() {
        println!("Can't find {:#?} - skipping", path);
        return Ok(());
    }

    let read = if let Some(last) = book.LastPageRead {
        if let Some(pages) = book.PageCount {
            (pages.parse::<u32>().unwrap_or(std::u32::MAX) - 2) <= last.parse::<u32>().unwrap_or(0) 
        } else {
            false
        }
    } else {
        false
    };

    let issue_id = db.store_comic(&ComicInfo::new(&path, Some(comic_info.to_string()))?, &Vec::new())?;
    if read && user_id.is_some() {
        println!("Marking {:#?} as read", path);
        db.mark_read(issue_id, user_id.unwrap())?;
    }
    Ok(())
}
