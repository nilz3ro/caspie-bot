use super::*;
use book_keeping::Message;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeStatusDetails {
    api_version: String,
    chainspec_name: String,
    our_public_signing_key: String,
}

#[derive(Debug, Clone)]
pub enum NodeStatus {
    Online(NodeStatusDetails),
    Offline,
}

impl Default for NodeStatus {
    fn default() -> NodeStatus {
        NodeStatus::Offline
    }
}

#[derive(Debug, Default)]
pub struct NodeSubscription {
    pub subscribed_chatters: Vec<i64>,
    current_status: NodeStatus,
    status_history: Vec<NodeStatus>,
}

impl NodeSubscription {
    pub fn new() -> NodeSubscription {
        NodeSubscription {
            subscribed_chatters: vec![],
            current_status: NodeStatus::Offline,
            status_history: vec![],
        }
    }
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
