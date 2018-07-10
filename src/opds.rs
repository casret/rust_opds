use super::ComicInfo;
use chrono::prelude::*;
use failure::Error;
use std::borrow::Cow;
use std::io::prelude::*;
use url::percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
use uuid::Uuid;
use xml::name::Name;
use xml::writer::{EventWriter, XmlEvent};

#[derive(Debug)]
pub enum Rel {
    RelSelf,
    Start,
    Subsection,
    SortNew,
    Image,
    Thumbnail,
    Acquisition,
}

impl Rel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Rel::RelSelf => "self",
            Rel::Start => "start",
            Rel::Subsection => "subsection",
            Rel::SortNew => "http://opds-spec.org/sort/new",
            Rel::Image => "http://opds-spec.org/image",
            Rel::Thumbnail => "http://opds-spec.org/image/thumbnail",
            Rel::Acquisition => "http://opds-spec.org/acquisition",
        }
    }
}

#[derive(Debug)]
pub enum LinkType {
    Jpeg,
    Acquisition,
    Navigation,
    OctetStream,
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkType::Jpeg => "image/jpeg",
            LinkType::Acquisition => "application/atom+xml; profile=opds-catalog; kind=acquisition",
            LinkType::Navigation => "application/atom+xml; profile=opds-catalog; kind=navigation",
            LinkType::OctetStream => "application/octet-stream",
        }
    }
}

#[derive(Debug)]
struct OpdsLink<'a> {
    link_type: LinkType,
    rel: Rel,
    url: Cow<'a, str>,
}

#[derive(Debug)]
struct OpdsEntry<'a> {
    id: Cow<'a, str>,
    updated: DateTime<Utc>,
    title: Cow<'a, str>,
    content: Cow<'a, str>,
    authors: Vec<&'a str>,
    links: Vec<OpdsLink<'a>>,
}

impl<'a> OpdsEntry<'a> {
    fn new(
        title: &'a str,
        content: &'a str,
        authors: Vec<&'a str>,
        links: Vec<OpdsLink<'a>>,
    ) -> OpdsEntry<'a> {
        OpdsEntry {
            id: Cow::Owned(get_uuid_id()),
            updated: Utc::now(),
            title: Cow::Borrowed(title),
            content: Cow::Borrowed(content),
            authors,
            links,
        }
    }
}

#[derive(Debug)]
struct OpdsFeed<'a> {
    id: &'a str,
    title: &'a str,
    entries: Vec<OpdsEntry<'a>>,
    links: Vec<OpdsLink<'a>>,
    updated: DateTime<Utc>,
}

pub fn make_acquisition_feed(
    url: &str,
    title: &str,
    entries: &[ComicInfo],
) -> Result<String, Error> {
    let links = vec![
        OpdsLink {
            link_type: LinkType::Acquisition,
            rel: Rel::RelSelf,
            url: Cow::Borrowed(url),
        },
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::Start,
            url: Cow::Borrowed("/"),
        },
    ];
    let entries = entries.into_iter().map(|e| make_entry(e)).collect();

    let feed = OpdsFeed {
        id: &get_uuid_id(),
        title,
        updated: Utc::now(),
        links,
        entries,
    };
    write_opds(&feed)
}

