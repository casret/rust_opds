use chrono::prelude::*;
use failure::Error;
use std::io::prelude::*;
use uuid::Uuid;
use xml::name::Name;
use xml::writer::{EmitterConfig, EventWriter, XmlEvent};

#[derive(Debug)]
pub enum Rel {
    RelSelf,
    Start,
    Subsection,
    SortNew,
}

impl Rel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Rel::RelSelf => "self",
            Rel::Start => "start",
            Rel::Subsection => "subsection",
            Rel::SortNew => "http://opds-spec.org/sort/new",
        }
    }
}

#[derive(Debug)]
pub enum LinkType {
    Jpeg,
    Acquisition,
    Navigation,
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkType::Jpeg => "image/jpeg",
            LinkType::Acquisition => {
                "application/atom+xml; profile=opds-catalog; kind=acquisition"
            }
            LinkType::Navigation => "application/atom+xml; profile=opds-catalog; kind=navigation",
        }
    }
}

#[derive(Debug)]
struct OpdsLink {
    link_type: LinkType,
    rel: Rel,
    url: String,
}

#[derive(Debug)]
struct OpdsEntry {
    id: String,
    updated: DateTime<Utc>,
    title: String,
    content: String,
    authors: Vec<String>,
    links: Vec<OpdsLink>,
}

impl OpdsEntry {
    fn new(
        title: String,
        content: String,
        authors: Vec<String>,
        links: Vec<OpdsLink>,
    ) -> OpdsEntry {
        OpdsEntry {
            id: get_uuid_id(),
            updated: Utc::now(),
            title,
            content,
            authors,
            links,
        }
    }
}

#[derive(Debug)]
struct OpdsFeed {
    id: String,
    title: String,
    entries: Vec<OpdsEntry>,
    links: Vec<OpdsLink>,
    updated: DateTime<Utc>,
}

pub fn get_navigation_feed() -> Result<String, Error> {
    let links = vec![
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::RelSelf,
            url: "/".to_owned(),
        },
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::Start,
            url: "/".to_owned(),
        },
    ];

    let entries = vec![
        OpdsEntry::new(
            "All comics".to_owned(),
            "All comics as a flat list".to_owned(),
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Acquisition,
                rel: Rel::Subsection,
                url: "/all".to_owned(),
            }],
        ),
        OpdsEntry::new(
            "Unread comics".to_owned(),
            "All unread comics sorted by recency".to_owned(),
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Acquisition,
                rel: Rel::SortNew,
                url: "/unread".to_owned(),
            }],
        ),
    ];

    let feed = OpdsFeed {
        id: get_uuid_id(),
        title: "Rust OPDS".to_owned(),
        updated: Utc::now(),
        links: links,
        entries: entries,
    };
    write_opds(feed)
}

fn get_uuid_id() -> String {
    format!("urn:uuid:{}", Uuid::new_v4())
}

fn write_links<W: Write>(writer: &mut EventWriter<W>, links: &Vec<OpdsLink>) -> Result<(), Error> {
    lazy_static! {
        static ref TYPE_NAME: Name<'static> = Name::local("type");
        static ref REL_NAME: Name<'static> = Name::local("rel");
        static ref HREF_NAME: Name<'static> = Name::local("href");
    }

    for link in links.iter() {
        writer.write(
            XmlEvent::start_element("link")
                .attr(*TYPE_NAME, link.link_type.as_str())
                .attr(*REL_NAME, link.rel.as_str())
                .attr(*HREF_NAME, &link.url),
        )?;
        writer.write(XmlEvent::end_element())?;
    }
    Ok(())
}

fn write_opds(opds: OpdsFeed) -> Result<String, Error> {
    let raw = Vec::new();
    let mut writer = EventWriter::new(raw);
    writer.write(XmlEvent::start_element("feed").default_ns("http://www.w3.org/2005/Atom").ns("opds", "http://opds-spec.org/2010/catalog"))?;

    writer.write(XmlEvent::start_element("id"))?;
    writer.write(XmlEvent::characters(&opds.id))?;
    writer.write(XmlEvent::end_element())?;

    writer.write(XmlEvent::start_element("title"))?;
    writer.write(XmlEvent::characters(&opds.title))?;
    writer.write(XmlEvent::end_element())?;

    writer.write(XmlEvent::start_element("updated"))?;
    writer.write(XmlEvent::characters(&opds.updated.to_rfc3339()))?;
    writer.write(XmlEvent::end_element())?;

    write_links(&mut writer, &opds.links)?;

    for entry in opds.entries.iter() {
        writer.write(XmlEvent::start_element("entry"))?;

        writer.write(XmlEvent::start_element("title"))?;
        writer.write(XmlEvent::characters(&entry.title))?;
        writer.write(XmlEvent::end_element())?;

        writer.write(XmlEvent::start_element("id"))?;
        writer.write(XmlEvent::characters(&entry.id))?;
        writer.write(XmlEvent::end_element())?;

        writer.write(XmlEvent::start_element("updated"))?;
        writer.write(XmlEvent::characters(&entry.updated.to_rfc3339()))?;
        writer.write(XmlEvent::end_element())?;

        writer.write(
            XmlEvent::start_element("content")
                .attr("type", "html")
                )?;
        writer.write(XmlEvent::characters(&entry.content))?;
        writer.write(XmlEvent::end_element())?;

        writer.write(XmlEvent::start_element("author"))?;
        for author in entry.authors.iter() {
            writer.write(XmlEvent::start_element("name"))?;
            writer.write(XmlEvent::characters(&author))?;
            writer.write(XmlEvent::end_element())?;
        }
        writer.write(XmlEvent::end_element())?;

        write_links(&mut writer, &entry.links)?;

        writer.write(XmlEvent::end_element())?; // entry
    }

    writer.write(XmlEvent::end_element())?; // feed
    Ok(String::from_utf8(writer.into_inner())?)
}
