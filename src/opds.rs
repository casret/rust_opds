use super::ComicInfo;
use super::Config;
use chrono::prelude::*;
use failure::Error;
use std::borrow::Cow;
use std::io::prelude::*;
use url::percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
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
    Stream,
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
            Rel::Stream => "http://vaemendis.net/opds-pse/stream",
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
    count: Option<i32>,
}

#[derive(Debug)]
struct OpdsEntry<'a> {
    id: String,
    updated: DateTime<Utc>,
    title: Cow<'a, str>,
    content: Cow<'a, str>,
    authors: Vec<&'a str>,
    links: Vec<OpdsLink<'a>>,
}

impl<'a> OpdsEntry<'a> {
    fn new(
        id: String,
        title: &'a str,
        content: &'a str,
        authors: Vec<&'a str>,
        links: Vec<OpdsLink<'a>>,
        updated: DateTime<Utc>,
    ) -> OpdsEntry<'a> {
        OpdsEntry {
            id,
            title: Cow::Borrowed(title),
            content: Cow::Borrowed(content),
            authors,
            links,
            updated,
        }
    }
}

#[derive(Debug)]
struct OpdsFeed<'a> {
    id: String,
    title: &'a str,
    entries: Vec<OpdsEntry<'a>>,
    links: Vec<OpdsLink<'a>>,
    updated: DateTime<Utc>,
}

fn make_id_from_url(tag_authority: &str, url: &str) -> String {
    format!(
        "tag:{}:{}",
        tag_authority,
        url.trim_left_matches('/').replace("/", ":")
    )
}

pub fn make_acquisition_feed(
    config: &Config,
    url: &str,
    title: &str,
    entries: &[ComicInfo],
) -> Result<String, Error> {
    let id = make_id_from_url(&config.tag_authority, url);
    let links = vec![
        OpdsLink {
            link_type: LinkType::Acquisition,
            rel: Rel::RelSelf,
            url: Cow::Borrowed(url),
            count: None,
        },
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::Start,
            url: Cow::Borrowed("/"),
            count: None,
        },
    ];
    let entries = entries
        .into_iter()
        .map(|e| make_entry(&config.tag_authority, e))
        .collect();

    let feed = OpdsFeed {
        id,
        title,
        updated: Utc::now(),
        links,
        entries,
    };
    write_opds(&feed)
}

pub fn make_subsection_feed(
    config: &Config,
    url_prefix: &str,
    title: &str,
    subs: &mut Vec<(String, DateTime<Utc>)>,
) -> Result<String, Error> {
    let id = make_id_from_url(&config.tag_authority, url_prefix);
    let links = vec![
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::RelSelf,
            url: Cow::Borrowed(url_prefix),
            count: None,
        },
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::Start,
            url: Cow::Borrowed("/"),
            count: None,
        },
    ];

    let entries = subs.iter_mut()
        .map(|sub| {
            let url = utf8_percent_encode(&format!("{}/{}", url_prefix, sub.0), DEFAULT_ENCODE_SET)
                .to_string();
            let id = make_id_from_url(&config.tag_authority, &url);
            OpdsEntry::new(
                id,
                &sub.0,
                &sub.0,
                Vec::new(),
                vec![OpdsLink {
                    link_type: LinkType::Navigation,
                    rel: Rel::Subsection,
                    url: Cow::Owned(url),
                    count: None,
                }],
                sub.1,
            )
        })
        .collect();

    let feed = OpdsFeed {
        id,
        title: &title,
        updated: Utc::now(),
        links,
        entries,
    };
    write_opds(&feed)
}
pub fn make_navigation_feed(config: &Config) -> Result<String, Error> {
    let id = format!("tag:{}:top", config.tag_authority);
    let links = vec![
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::RelSelf,
            url: Cow::Borrowed("/"),
            count: None,
        },
        OpdsLink {
            link_type: LinkType::Navigation,
            rel: Rel::Start,
            url: Cow::Borrowed("/"),
            count: None,
        },
    ];

    let entries = vec![
        OpdsEntry::new(
            format!("tag:{}:all", config.tag_authority),
            "All comics",
            "All comics as a flat list",
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Acquisition,
                rel: Rel::Subsection,
                url: Cow::Borrowed("/all"),
                count: None,
            }],
            Utc::now(),
        ),
        OpdsEntry::new(
            format!("tag:{}:recent", config.tag_authority),
            "Recent comics",
            "All comics sorted by recency",
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Acquisition,
                rel: Rel::SortNew,
                url: Cow::Borrowed("/recent"),
                count: None,
            }],
            Utc::now(),
        ),
        OpdsEntry::new(
            format!("tag:{}:publishers", config.tag_authority),
            "comics by publisher",
            "All comics sorted by publisher",
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Navigation,
                rel: Rel::SortNew,
                url: Cow::Borrowed("/publishers"),
                count: None,
            }],
            Utc::now(),
        ),
        OpdsEntry::new(
            format!("tag:{}:unread_all", config.tag_authority),
            "All unread comics",
            "All unread comics sorted by published date",
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Acquisition,
                rel: Rel::Subsection,
                url: Cow::Borrowed("/unread_all"),
                count: None,
            }],
            Utc::now(),
        ),
        OpdsEntry::new(
            format!("tag:{}:unread", config.tag_authority),
            "Unread comics by Series",
            "Unread comics by Series",
            Vec::new(),
            vec![OpdsLink {
                link_type: LinkType::Navigation,
                rel: Rel::Subsection,
                url: Cow::Borrowed("/unread"),
                count: None,
            }],
            Utc::now(),
        ),
    ];

    let feed = OpdsFeed {
        id,
        title: "Rust OPDS",
        updated: Utc::now(),
        links,
        entries,
    };
    write_opds(&feed)
}