pub fn make_subsection_feed(
    url_prefix: &str,
    title: &str,
    subs: &mut Vec<String>,
) -> Result<String, Error> {
    let links = vec![
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::RelSelf,
            url: Cow::Borrowed(url_prefix),
        },
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::Start,
            url: Cow::Borrowed("/"),
        },
    ];

    let entries = subs.iter_mut()
        .map(|sub| {
            let url = utf8_percent_encode(&format!("{}/{}", url_prefix, sub), DEFAULT_ENCODE_SET)
                .to_string();
            OpdsEntry::new(
                sub,
                sub,
                Vec::new(),
                vec![OpdsLink {
                    link_type: LinkType::Navigation,
                    rel: Rel::Subsection,
                    url: Cow::Owned(url),
                }],
            )
        })
        .collect();

    let feed = OpdsFeed {
        id: &get_uuid_id(),
        title: &title,
        updated: Utc::now(),
        links,
        entries,
    };
    write_opds(&feed)
}
pub fn make_navigation_feed() -> Result<String, Error> {
    let links = vec![
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::RelSelf,
            url: Cow::Borrowed("/"),
        },
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::Start,
            url: Cow::Borrowed("/"),
        },
    ];

    let entries = vec![
        OpdsEntry::new(
            "All comics",
            "All comics as a flat list",
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Acquisition,
                rel: Rel::Subsection,
                url: Cow::Borrowed("/all"),
            }],
        ),
        OpdsEntry::new(
            "Recent comics",
            "All comics sorted by recency",
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Acquisition,
                rel: Rel::SortNew,
                url: Cow::Borrowed("/recent"),
            }],
        ),
        OpdsEntry::new(
            "Unread comics",
            "All unread comics sorted by published date",
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Acquisition,
                rel: Rel::Subsection,
                url: Cow::Borrowed("/unread"),
            }],
        ),
    ];

    let feed = OpdsFeed {
        id: &get_uuid_id(),
        title: "Rust OPDS",
        updated: Utc::now(),
        links,
        entries,
    };
    write_opds(&feed)
}

fn make_entry(entry: &ComicInfo) -> OpdsEntry {
    let mut authors: Vec<&str> = Vec::new();
    if let Some(ref writer) = entry.writer {
        authors.push(&writer);
    }
    if let Some(ref penciller) = entry.penciller {
        authors.push(&penciller);
    }
    if let Some(ref inker) = entry.inker {
        authors.push(&inker);
    }
    if let Some(ref colorist) = entry.colorist {
        authors.push(&colorist);
    }
    if let Some(ref cover_artist) = entry.cover_artist {
        authors.push(&cover_artist);
    }

    let url_prefix = format!("/comic/{}", entry.id.unwrap_or(0));
    let filename: String =
        utf8_percent_encode(&entry.get_filename(), DEFAULT_ENCODE_SET).to_string();
    let links = vec![
        OpdsLink {
            link_type: LinkType::Jpeg,
            rel: Rel::Image,
            url: Cow::Owned(format!("/cover/{}/cover.jpg", entry.id.unwrap_or(0))),
        },
        OpdsLink {
            link_type: LinkType::Jpeg,
            rel: Rel::Thumbnail,
            url: Cow::Owned(format!("/cover/{}/cover.jpg", entry.id.unwrap_or(0))),
        },
        OpdsLink {
            link_type: LinkType::OctetStream,
            rel: Rel::Acquisition,
            url: Cow::Owned(format!("{}/download/{}", url_prefix, filename)),
        },
    ];

    let series = entry.series.as_ref().map_or("", |x| &**x);
    let title = entry.title.as_ref().map_or("", |x| &**x);
    let summary = entry.summary.as_ref().map_or("", |x| &**x);
    OpdsEntry {
        id: Cow::Owned(entry.id.unwrap_or(0).to_string()),
        updated: entry.modified_at.with_timezone(&Utc),
        title: Cow::Owned(format!(
            "{} v{} {}",
            series,
            entry.volume.unwrap_or(1),
            entry.issue_number.unwrap_or(1)
        )),
        content: Cow::Owned(format!("{} {}", title, summary)),
        authors,
        links,
    }
}

fn get_uuid_id() -> String {
    format!("urn:uuid:{}", Uuid::new_v4())
}

fn write_links<W: Write>(writer: &mut EventWriter<W>, links: &[OpdsLink]) -> Result<(), Error> {
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

fn write_opds(opds: &OpdsFeed) -> Result<String, Error> {
    let raw = Vec::new();
    let mut writer = EventWriter::new(raw);
    writer.write(
        XmlEvent::start_element("feed")
            .default_ns("http://www.w3.org/2005/Atom")
            .ns("opds", "http://opds-spec.org/2010/catalog"),
    )?;

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

    for entry in &opds.entries {
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

        writer.write(XmlEvent::start_element("content").attr("type", "html"))?;
        writer.write(XmlEvent::characters(&entry.content))?;
        writer.write(XmlEvent::end_element())?;

        writer.write(XmlEvent::start_element("author"))?;
        for author in &entry.authors {
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
