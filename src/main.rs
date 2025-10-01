mod binance;
mod config;
mod error;
mod redis;
mod state;
mod telegram;

use std::sync::Arc;

use rust_decimal::Decimal;
use teloxide::dispatching::UpdateHandler;
use teloxide::dispatching::dialogue::{self, GetChatId, InMemStorage};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use teloxide::utils::command::BotCommands;

use crate::config::ServiceConfig;
use crate::state::{AppState, periodic_exchange_info_update};
use crate::telegram::format_message;

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    ReceiveToken,
    ReceiveFilters {
        token: String,
    },
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    Help,
    Start,
    Cancel,
}

fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(case![State::Start].branch(case![Command::Start].endpoint(start)))
        .branch(case![Command::Help].endpoint(help))
        .branch(case![Command::Cancel].endpoint(cancel));

    let message_handler = Update::filter_message()
        .branch(command_handler)
        .branch(case![State::Start].endpoint(start))
        .branch(case![State::ReceiveToken].endpoint(receive_token))
        .branch(dptree::endpoint(invalid_state));

    let callback_query_handler = Update::filter_callback_query()
        .branch(case![State::ReceiveFilters { token }].endpoint(perform));

    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(message_handler)
        .branch(callback_query_handler)
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let config = ServiceConfig::read_from_file().expect("Failed to read config");

    let bot = Bot::new(config.telegram_token);
    let app_state = Arc::new(AppState::new(config.redis_url, config.allowed_users));

    let exch_info_update_handler = tokio::spawn(periodic_exchange_info_update(app_state.clone()));

    let dispatcher_handler = tokio::spawn(async move {
        Dispatcher::builder(bot, schema())
            .dependencies(dptree::deps![
                InMemStorage::<State>::new(),
                app_state.clone()
            ])
            .build()
            .dispatch()
            .await;
    });

    if let Err(e) = tokio::try_join!(exch_info_update_handler, dispatcher_handler) {
        log::error!("Something went wrong: {:?}", e);
    }
}

async fn start(bot: Bot, dialogue: MyDialogue, msg: Message, app_state: Arc<AppState>) -> HandlerResult {
    match app_state.authorize(msg.chat.id).await {
        Ok(_) => {
            bot.send_message(msg.chat.id, "Enter Binance spot token").await?;
            dialogue.update(State::ReceiveToken).await?
        },
        Err(e) => {
            bot.send_message(msg.chat.id, e.to_string()).await?;
            dialogue.exit().await?
        }
    }

    Ok(())
}

async fn help(bot: Bot, msg: Message, app_state: Arc<AppState>) -> HandlerResult {
    let message = match app_state.authorize(msg.chat.id).await {
        Ok(_) => Command::descriptions().to_string(),
        Err(e) => e.to_string(),
    };

    bot.send_message(msg.chat.id, message).await?;
    Ok(())
}

async fn cancel(bot: Bot, dialogue: MyDialogue, msg: Message, app_state: Arc<AppState>) -> HandlerResult {
    let message = match app_state.authorize(msg.chat.id).await {
        Ok(_) => "Cancelled. Enter /start to check order book",
        Err(e) => &e.to_string(),
    };

    bot.send_message(msg.chat.id, message).await?;
    dialogue.exit().await?;
    Ok(())
}

async fn invalid_state(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(
        msg.chat.id,
        "Unable to handle the message. Type /help to see the usage.",
    )
    .await?;
    Ok(())
}

async fn receive_token(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    app_state: Arc<AppState>,
) -> HandlerResult {
    match msg.text() {
        Some(token) => match app_state.validate_symbol(token).await {
            Ok(validated) => {
                let options = ["3%", "5%", "8%", "10%", "15%"]
                    .map(|product| InlineKeyboardButton::callback(product, product));

                bot.send_message(msg.chat.id, format!("{} ✅\nChoose depth", validated))
                    .reply_markup(InlineKeyboardMarkup::new([options]))
                    .await?;
                dialogue
                    .update(State::ReceiveFilters { token: validated })
                    .await?;
            }
            Err(e) => {
                let err_msg = format!("Try again. {} ❌", e);
                bot.send_message(msg.chat.id, err_msg).await?;
                dialogue.update(State::ReceiveToken).await?
            }
        },
        None => {
            bot.send_message(msg.chat.id, "Send me plain text.").await?;
        }
    }

    Ok(())
}

async fn perform(
    bot: Bot,
    dialogue: MyDialogue,
    token: String,
    query: CallbackQuery,
    app_state: Arc<AppState>,
) -> HandlerResult {
    let parsed_query = query.clone().data.map(|mut depth| {
        depth.pop();
        depth.parse::<Decimal>()
    });

    match parsed_query {
        Some(Ok(depth)) => {
            let order_book = app_state
                .get_filtered_order_book(token.clone(), depth)
                .await;

            let msg = match order_book {
                Ok(order_book) => format_message(order_book),
                Err(e) => {
                    log::error!("Error while requesting order book for {}: {}", token, e);
                    "Something went wrong. Try again later".to_string()
                }
            };

            bot.send_message(query.chat_id().unwrap(), msg)
                .parse_mode(ParseMode::MarkdownV2)
                .await?;

            dialogue.update(State::ReceiveToken).await.unwrap()
        }
        _ => {
            bot.send_message(query.chat_id().unwrap(), "Send me plain text.")
                .await?;
        }
    }

    Ok(())
}
