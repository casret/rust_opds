extern crate chrono;
extern crate xml;
extern crate failure;

use chrono::prelude::*;
use failure::Error;
use xml::reader::{EventReader, XmlEvent};

pub struct ComicInfo {
    pub filepath: String,
    pub imported_at: DateTime<Local>,
    pub read_at: Option<DateTime<Local>>,
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
    pub page_count: Option<i32>
}

impl ComicInfo {
    pub fn new(filepath: &str, content: &str) -> Result<ComicInfo, Error> {
        let mut info = ComicInfo {
            filepath: filepath.to_string(),
            imported_at: Local::now(),
            read_at: None,
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
            page_count: None
        };
        let parser = EventReader::from_str(content);
        let mut current_string: String = String::from("");
        let mut year: Option<String> = None;
        let mut month: Option<String> = None;
        let mut day: Option<String> = None;
        for e in parser {
            match e {
                Ok(XmlEvent::Characters(s)) => current_string = s,
                Ok(XmlEvent::EndElement{name}) => {
                    match name.local_name.as_ref() {
                        "Title" => info.title = Some(current_string.clone()),
                        "Series" => info.series = Some(current_string.clone()),
                        "Number" => info.issue_number = Some(current_string.parse().unwrap()),
                        "Web" => info.comicvine_url = Some(current_string.clone()),
                        "Notes" => (), // TODO: Parse out the comicvine id
                        "Volume" => info.volume = Some(current_string.parse().unwrap()),
                        "Summary" => info.summary = Some(current_string.clone()),
                        "Year" => year = Some(current_string.clone()),
                        "Month" => month = Some(current_string.clone()),
                        "Day" => day = Some(current_string.clone()),
                        "Writer" => info.writer = Some(current_string.clone()),
                        "Penciller" => info.penciller = Some(current_string.clone()),
                        "Inker" => info.inker = Some(current_string.clone()),
                        "Colorist" => info.colorist = Some(current_string.clone()),
                        "CoverArtist" => info.cover_artist = Some(current_string.clone()),
                        "Publisher" => info.publisher = Some(current_string.clone()),
                        "PageCount" => info.page_count = Some(current_string.parse().unwrap()),
                        _ => ()
                    }
                },
                Err(e) => Err(e)?,
                _ => ()
            }
        }
        Ok(info)
    }
}
