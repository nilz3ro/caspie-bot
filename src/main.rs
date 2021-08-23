use lazy_static::*;
use log::info;
use reqwest;
use serde::{Deserialize, Serialize};
use status::BasicNodeStatus;
use std::sync::Mutex;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use teloxide::{prelude::*, utils::command::BotCommand};
use tokio::sync::{mpsc, oneshot};

lazy_static! {
    static ref BKCHAN: (
        mpsc::Sender<book_keeping::Message>,
        Mutex<mpsc::Receiver<book_keeping::Message>>
    ) = {
        let (tx, mut rx) = mpsc::channel(64);
        (tx, Mutex::new(rx))
    };
}

#[tokio::main]
async fn main() {
    teloxide::enable_logging!();
    info!("starting");

    let bot = Bot::from_env().auto_send();

    tokio::join!(
        bot::start_repl(bot.clone()),
        book_keeping::run_book_keeping(bot.clone()),
        status::get_status()
    );
}

type ResponseChannel<T> = Option<oneshot::Sender<anyhow::Result<T>>>;

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

    pub async fn start_repl(bot: AutoSend<Bot>) {
        info!("starting repl");
        teloxide::repl(bot, |message| async move {
            let t = message.update.text().unwrap();
            match BotCommand::parse(t, String::from("caspiebot")) {
                Ok(Command::Help) => {
                    info!("help called");
                }
                Ok(Command::Subscribe(s)) => {
                    info!("sub called");
                    BKCHAN
                        .0
                        .send(book_keeping::Message::Subscribe {
                            chat_id: message.chat_id(),
                            status_url: s,
                            response_tx: None,
                        })
                        .await
                        .unwrap();
                }
                Ok(Command::Unsubscribe(s)) => {
                    info!("unsub called");
                }
                Err(_) => {
                    info!("parse error.")
                }
            }
            respond(())
        })
        .await
    }

    pub async fn say_hi(b: AutoSend<Bot>) {
        b.send_message(String::from("@nilz3ro"), "sup dood.")
            .send()
            .await
            .expect("failed to send.");
        info!("sent message.");
    }
}

mod book_keeping {
    use super::*;

    #[derive(Debug)]
    pub enum Message {
        Subscribe {
            status_url: String,
            chat_id: i64,
            response_tx: ResponseChannel<()>,
        },
        Unsubscribe {
            status_url: String,
            chat_id: i64,
            response_tx: ResponseChannel<()>,
        },
        GetNodesList {
            response_tx: ResponseChannel<Vec<String>>,
        },
        NodeStatusUpdate {
            url: String,
            node_status: BasicNodeStatus,
        },
        // notify subscriber of node status change
    }

    pub struct BookKeeper {
        pub bot: AutoSend<Bot>,
        pub subscriptions: HashMap<String, HashSet<i64>>,
    }

    impl BookKeeper {
        pub fn new(bot: AutoSend<Bot>) -> Self {
            let subscriptions = HashMap::default();
            BookKeeper { bot, subscriptions }
        }
    }

    pub async fn run_book_keeping(bot: AutoSend<Bot>) {
        let mut book_keeper = BookKeeper::new(bot);

        while let Some(message) = BKCHAN.1.lock().expect("failed to lock rx").recv().await {
            match message {
                Message::Subscribe {
                    chat_id,
                    status_url,
                    ..
                } => {
                    info!("subscribing chatter.");
                    let subscribed_chatters = book_keeper
                        .subscriptions
                        .entry(status_url.clone())
                        .or_default();
                    if subscribed_chatters.contains(&chat_id) {
                        info!("chatter already subscribed.");
                        book_keeper
                            .bot
                            .send_message(
                                chat_id,
                                format!(
                                    "You are already subscribed to updates for {}",
                                    status_url.clone()
                                ),
                            )
                            .await
                            .expect("failed to send message");
                    } else {
                        info!("chatter being subscribed.");
                        subscribed_chatters.insert(chat_id);
                        book_keeper
                            .bot
                            .send_message(
                                chat_id,
                                format!("You are now subscribed to updates for {}", status_url),
                            )
                            .await
                            .expect("failed to send message");
                    }
                }
                Message::Unsubscribe {
                    chat_id,
                    status_url,
                    ..
                } => {
                    info!("handling unsubscribe");
                    let subscribed_chatters = book_keeper
                        .subscriptions
                        .entry(status_url.clone())
                        .or_default();
                    if subscribed_chatters.contains(&chat_id) {
                        info!("removing chatter from subscriptions.");
                        subscribed_chatters.remove(&chat_id);
                        book_keeper
                            .bot
                            .send_message(
                                chat_id,
                                format!("You will no longer receive updates for {}", status_url),
                            )
                            .await
                            .expect("failed to send message");
                    } else {
                        book_keeper
                            .bot
                            .send_message(
                                chat_id,
                                format!("You are not subscribed to updates for {}", status_url),
                            )
                            .await
                            .expect("failed to send message");
                    }
                }
                Message::GetNodesList { response_tx } => {
                    let nodes_list: Vec<String> = book_keeper
                        .subscriptions
                        .keys()
                        .map(|s| String::from(s))
                        .collect();
                    response_tx
                        .unwrap()
                        .send(Ok(nodes_list))
                        .expect("failed to send nodes list");
                }
                Message::NodeStatusUpdate { url, node_status } => {}
            }
        }
    }
}

mod status {
    use super::*;
    use book_keeping::Message;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct BasicNodeStatus {
        api_version: String,
        chainspec_name: String,
        our_public_signing_key: String,
    }

    pub async fn get_status() {
        loop {
            info!("requesting addresses");

            let (tx, rx) = oneshot::channel();

            BKCHAN
                .0
                .send(Message::GetNodesList {
                    response_tx: tx.into(),
                })
                .await
                .unwrap();

            let node_urls = rx.await.unwrap().unwrap();

            info!("got node urls! {:?}", node_urls);

            let response = reqwest::get("http://example.com")
                .await
                .expect("failed to fetch status.")
                .json::<BasicNodeStatus>()
                .await;

            match response {
                Ok(response) => {
                    info!("got response from node");
                }
                Err(e) => {
                    info!("failed to parse response as json.");
                }
            }

            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}
