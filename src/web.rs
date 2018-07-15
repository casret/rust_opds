use super::db::DB;
use super::opds;
use failure::Error;
use futures::{future, Future};
use hyper::header;
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use regex::Regex;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use url::percent_encoding::percent_decode;

type ResponseFuture = Box<Future<Item = Response<Body>, Error = io::Error> + Send>;

static NOTFOUND: &[u8] = b"Not Found";

fn not_found() -> ResponseFuture {
    Box::new(future::ok(
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(NOTFOUND.into())
            .unwrap(),
    ))
}

fn unauthorized() -> ResponseFuture {
    Box::new(future::ok(
        Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header(header::WWW_AUTHENTICATE, r#"Basic realm="Rust OPDS""#)
            .body("Please provide username and password.".into())
            .unwrap(),
    ))
}

fn parse_auth_header(auth: &str) -> Option<(String, String)> {
    use base64::decode;
    lazy_static! {
        static ref AUTH_RE: Regex = Regex::new(r"Basic (.*)$").unwrap();
    }
    if let Some(caps) = AUTH_RE.captures(auth) {
        let auth = caps.get(1).unwrap().as_str();
        if let Ok(auth) = decode(auth) {
            if let Ok(auth) = String::from_utf8(auth) {
                let parts: Vec<&str> = auth.splitn(2, ':').collect();
                return Some((parts[0].to_owned(), parts[1].to_owned()));
            }
        }
    }
    None
}

// TODO: figure out Stream
fn serve_opds(req: &Request<Body>, db: &DB) -> ResponseFuture {
    debug!("Handling request {:#?}", req);
    let user_id = match req.headers().get(header::AUTHORIZATION) {
        None => {
            return unauthorized();
        }
        Some(auth) => {
            if let Some((username, password)) = parse_auth_header(auth.to_str().unwrap_or_default())
            {
                match db.check_or_provision_user(&username, &password) {
                    Ok(0) => return unauthorized(),
                    Ok(user_id) => user_id,
                    _ => return unauthorized(),
                }
            } else {
                return unauthorized();
            }
        }
    };

    // Why doesn't hyper do this for me?
    let path = &percent_decode(req.uri().path().as_bytes()).decode_utf8_lossy();
    let mut path_parts = path.split('/');
    path_parts.next(); // first part is always empty

    match (req.method(), path_parts.next()) {
        (&Method::GET, None) | (&Method::GET, Some("")) => {
            let body = Body::from(opds::make_navigation_feed().unwrap());
            Box::new(future::ok(Response::new(body)))
        }
        (&Method::GET, Some("all")) => {
            let entries = db.get_all().unwrap();
            let body =
                Body::from(opds::make_acquisition_feed("/all", "All Comics", &entries).unwrap());
            Box::new(future::ok(Response::new(body)))
        }
        (&Method::GET, Some("recent")) => {
            let entries = db.get_recent().unwrap();
            let body = Body::from(
                opds::make_acquisition_feed("/recent", "Recent Comics", &entries).unwrap(),
            );
            Box::new(future::ok(Response::new(body)))
        }
        (&Method::GET, Some("publishers")) => {
            let body = match path_parts.next() {
                Some(publisher) => match path_parts.next() {
                    Some(series) => {
                        let entries = db.get_for_publisher_series(&publisher, &series).unwrap();
                        Body::from(opds::make_acquisition_feed(path, series, &entries).unwrap())
                    },
                    None => {
                        let mut entries = db.get_series_for_publisher(&publisher).unwrap();
                        Body::from(opds::make_subsection_feed(path, publisher, &mut entries).unwrap())
                    },
                },
                None => {
                    let mut entries = db.get_publishers().unwrap();
                    Body::from(
                        opds::make_subsection_feed("/publishers", "Comics by publisher", &mut entries)
                        .unwrap(),
                        )
                }
            };
            Box::new(future::ok(Response::new(body)))
        }
        (&Method::GET, Some("unread")) => {
            let body = match path_parts.next() {
                Some(series) => {
                    let mut entries = db.get_unread_for_series(user_id, &series).unwrap();
                    Body::from(opds::make_acquisition_feed(path, series, &entries).unwrap())
                }
                None => {
                    let mut entries = db.get_unread_series(user_id).unwrap();
                    Body::from(
                        opds::make_subsection_feed("/unread", "Unread comics by series", &mut entries)
                    .unwrap(),
                    )
                }
            };
            Box::new(future::ok(Response::new(body)))
        }
        (&Method::GET, Some("unread_all")) => {
            let entries = db.get_unread(user_id).unwrap();
            let body = Body::from(
                opds::make_acquisition_feed("/unread", "Unread Comics", &entries).unwrap(),
            );
            Box::new(future::ok(Response::new(body)))
        }
        (&Method::GET, Some("comic")) => {
            match path_parts.next() {
                Some(id) => {
                    let id = id.parse::<i64>().unwrap();
                    let entry = db.get(id).unwrap();
                    db.mark_read(id, user_id).unwrap();
                    simple_file_send(&entry.filepath)
                },
                _ => not_found()
            }
        }
        (&Method::GET, Some("stream")) => {
            match path_parts.next() {
                Some(issue_id) => match path_parts.next() {
                    Some(page_id) => {
                        let issue_id = issue_id.parse::<i64>().unwrap();
                        let page_id = page_id.parse::<i32>().unwrap();
                        let body = Body::from(db.get_page(issue_id, page_id, user_id).unwrap());
                        Box::new(future::ok(Response::new(body)))
                    }
                    _ => not_found()
                },
                _ => not_found()
            }
        }
        _ => not_found(),
    }
}

pub fn start_web_service(db: Arc<DB>, addr: SocketAddr) -> Result<(), Error> {
    let new_svc = move || {
        let db = db.clone();
        service_fn(move |req| serve_opds(&req, &db))
    };

    let server = Server::bind(&addr)
        .serve(new_svc)
        .map_err(|e| eprintln!("server error: {}", e));

    info!("Starting server on {}", addr);
    // Run this server for... forever!
    ::hyper::rt::run(server);
    Ok(())
}

// Taken from the hyper example
fn simple_file_send(f: &str) -> ResponseFuture {
    // Serve a file by asynchronously reading it entirely into memory.
    // Uses tokio_fs to open file asynchronously, then tokio_io to read into
    // memory asynchronously.
    let filename = f.to_string(); // we need to copy for lifetime issues
    Box::new(
        ::tokio_fs::file::File::open(filename)
            .and_then(|file| {
                let buf: Vec<u8> = Vec::new();
                ::tokio_io::io::read_to_end(file, buf)
                    .and_then(|item| Ok(Response::new(item.1.into())))
                    .or_else(|_| {
                        Ok(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Body::empty())
                            .unwrap())
                    })
            })
            .or_else(|_| not_found()),
    )
}
