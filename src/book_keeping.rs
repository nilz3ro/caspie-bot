use super::*;
use status::{NodeStatus, NodeStatusDetails, NodeSubscription};

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
    pub subscriptions: HashMap<String, NodeSubscription>,
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
                let node_sub = book_keeper
                    .subscriptions
                    .entry(status_url.clone())
                    .or_default();

                if node_sub.subscribed_chatters.contains(&chat_id) {
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
                    node_sub.subscribed_chatters.push(chat_id);
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
                let node_sub = book_keeper
                    .subscriptions
                    .entry(status_url.clone())
                    .or_default();

                if node_sub.subscribed_chatters.contains(&chat_id) {
                    info!("removing chatter from subscriptions.");
                    let chatter_to_remove = node_sub
                        .subscribed_chatters
                        .iter()
                        .position(|n| n == &chat_id)
                        .unwrap();
                    node_sub.subscribed_chatters.remove(chatter_to_remove);
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

                if node_sub.subscribed_chatters.len() == 0 {
                    book_keeper.subscriptions.remove(&status_url);
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
            } => {
                info!("status url: {}, status: {:?}", status_url, node_status);
                let node_sub = book_keeper
                    .subscriptions
                    .get_mut(&status_url)
                    .expect("Failed to get node subscription.");

                node_sub.add_latest_status(node_status);
                if node_sub.status_changed() {
                    info!("\n\n\nSTATUS CHANGED\n\n\n")
                }
            }
        }
    }
}
