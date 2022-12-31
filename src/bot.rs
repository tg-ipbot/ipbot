use core::fmt::Debug;
use std::error;

use log::debug;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use teloxide::utils::command::BotCommands;

use crate::ApplicationCommand;
use crate::DbCommand;

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Generate application token")]
    Token,
    #[command(description = "Get my PC VPN IP")]
    GetMyIp,
    #[command(description = "Show help message")]
    Help,
}

pub(super) async fn setup_bot(
    bot: Bot,
    tx: tokio::sync::mpsc::Sender<ApplicationCommand<String>>,
) -> Result<(), &'static str> {
    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![tx])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

fn schema() -> teloxide::dispatching::UpdateHandler<Box<dyn error::Error + Send + Sync + 'static>> {
    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(dptree::case![Command::Token].endpoint(token::<String>))
        .branch(dptree::case![Command::GetMyIp].endpoint(get_my_ip::<String>))
        .branch(dptree::case![Command::Help].endpoint(help));

    Update::filter_message()
        .branch(command_handler)
        .branch(
            dptree::filter(|msg: Message| {
                if let Some(text) = msg.text() {
                    return text == "/start";
                }

                false
            })
            .endpoint(help),
        )
        .branch(
            dptree::filter(|msg: Message| {
                if let Some(text) = msg.text() {
                    return text.starts_with('/');
                }

                false
            })
            .endpoint(invalid_command),
        )
        .branch(dptree::endpoint(help))
}

async fn token<T: 'static + Sync + Send + Debug + AsRef<str>>(
    bot: Bot,
    transmitter: tokio::sync::mpsc::Sender<ApplicationCommand<T>>,
    msg: Message,
) -> Result<(), Box<dyn error::Error + Send + Sync>> {
    let (tx, rx) = tokio::sync::oneshot::channel::<T>();
    let user = msg.from().unwrap();

    debug!(
        "Message from user {} - locale {}",
        user.id,
        user.language_code.as_ref().unwrap()
    );

    transmitter
        .send(ApplicationCommand::new(
            DbCommand::TokenGenerateRequest(user.id),
            tx,
        ))
        .await?;
    let token = rx.await.unwrap();

    bot.send_message(msg.chat.id, format!("Your token is `{}`", token.as_ref()))
        .parse_mode(ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

async fn get_my_ip<T: 'static + Sync + Send + Debug + AsRef<str>>(
    bot: Bot,
    transmitter: tokio::sync::mpsc::Sender<ApplicationCommand<T>>,
    msg: Message,
) -> Result<(), Box<dyn error::Error + Send + Sync>> {
    let (tx, rx) = tokio::sync::oneshot::channel::<T>();
    let user = msg.from().unwrap();

    transmitter
        .send(ApplicationCommand::new(
            DbCommand::IpGetRequest(user.id),
            tx,
        ))
        .await?;

    let response = rx.await.unwrap();
    debug!("Get my IP response: {:?}", response);
    bot.send_message(msg.chat.id, response.as_ref())
        .parse_mode(ParseMode::MarkdownV2)
        .await?;

    Ok(())
}

async fn help(bot: Bot, msg: Message) -> Result<(), Box<dyn error::Error + Send + Sync>> {
    let help_str = format!(
        r"I can help you with tracking your PC VPN IP address, so you can
connect to it from another location if you are connected to the same VPN network.

{}",
        Command::descriptions()
    );

    bot.send_message(msg.chat.id, help_str).await?;

    Ok(())
}

async fn invalid_command(
    bot: Bot,
    msg: Message,
) -> Result<(), Box<dyn error::Error + Send + Sync>> {
    bot.send_message(msg.chat.id, "Unknown command. Would you repeat?")
        .await?;

    Ok(())
}
