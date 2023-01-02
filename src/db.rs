use core::fmt::Debug;
use std::net::IpAddr;
use std::str::FromStr;
use std::time;

use blake2::{digest::consts::U16, Digest};
use log::{debug, error, warn};
use rand::prelude::*;
use redis::aio::Connection;
use redis::{AsyncCommands, RedisResult};
use teloxide::prelude::UserId;

use super::ApplicationCommand;

#[derive(Debug)]
pub(crate) struct AppIpReport {
    id: UserId,
    token: String,
    addr: IpAddr,
}

impl AppIpReport {
    pub(crate) fn new(id: UserId, token: String, addr: IpAddr) -> Self {
        Self { id, token, addr }
    }

    pub(crate) fn from_str(auth_token: &str, addr: IpAddr) -> Result<Self, &'static str> {
        let (id, token) = if let Some((id_str, _)) = auth_token.split_once(':') {
            let id_value: Result<u64, _> = id_str.parse();

            if id_value.is_err() {
                return Err("Failed to convert ID");
            }

            (UserId(id_value.unwrap()), auth_token.to_string())
        } else {
            return Err("Invalid token format");
        };

        Ok(AppIpReport::new(id, token, addr))
    }
}

#[derive(Debug)]
pub(crate) enum DbCommand {
    TokenGenerateRequest(UserId),
    IpGetRequest(UserId),
    IpReport(AppIpReport),
}

const USER_ID_KEY: &str = "user:id";
const APP_TOKEN_KEY: &str = "token";
const APP_ADDRESS_KEY: &str = "address";
const APP_ID_KEY: &str = "id";

pub(crate) async fn db_task<T: Sync + Send + Debug + From<&'static str> + From<String>>(
    mut receiver: tokio::sync::mpsc::Receiver<ApplicationCommand<T>>,
) -> Result<(), &'static str> {
    let redis_socket =
        std::env::var("REDIS_SOCKET").expect("Specify REDIS_SOCKET environment variable");
    debug!("socket: {}", redis_socket);
    let client = match redis::Client::open(redis_socket) {
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
                    process_get_ip_reqeust(&mut con, command, id).await;
                }
                DbCommand::IpReport(report) => {
                    debug!("Report: {} - IP: {}", report.id, report.addr);
                    process_ip_report(&mut con, command.tx_channel, report).await;
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

async fn process_ip_report<'a, T: Sync + Send + Debug + From<&'a str>>(
    con: &mut Connection,
    tx: tokio::sync::oneshot::Sender<T>,
    report: AppIpReport,
) {
    let app_key = format!("app:{}", report.id.0);
    let is_exists: RedisResult<bool> = con.exists(app_key.as_str()).await;

    if let Err(e) = is_exists {
        error!("Failed to process report: {}", e.category());
        return;
    }

    if !is_exists.unwrap() {
        error!("Application ID does not exists");
        return;
    }

    let db_token: String = con.hget(app_key.as_str(), APP_TOKEN_KEY).await.unwrap();

    if db_token != report.token {
        warn!("Token mismatch");
        return;
    }

    let result: RedisResult<bool> = con
        .hset(app_key.as_str(), APP_ADDRESS_KEY, report.addr.to_string())
        .await;

    if let Err(e) = result {
        error!("Failed to set report value: {}", e.category());
        return;
    }

    let _ = tx.send("ok".into());
}

async fn process_get_ip_reqeust<T: Sync + Send + Debug + From<String>>(
    con: &mut Connection,
    command: ApplicationCommand<T>,
    id: UserId,
) {
    let user_key = format!("user:{}", id.0);
    let app_iter: Option<Vec<String>> = con.smembers(user_key.as_str()).await.ok();

    if app_iter.is_none() {
        warn!("Failed to retrieve user's applications");
        return;
    }

    let app_iter = app_iter.unwrap();
    let app_key = app_iter.first();

    if app_key.is_none() {
        let response = "No application registered, so no reports available".to_string();
        debug!("No application registered for the user {id}");

        if command.tx_channel.send(response.into()).is_err() {
            error!("Sending to channel failed");
        }

        return;
    }

    let result: Option<String> = con.hget(app_key.unwrap(), APP_ADDRESS_KEY).await.ok();

    let response = match result {
        Some(addr_str) => format!("Your reported IP is `{addr_str}`"),
        None => "No reported IP addresses for you".to_string(),
    };

    if command.tx_channel.send(response.into()).is_err() {
        error!("Sending to channel failed");
    }
}

async fn process_token_request<T: Sync + Send + Debug + From<String>>(
    con: &mut Connection,
    command: ApplicationCommand<T>,
    id: UserId,
) {
    let user_key = format!("user:{}", id.0);
    let app_iter: Vec<String> = con.smembers(user_key.as_str()).await.unwrap();

    let (app_key, app_id) = if let Some(app) = app_iter.first() {
        debug!("Found application registered: {}", app);
        let mut it = app.split(':');
        let app_id = loop {
            if let Some(part) = it.next() {
                let result = u32::from_str(part).ok();

                if result.is_some() {
                    break result.unwrap();
                }
            } else {
                error!("Invalid application entry in the DB: {}", app);
                panic!();
            }
        };

        (app.clone(), app_id)
    } else {
        let app_id = con.get(USER_ID_KEY).await.unwrap();
        debug!("Register new application: {}", app_id);
        (format!("app:{}", app_id), app_id)
    };

    let user_token: String = match con.hget(app_key.as_str(), APP_TOKEN_KEY).await.ok() {
        Some(token) => {
            debug!("Found existing token for the application {}", app_id);
            token
        }
        None => {
            debug!("Generate a token for the application {}", app_id);
            let token = generate_token(app_id, &id);

            if let Err(e) = redis::pipe()
                .atomic()
                .hset(app_key.as_str(), APP_TOKEN_KEY, &token)
                .ignore()
                .hset(app_key.as_str(), APP_ID_KEY, id.0)
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

    if command.tx_channel.send(user_token.into()).is_err() {
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

#[cfg(test)]
mod db_tests {
    use super::generate_token;
    use teloxide::types::UserId;

    #[test]
    fn test_generate_token() {
        let token = generate_token(u32::MIN, &UserId(u64::MIN));
        let (split_left, split_right) = token.split_once(':').unwrap();
        assert!(!token.is_empty());
        assert!(!split_left.is_empty());
        assert!(!split_right.is_empty());
        let token = generate_token(u32::MIN, &UserId(u64::MAX));
        let (split_left, split_right) = token.split_once(':').unwrap();
        assert!(!token.is_empty());
        assert!(!split_left.is_empty());
        assert!(!split_right.is_empty());
        let token = generate_token(u32::MAX, &UserId(u64::MIN));
        let (split_left, split_right) = token.split_once(':').unwrap();
        assert!(!token.is_empty());
        assert!(!split_left.is_empty());
        assert!(!split_right.is_empty());
        let token = generate_token(u32::MAX, &UserId(u64::MAX));
        let (split_left, split_right) = token.split_once(':').unwrap();
        assert!(!token.is_empty());
        assert!(!split_left.is_empty());
        assert!(!split_right.is_empty());
    }
}
