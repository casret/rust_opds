use super::db::DB;
use super::opds;
use failure::Error;
use futures::{future, Future, Stream};
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::net::SocketAddr;
use std::sync::Arc;

// TODO: figure out Stream
fn serve_opds(
    req: Request<Body>,
    db: &DB,
) -> Box<Future<Item = Response<Body>, Error = ::hyper::Error> + Send> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            let body = Body::from(opds::get_navigation_feed().unwrap());
            Box::new(future::ok(Response::new(body)))
        }
        _ => {
            let body = Body::from("Not Found");
            Box::new(future::ok(
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(body)
                    .unwrap(),
            ))
        }
    }
}

pub fn start_web_service(db: Arc<DB>, addr: SocketAddr) -> Result<(), Error> {
    let new_svc = move || {
        let db = db.clone();
        service_fn(move |req| serve_opds(req, &db))
    };

    let server = Server::bind(&addr)
        .serve(new_svc)
        .map_err(|e| eprintln!("server error: {}", e));

    // Run this server for... forever!
    ::hyper::rt::run(server);
    Ok(())
}
