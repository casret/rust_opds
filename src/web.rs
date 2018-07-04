use super::db::DB;
use super::opds;
use failure::Error;
use futures::{future, Future, Stream};
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use regex::Regex;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

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

// TODO: figure out Stream
fn serve_opds(req: &Request<Body>, db: &DB) -> ResponseFuture {
    lazy_static! {
        static ref COMIC_RE: Regex = Regex::new(r"/comic/(\d+)/").unwrap();
    }
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            let body = Body::from(opds::get_navigation_feed().unwrap());
            Box::new(future::ok(Response::new(body)))
        }
        (&Method::GET, "/all") => {
            let entries = db.get_all().unwrap();
            let body =
                Body::from(opds::make_acquisiton_feed("/all", "All Comics", &entries).unwrap());
            Box::new(future::ok(Response::new(body)))
        }
        (&Method::GET, path) if COMIC_RE.is_match(path) => {
            let id = COMIC_RE.captures(path).unwrap()[1].parse::<i64>().unwrap();
            let entry = db.get(id).unwrap();
            simple_file_send(&entry.filepath)
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
