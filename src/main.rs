use caspie_bot::prelude::*;
use teloxide;

#[tokio::main]
async fn main() {
    teloxide::enable_logging!();
    log::info!("starting");

    let bot = Bot::from_env().auto_send();

    tokio::join!(
        start_repl(bot.clone()),
        run_book_keeping(bot.clone()),
        get_status()
    );
}
