use std::convert::Infallible;
use std::net::IpAddr;
use std::str::FromStr;

use warp::http::StatusCode;
use warp::hyper::body::Bytes;
use warp::{Filter, Rejection};

use super::ApplicationCommand;

pub(super) async fn run_server(
    tx: tokio::sync::mpsc::Sender<ApplicationCommand<String>>,
) -> Result<(), &'static str> {
    let rest_routes = init_rest(tx);

    warp::serve(rest_routes).run(([127, 0, 0, 1], 1234)).await;

    Ok(())
}

async fn request_handler(id: String, addr: IpAddr) -> Result<impl warp::Reply, Infallible> {
    if let IpAddr::V6(_) = addr {
        return Ok(StatusCode::NOT_ACCEPTABLE);
    }

    println!("ID: {id}: {addr}");
    Ok(StatusCode::OK)
}

fn app_request_handler(id: u32) -> String {
    format!("app:{}", id)
}

fn init_rest(
    _tx: tokio::sync::mpsc::Sender<ApplicationCommand<String>>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = Rejection> + Clone + Send + Sync {
    let app_path = warp::path("app")
        .and(warp::post())
        .and(warp::path::param().map(app_request_handler))
        .and(warp::body::bytes().and_then(|data: Bytes| async move {
            IpAddr::from_str(String::from_utf8_lossy(data.as_ref()).as_ref())
                .map_err(|_e| warp::reject::not_found())
        }))
        .and_then(request_handler);

    app_path
}
