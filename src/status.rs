use super::*;
use book_keeping::Message;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct NodeStatusDetails {
    api_version: String,
    chainspec_name: String,
    our_public_signing_key: String,
}

#[derive(Debug, Clone, PartialEq)]
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
    previous_status: NodeStatus,
    status_history: Vec<NodeStatus>,
}

impl NodeSubscription {
    pub fn new() -> NodeSubscription {
        NodeSubscription {
            subscribed_chatters: vec![],
            current_status: NodeStatus::Offline,
            previous_status: NodeStatus::Offline,
            status_history: vec![],
        }
    }

    /// Adds the latest status to self.status_history,
    /// Returns a tuple (u8, u8) representing the number of online and offline
    /// statuses respectively: (online_count, offline_count)
    pub fn add_latest_status(&mut self, latest: NodeStatus) {
        self.status_history.push(latest);
        if self.status_history.len() > 5 {
            self.status_history.remove(0);
        }
    }

    #[inline]
    fn count_status_history(&mut self) -> (u8, u8) {
        self.status_history
            .iter()
            .fold((0_u8, 0_u8), |(online_count, offline_count), s| match s {
                NodeStatus::Online(_) => (online_count + 1, offline_count),
                NodeStatus::Offline => (online_count, offline_count + 1),
            })
    }

    pub fn status_changed(&mut self) -> bool {
        let len = self.status_history.len();
        let latest_status = self.status_history[len - 1].clone();
        let (online_count, offline_count) = self.count_status_history();
        // (5, 0) -> node has been online for 5 checks
        //    if self.current_status matches NodeStatus::Offline
        //      self.previous_status = self.current_status.clone()
        //      self.current_status = latest online status.
        //    if self.current_status matches NodeStatus::Online return false.
        //

        match (
            online_count,
            offline_count,
            latest_status.clone(),
            self.current_status.clone(),
        ) {
            (5, 0, NodeStatus::Online(_), NodeStatus::Online(_)) => false,
            (5, 0, NodeStatus::Online(_), NodeStatus::Offline) => {
                self.previous_status = self.current_status.clone();
                self.current_status = latest_status;
                true
            }
            (0, 5, NodeStatus::Offline, NodeStatus::Offline) => false,
            (0, 5, NodeStatus::Offline, NodeStatus::Online(_)) => {
                self.previous_status = self.current_status.clone();
                self.current_status = latest_status;
                true
            }
            _ => false,
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
