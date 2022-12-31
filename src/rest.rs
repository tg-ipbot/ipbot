use std::convert::Infallible;
use std::net::IpAddr;
use std::str::FromStr;

use log::{debug, warn};
use warp::http::StatusCode;
use warp::hyper::body::Bytes;
use warp::{Filter, Rejection};

use crate::db::{AppIpReport, DbCommand};

use super::ApplicationCommand;

pub(super) async fn run_server(
    tx: tokio::sync::mpsc::Sender<ApplicationCommand<String>>,
) -> Result<(), &'static str> {
    let rest_routes = init_rest(tx);

    warp::serve(rest_routes).run(([127, 0, 0, 1], 1234)).await;

    Ok(())
}

async fn request_handler(
    id: String,
    addr: IpAddr,
    cmd_tx: tokio::sync::mpsc::Sender<ApplicationCommand<String>>,
) -> Result<impl warp::Reply, Infallible> {
    if addr.is_ipv6() {
        return Ok(StatusCode::NOT_ACCEPTABLE);
    }

    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    debug!("ID: {id}: {addr}");

    match AppIpReport::from_str(id.as_str(), addr) {
        Ok(report) => {
            match cmd_tx
                .send(ApplicationCommand::new(DbCommand::IpReport(report), tx))
                .await
            {
                Ok(_) => {
                    let _ = rx.await;
                }
                Err(e) => {
                    warn!("{e}");
                    return Ok(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }

            Ok(StatusCode::OK)
        }
        Err(e) => {
            warn!("Report create error: {e}");
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn init_rest(
    tx: tokio::sync::mpsc::Sender<ApplicationCommand<String>>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone + Send + Sync {
    warp::path("app")
        .and(warp::post())
        .and(warp::header::header::<String>("Credential"))
        .and(warp::body::bytes().and_then(|data: Bytes| async move {
            IpAddr::from_str(String::from_utf8_lossy(data.as_ref()).as_ref())
                .map_err(|_e| warp::reject::not_found())
        }))
        .and(warp::any().map(move || tx.clone()))
        .and_then(request_handler)
}
