mod book_keeping;
mod bot;
mod status;

pub mod prelude {
    pub use super::book_keeping::{run_book_keeping, Message};
    pub use super::bot::start_repl;
    pub use super::status::get_status;
    pub use lazy_static::*;
    pub use log::info;
    pub use reqwest;
    pub use serde::{Deserialize, Serialize};
    pub use std::sync::Mutex;
    pub use std::{
        collections::{HashMap, HashSet},
        time::Duration,
    };
    pub use teloxide::{prelude::*, utils::command::BotCommand};
    pub use tokio::sync::{mpsc, oneshot};

    lazy_static! {
        pub static ref BKCHAN: (mpsc::Sender<Message>, Mutex<mpsc::Receiver<Message>>) = {
            let (tx, mut rx) = mpsc::channel(64);
            (tx, Mutex::new(rx))
        };
    }
}

pub use prelude::*;

type ResponseChannel<T> = Option<oneshot::Sender<anyhow::Result<T>>>;
