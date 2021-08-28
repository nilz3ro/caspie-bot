
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
