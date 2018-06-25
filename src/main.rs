extern crate walkdir;
extern crate unrar;
extern crate zip;

use walkdir::{DirEntry, WalkDir};
use std::io::prelude::*;


fn main() {
    for entry in WalkDir::new("/Users/casret/comics").into_iter()
    .filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) { 
        match entry.file_name().to_str() {
            Some(name) if name.ends_with("cbr") => process_rar(&entry),
            Some(name) if name.ends_with("cbz") => process_zip(&entry),
            _ => println!("Skipping {}", entry.path().display()),
        }
    }
}

fn process_rar(entry: &DirEntry) {
    println!("Processing {}", entry.path().display());
    for entry in unrar::Archive::new(entry.path().to_string_lossy().into()).list().unwrap() {
        if let Ok(entry) = entry {
            if entry.filename != "ComicInfo.xml" { continue; }
            println!("{}", entry);
        }
    }
}

fn process_zip(entry: &DirEntry) {
    println!("Processing {}", entry.path().display());
    let zipfile = std::fs::File::open(&entry.path()).unwrap();
    let mut archive = zip::ZipArchive::new(zipfile).unwrap();

    let mut file = match archive.by_name("ComicInfo.xml")
    {
        Ok(file) => file,
        Err(..) => { println!("ComicInfo.xml not found"); return; }
    };

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    println!("{}", contents);
}
