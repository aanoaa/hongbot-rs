use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::Result;

// addr: 127.0.0.1:8080
pub fn serve(addr: &str, running: Arc<Mutex<bool>>) -> Result<JoinHandle<()>> {
    // serve http
    let listener = TcpListener::bind(addr).unwrap();
    log::info!("Listening for connections on http://{}", addr);
    listener
        .set_nonblocking(true)
        .expect("cannot set non-blocking");
    Ok(thread::spawn(move || {
        let dur = Duration::from_millis(100);
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    handle_client(stream);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if *running.lock().unwrap() {
                        thread::sleep(dur);
                        continue;
                    } else {
                        drop(listener);
                        break;
                    }
                }
                Err(e) => {
                    log::error!("http connect fail: {}", e);
                }
            }
        }
    }))
}

fn handle_client(stream: TcpStream) {
    handle_read(&stream);
    handle_write(stream);
}

fn handle_read(mut stream: &TcpStream) {
    let mut buf = [0u8; 4096];
    match stream.read(&mut buf) {
        Ok(_) => {
            let req = String::from_utf8_lossy(&buf);
            log::trace!("{}", req);
        }
        Err(e) => {
            log::error!("http read fail: {e}")
        }
    }
}

fn handle_write(mut stream: TcpStream) {
    const CRLF: &str = "\n\r";
    let resp =
        format!("HTTP/1.1 200 OK{CRLF}Content-Type: text/plain; charset=UTF-8{CRLF}{CRLF}OK");
    match stream.write(resp.as_bytes()) {
        Ok(_) => (),
        Err(e) => {
            log::error!("http resp fail: {e}")
        }
    }
}
