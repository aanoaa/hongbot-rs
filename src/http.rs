use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

use anyhow::Result;
use tiny_http::{Response, Server};

pub fn serve(addr: &str) -> Result<(Arc<Server>, Vec<JoinHandle<()>>)> {
    let server = Arc::new(Server::http(addr).unwrap());
    log::info!("Listening for connections on http://{}", addr);
    const MAX_WORKERS: usize = 4;
    let mut guards = Vec::with_capacity(MAX_WORKERS);
    for _ in 0..MAX_WORKERS {
        let server = server.clone();
        let guard = thread::spawn(move || {
            for req in server.incoming_requests() {
                let resp = Response::from_string("OK");
                req.respond(resp).unwrap();
            }
        });
        guards.push(guard);
    }

    Ok((server, guards))
}