fn make_entry<'a>(tag_authority: &str, entry: &'a ComicInfo) -> OpdsEntry<'a> {
    let id = format!("tag:{}:entry:{}", tag_authority, entry.id.unwrap_or(0));
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
            url: Cow::Owned(format!("/stream/{}/0/cover.jpg", entry.id.unwrap_or(0))),
            count: None,
        },
        OpdsLink {
            link_type: LinkType::Jpeg,
            rel: Rel::Thumbnail,
            url: Cow::Owned(format!("/stream/{}/0/cover.jpg", entry.id.unwrap_or(0))),
            count: None,
        },
        OpdsLink {
            link_type: LinkType::OctetStream,
            rel: Rel::Acquisition,
            url: Cow::Owned(format!("{}/download/{}", url_prefix, filename)),
            count: None,
        },
        OpdsLink {
            link_type: LinkType::Jpeg,
            rel: Rel::Stream,
            url: Cow::Owned(format!("/stream/{}/{{pageNumber}}", entry.id.unwrap_or(0))),
            count: entry.page_count,
        },
    ];

    let series = entry.series.as_ref().map_or("", |x| &**x);
    let title = entry.title.as_ref().map_or("", |x| &**x);
    let summary = entry.summary.as_ref().map_or("", |x| &**x);
    OpdsEntry {
        id,
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

fn write_links<W: Write>(writer: &mut EventWriter<W>, links: &[OpdsLink]) -> Result<(), Error> {
    lazy_static! {
        static ref TYPE_NAME: Name<'static> = Name::local("type");
        static ref REL_NAME: Name<'static> = Name::local("rel");
        static ref HREF_NAME: Name<'static> = Name::local("href");
        static ref COUNT_NAME: Name<'static> = Name::prefixed("count", "pse");
    }

    for link in links.iter() {
        let mut event = XmlEvent::start_element("link")
            .attr(*TYPE_NAME, link.link_type.as_str())
            .attr(*REL_NAME, link.rel.as_str())
            .attr(*HREF_NAME, &link.url);

        let count_str;

        let event = match link.count {
            Some(count) => {
                count_str = count.to_string();
                event.attr(*COUNT_NAME, &count_str)
            }
            None => event,
        };

        writer.write(event)?;
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
            .ns("opds", "http://opds-spec.org/2010/catalog")
            .ns("pse", "http://vaemendis.net/opds-pse/ns"),
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
