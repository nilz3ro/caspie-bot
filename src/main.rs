use log::info;
use reqwest;
use serde::{Deserialize, Serialize};
use status::BasicNodeStatus;
use std::{collections::HashMap, time::Duration};
use teloxide::{prelude::*, utils::command::BotCommand};
use tokio::sync::{mpsc, oneshot};

#[tokio::main]
async fn main() {
    teloxide::enable_logging!();
    info!("starting...");

    let bot = Bot::from_env().auto_send();
    let (tx, mut rx) = mpsc::channel(64);

    tokio::join!(
        book_keeping::run_book_keeping(bot.clone(), &mut rx),
        bot::start_repl(bot.clone(), tx)
    );
}

pub enum Message {
    Subscribe {
        status_url: String,
        chat_id: i64,
        response_tx: oneshot::Sender<anyhow::Result<()>>,
    },
    Unsubscribe {
        status_url: String,
        chat_id: i64,
        response_tx: oneshot::Sender<anyhow::Result<()>>,
    },
    // get latest list of node status urls
    // got node status from status endpoint.
    // notify subscriber of node status change
}

mod bot {
    use super::*;

    #[derive(BotCommand)]
    #[command(rename = "lowercase", description = "These commands are supported:")]
    pub enum Command {
        #[command(description = "display this text.")]
        Help,
        #[command(description = "handle a username.")]
        Subscribe(String),
        #[command(description = "Unsubscribe.")]
        Unsubscribe(String),
    }

    pub async fn start_repl(bot: AutoSend<Bot>, bktx: mpsc::Sender<Message>) {
        info!("starting repl...");
        teloxide::repl(bot, |message| async move {
            let t = message.update.text().unwrap();
            match BotCommand::parse(t, String::from("caspiebot")) {
                Ok(Command::Help) => {
                    info!("help called");
                }
                Ok(Command::Subscribe(s)) => {
                    info!("sub called");
                    let (tx, rx) = oneshot::channel();
                    bktx.send(Message::Subscribe {
                        chat_id: message.chat_id(),
                        status_url: s,
                        response_tx: tx,
                    });
                }
                Ok(Command::Unsubscribe(s)) => {
                    info!("unsub called");
                }
                Err(_) => todo!(),
            }
            respond(())
        })
        .await
    }

    pub async fn say_hi(b: AutoSend<Bot>) {
        b.send_message(String::from("1738759164"), "sup dood.")
            .send()
            .await
            .expect("failed to send.");
        info!("sent message.");
    }
}

mod book_keeping {
    use super::*;

    pub struct BookKeeper {
        bot: AutoSend<Bot>,
        subscriptions: HashMap<String, i64>,
    }

    impl BookKeeper {
        pub fn new(bot: AutoSend<Bot>) -> Self {
            let subscriptions = HashMap::default();
            BookKeeper { bot, subscriptions }
        }

        pub fn bot(&self) -> &AutoSend<Bot> {
            &self.bot
        }

        pub fn subscriptions(&self) -> &HashMap<String, i64> {
            &self.subscriptions
        }
    }

    pub async fn run_book_keeping(bot: AutoSend<Bot>, bkrx: &mut mpsc::Receiver<Message>) {
        let book_keeper = BookKeeper::new(bot);

        while let Some(message) = bkrx.recv().await {
            match message {
                Message::Subscribe {
                    chat_id,
                    status_url,
                    ..
                } => {
                    info!("got a request to sub {} from {}", chat_id, status_url);
                }
                Message::Unsubscribe {
                    chat_id,
                    status_url,
                    ..
                } => {
                    info!("got a request to unsub {} from {}", chat_id, status_url);
                }
            }
        }
    }
}

mod status {
    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct BasicNodeStatus {
        api_version: String,
        chainspec_name: String,
        our_public_signing_key: String,
    }

    pub async fn get_status(bktx: mpsc::Sender<Message>) {
        info!("getting status...");

        let response = reqwest::get("http://sathq.know.systems:8888/status")
            .await
            .expect("failed to fetch status.")
            .json::<BasicNodeStatus>()
            .await
            .expect("failed to parse as json.");

        info!("got response: {:?}", response);
    }
}
