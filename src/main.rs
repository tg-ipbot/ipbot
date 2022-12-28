use std::fmt::Debug;

use log::error;
use teloxide::prelude::*;

use bot::setup_bot;
use db::{db_task, DbCommand};
use rest::run_server;

mod bot;
mod db;
mod rest;

#[derive(Debug)]
pub(self) struct ApplicationCommand<T> {
    db_cmd: DbCommand,
    tx_channel: tokio::sync::oneshot::Sender<T>,
}

impl<T> ApplicationCommand<T> {
    fn new(db_cmd: DbCommand, tx_channel: tokio::sync::oneshot::Sender<T>) -> Self {
        ApplicationCommand { db_cmd, tx_channel }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let tgbot = Bot::from_env();
    let (tx, rx) = tokio::sync::mpsc::channel::<ApplicationCommand<String>>(16);
    let tx2 = tx.clone();

    if let Err(e) = tokio::try_join!(setup_bot(tgbot, tx), db_task(rx), run_server(tx2)) {
        error!("{}", e);
    }
}
