/*
 * bot
 * - run
 * - listen?
 * - shutdown
 * - onmessage
 *   - send
 *   - reply
 *   - hear
 *   - respond
 * - onconnect
 * - onclose
 *
 * server
 * - connect
 * - close
 * - send
 * - onmessage
 *
 *
 * configuration 으로 server connect 해서 bot 에 전달
 */

use std::{
    sync::{mpsc::channel, Arc, RwLock},
    thread,
};

use hongbot_rs::server::{Server, Shell};

fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info,hongbot=trace"));
    let (tx, rx) = channel::<String>();

    // shell 안의 특정 property 가 shared 이면 됨

    let shutdown = Arc::new(RwLock::new(false));
    let s1 = shutdown.clone();
    let handle = thread::spawn(move || {
        let mut shell = Shell::new(tx, s1);
        shell.connect();
    });

    loop {
        let message = rx.recv().unwrap();
        if has_quit(&message) {
            match shutdown.try_write() {
                Ok(mut r) => {
                    *r = true;
                }
                Err(e) => log::error!("write lock fail: {e}"),
            }
            break;
        }
    }

    match handle.join() {
        Ok(()) => (),
        Err(e) => {
            log::error!("thread join fail: {:?}", e);
        }
    }

    log::trace!("done");
}

/// exit, quit 를 판단
fn has_quit(s: &str) -> bool {
    matches!(s, "exit" | "quit" | "bye")
}
