use core::fmt::Debug;
use std::time;

use blake2::{digest::consts::U16, Digest};
use rand::prelude::*;
use redis::{AsyncCommands, RedisResult};
use teloxide::prelude::UserId;

use log::{debug, error};
use redis::aio::Connection;

use super::ApplicationCommand;

#[derive(Debug)]
pub(crate) enum DbCommand {
    TokenGenerateRequest(UserId),
    IpGetRequest(UserId),
}

const USER_ID_KEY: &str = "user:id";

pub(crate) async fn db_task<T: Sync + Send + Debug + From<&'static str> + From<String>>(
    mut receiver: tokio::sync::mpsc::Receiver<ApplicationCommand<T>>,
) -> Result<(), &'static str> {
    debug!("socket: {}", env!("REDIS_SOCKET"));
    let client = match redis::Client::open(env!("REDIS_SOCKET")) {
        Ok(client) => client,
        Err(ref db_err) => {
            error!("DB open error: {}", db_err.category());
            return Err("Failed to create DB client");
        }
    };
    let mut con = match client.get_async_connection().await {
        Ok(con) => con,
        Err(_) => return Err("Failed to create redis connection"),
    };

    check_user_id(&mut con).await?;

    loop {
        if let Some(command) = receiver.recv().await {
            debug!("Receive command: {:?}", command.db_cmd);

            match command.db_cmd {
                DbCommand::TokenGenerateRequest(id) => {
                    process_token_request(&mut con, command, id).await
                }
                DbCommand::IpGetRequest(id) => {
                    let result: Option<String> = con.get(format!("user:{}:ip", id.0)).await.ok();

                    let response = match result {
                        Some(value) => format!("{}", value),
                        None => "No reported IP addresses for you".to_string(),
                    };

                    if let Err(_) = command.tx_channel.send(response.into()) {
                        error!("Sending to channel failed");
                    }
                }
            }
        }
    }
}

async fn check_user_id(con: &mut Connection) -> Result<(), &'static str> {
    let result: Result<(), &'static str> = match con.exists(USER_ID_KEY).await {
        Ok(true) => {
            debug!("user:id key exists");
            Ok(())
        }
        Ok(false) => {
            let result: RedisResult<()> = con
                .set(USER_ID_KEY, thread_rng().gen_range::<u32, _>(1000..10000))
                .await;

            if let Some(err) = result.err() {
                error!("Failed to set {} - {}", USER_ID_KEY, err.category());
                Err("Failed to set user ID")
            } else {
                Ok(())
            }
        }
        Err(_) => Err("Failed to execute EXISTS command"),
    };

    result
}

async fn process_token_request<T: Sync + Send + Debug + From<String>>(
    con: &mut Connection,
    command: ApplicationCommand<T>,
    id: UserId,
) {
    let user_key = format!("user:{}", id.0);

    let app_iter: Vec<String> = con.smembers(user_key.as_str()).await.unwrap();
    let mut app_id: Option<u32> = None;

    let app_key = if let Some(app) = app_iter.iter().next() {
        debug!("Found application registered: {}", app);
        app.clone()
    } else {
        app_id = Some(con.get(USER_ID_KEY).await.unwrap());
        format!("app:{}", app_id.unwrap())
    };

    let user_token: String = match con.hget(app_key.as_str(), "token").await.ok() {
        Some(token) => token,
        None => {
            let token = generate_token(app_id.unwrap(), &id);

            if let Err(e) = redis::pipe()
                .atomic()
                .hset(app_key.as_str(), "token", &token)
                .ignore()
                .hset(app_key.as_str(), "id", id.0)
                .ignore()
                .cmd("INCR")
                .arg(USER_ID_KEY)
                .ignore()
                .sadd(user_key.as_str(), app_key.as_str())
                .ignore()
                .query_async::<_, ()>(con)
                .await
            {
                error!("Failed to make a transaction: {}", e.category());
            }

            token
        }
    };

    if let Err(_) = command.tx_channel.send(user_token.into()) {
        error!("Sending to channel failed");
    }
}

fn generate_token(app_id: u32, id: &UserId) -> String {
    type Hasher = blake2::Blake2s<U16>;
    let hash_input = format!(
        "{}{}",
        id,
        time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    format!(
        "{}:{}",
        app_id,
        hex::encode(Hasher::digest(hash_input.into_bytes()))
    )
}
