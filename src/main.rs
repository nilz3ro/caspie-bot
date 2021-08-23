use lazy_static::*;
use log::info;
use reqwest;
use serde::{Deserialize, Serialize};
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

    const HELP_TEXT: &'static str = r#"
Hello! I'm Caspie Bot, here is a list of commands I respond to:

/subscribe {url} - Get notifications for the node at {url} when it goes offline or comes back online. 
/unsubscribe {url} - Stop getting notifications for the node at {url}.
/help - display this help text.

Note: {url} must be the full url of the status endpoint for a node: `http://mynode.mysite.com:8888/status`.
"#;

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
                    info!("bot help command called.");
                    // send help message.
                }
                Ok(Command::Subscribe(s)) => {
                    info!("bot sub command called.");
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
                    info!("bot unsubscribe command called.");
                    BKCHAN
                        .0
                        .send(book_keeping::Message::Unsubscribe {
                            chat_id: message.chat_id(),
                            status_url: s,
                            response_tx: None,
                        })
                        .await
                        .unwrap();
                }
                Err(_) => {
                    info!("parse error.")
                    // send help text.
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

    pub async fn send_help_text(bot: AutoSend<Bot>, chat_id: i64) {
        bot.send_message(chat_id, HELP_TEXT)
            .await
            .expect("failed to send help text.");
    }
}

mod book_keeping {
    use super::*;
    use status::{NodeStatus, NodeStatusDetails};

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
            status_url: String,
            node_status: NodeStatus,
        },
        // notify subscriber of node status change
    }

    pub struct BookKeeper {
        pub bot: AutoSend<Bot>,
        pub subscriptions: HashMap<String, (HashSet<i64>, Vec<NodeStatus>)>,
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
                    let (subscribed_chatters, _history) = book_keeper
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
                    let (subscribed_chatters, _history) = book_keeper
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
                Message::NodeStatusUpdate {
                    status_url,
                    node_status,
                } => {}
            }
        }
    }
}

mod status {
    use super::*;
    use book_keeping::Message;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct NodeStatusDetails {
        api_version: String,
        chainspec_name: String,
        our_public_signing_key: String,
    }

    #[derive(Debug)]
    pub enum NodeStatus {
        Online(NodeStatusDetails),
        Offline,
    }

    pub async fn get_status() {
        let client = reqwest::Client::new();

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

            for url in node_urls {
                let client = client.clone();
                tokio::spawn(async move {
                    let response = client.get(url.clone()).send().await;

                    // Map to Some valid & parsed online status or None.
                    let response = match response {
                        Ok(res) => match res.json::<NodeStatusDetails>().await {
                            Ok(parsed) => Some(parsed),
                            Err(_) => None,
                        },
                        Err(_) => None,
                    };

                    let status = match response {
                        Some(details) => NodeStatus::Online(details),
                        None => NodeStatus::Offline,
                    };

                    BKCHAN
                        .0
                        .send(Message::NodeStatusUpdate {
                            status_url: url,
                            node_status: status,
                        })
                        .await
                        .expect("failed to send status update.");
                });
            }

            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}
