use std::{
    io::Cursor,
    sync::Arc,
    thread::{self, JoinHandle},
};

use anyhow::Result;
use tiny_http::{Request, Response, Server, StatusCode};

pub fn serve(addr: &str) -> Result<(Arc<Server>, Vec<JoinHandle<()>>)> {
    let server = Arc::new(Server::http(addr).unwrap());
    log::info!("Listening for connections on http://{}", addr);
    const MAX_WORKERS: usize = 4;
    let mut guards = Vec::with_capacity(MAX_WORKERS);
    for _ in 0..MAX_WORKERS {
        let server = server.clone();
        let guard = thread::spawn(move || {
            for req in server.incoming_requests() {
                let path = req.url();
                let method = req.method();
                let resp = match method {
                    tiny_http::Method::Get => match path {
                        "/" => index(&req),
                        _ => error_resp(404),
                    },
                    _ => error_resp(405),
                };
                req.respond(resp).unwrap();
            }
        });
        guards.push(guard);
    }

    Ok((server, guards))
}

fn index(req: &Request) -> Response<Cursor<Vec<u8>>> {
    log::trace!("{} {}", req.method(), req.url());
    // do something
    Response::from_string("OK")
}

fn error_resp(code: u16) -> Response<Cursor<Vec<u8>>> {
    let code = StatusCode(code);
    Response::from_string(code.default_reason_phrase()).with_status_code(code)
}
