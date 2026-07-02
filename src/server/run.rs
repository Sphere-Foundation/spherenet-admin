//! Read-only JSON HTTP API over the admin commands.
//!
//! Follows the house actix-web style. Currently exposes the monetary policy
//! account (`GET /mp`) by reusing `mp::show::fetch` — the same data the
//! `mp show --output json` command prints. The intent is to grow this into a
//! full admin API/dashboard: every `fetch` becomes a read endpoint, and — once
//! signing is wrapped behind an authenticated layer — the write commands become
//! POST endpoints.

use crate::cli::output::{field, newline};
use actix_cors::Cors;
use actix_web::{
    dev::Server, get, http, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};

const DEFAULT_HOST: &str = "0.0.0.0";

const BANNER_ART: &str = concat!(
    "\n",
    r"   ____     __               _  __    __
  / __/__  / /  ___ _______ / |/ /__ / /_
 _\ \/ _ \/ _ \/ -_) __/ -_)    / -_) __/
/___/ .__/_//_/\__/_/  \__/_/|_/\__/\__/
   /_/                 a d m i n   a p i"
);

/// Injected app state shared across handlers.
#[derive(Clone)]
struct ApiState {
    rpc_url: String,
}

/// Build and start the HTTP server (returns the running `Server` future).
pub fn server(rpc_url: String, port: u16) -> Server {
    let state = ApiState { rpc_url };

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![http::header::AUTHORIZATION, http::header::ACCEPT])
            .allowed_header(http::header::CONTENT_TYPE)
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(state.clone()))
            .service(index)
            .service(health)
            .service(monetary_policy)
            .service(validator_whitelist)
            .service(program_whitelist)
            .service(balance)
            .service(epoch)
            .service(vote_account)
            .service(stake_account)
            .service(airdrop)
    })
    .bind(format!("{}:{}", DEFAULT_HOST, port))
    .expect("unable to bind server")
    .run()
}

/// Run the API to completion on an actix runtime (blocks). Called from the
/// synchronous CLI dispatch for `spherenet-admin serve`.
pub fn serve(rpc_url: &str, port: u16) -> eyre::Result<()> {
    let mut banner = String::from(BANNER_ART);
    banner.push_str(newline());
    banner.push_str(newline());
    banner.push_str(&field("Listening", format!("http://{}:{}", DEFAULT_HOST, port)));
    banner.push_str(&field("RPC", rpc_url));
    banner.push_str(newline());
    banner.push_str("Endpoints:");
    banner.push_str(newline());
    // Wider column than `subfield` — the method-prefixed signatures are long.
    let ep = |sig: &str, desc: &str| format!("  {:<24}{}\n", sig, desc);
    banner.push_str(&ep("GET  /", "index"));
    banner.push_str(&ep("GET  /health", "health check"));
    banner.push_str(&ep("GET  /mp", "monetary policy"));
    banner.push_str(&ep("GET  /vw", "validator whitelist"));
    banner.push_str(&ep("GET  /pw", "program whitelist"));
    banner.push_str(&ep("GET  /epoch", "current epoch"));
    banner.push_str(&ep("GET  /balance/{pubkey}", "native SPHR balance"));
    banner.push_str(&ep("GET  /vote/{pubkey}", "vote account"));
    banner.push_str(&ep("GET  /stake/{pubkey}", "stake account"));
    banner.push_str(&ep("POST /airdrop/{pubkey}", "request airdrop (testnet, ?amount=)"));
    banner.push_str(newline());
    banner.push_str("Append ?pretty for pretty-printed JSON.");
    println!("{}", banner);

    let rpc_url = rpc_url.to_string();
    actix_web::rt::System::new()
        .block_on(async move { server(rpc_url, port).await })
        .map_err(|e| eyre::eyre!("server error: {}", e))?;
    Ok(())
}

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body("spherenet-admin API")
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok()
}

// The `fetch`es use the blocking solana RpcClient, which spins its own runtime
// internally — that can't run on an async worker, so each handler hands its
// fetch to the blocking threadpool via `web::block`, then `respond` renders it.

#[get("/mp")]
async fn monetary_policy(req: HttpRequest, state: web::Data<ApiState>) -> impl Responder {
    let rpc_url = state.rpc_url.clone();
    respond(&req, web::block(move || crate::mp::show::fetch(&rpc_url)).await)
}

#[get("/vw")]
async fn validator_whitelist(req: HttpRequest, state: web::Data<ApiState>) -> impl Responder {
    let rpc_url = state.rpc_url.clone();
    respond(&req, web::block(move || crate::vw::show::fetch(&rpc_url)).await)
}

#[get("/pw")]
async fn program_whitelist(req: HttpRequest, state: web::Data<ApiState>) -> impl Responder {
    let rpc_url = state.rpc_url.clone();
    respond(&req, web::block(move || crate::pw::show::fetch(&rpc_url)).await)
}

#[get("/balance/{pubkey}")]
async fn balance(
    req: HttpRequest,
    state: web::Data<ApiState>,
    pubkey: web::Path<String>,
) -> impl Responder {
    let rpc_url = state.rpc_url.clone();
    let pubkey = pubkey.into_inner();
    respond(
        &req,
        web::block(move || crate::utils::show::fetch_balance(&rpc_url, pubkey)).await,
    )
}

#[get("/epoch")]
async fn epoch(req: HttpRequest, state: web::Data<ApiState>) -> impl Responder {
    let rpc_url = state.rpc_url.clone();
    respond(&req, web::block(move || crate::utils::show::fetch_epoch(&rpc_url)).await)
}

#[get("/vote/{pubkey}")]
async fn vote_account(
    req: HttpRequest,
    state: web::Data<ApiState>,
    pubkey: web::Path<String>,
) -> impl Responder {
    let rpc_url = state.rpc_url.clone();
    let pubkey = pubkey.into_inner();
    respond_opt(
        &req,
        web::block(move || crate::vote::show::fetch(&rpc_url, &pubkey)).await,
    )
}

#[get("/stake/{pubkey}")]
async fn stake_account(
    req: HttpRequest,
    state: web::Data<ApiState>,
    pubkey: web::Path<String>,
) -> impl Responder {
    let rpc_url = state.rpc_url.clone();
    let pubkey = pubkey.into_inner();
    respond_opt(
        &req,
        web::block(move || crate::stake::show::fetch(&rpc_url, &pubkey)).await,
    )
}

#[derive(serde::Deserialize)]
struct AirdropParams {
    amount: Option<f64>,
}

/// Keyless faucet request (testnet) — state-changing, so POST. No signing:
/// the faucet funds the account.
#[post("/airdrop/{pubkey}")]
async fn airdrop(
    req: HttpRequest,
    state: web::Data<ApiState>,
    pubkey: web::Path<String>,
    params: web::Query<AirdropParams>,
) -> impl Responder {
    let rpc_url = state.rpc_url.clone();
    let pubkey = pubkey.into_inner();
    let amount = params.amount.unwrap_or(1.0);
    respond(
        &req,
        web::block(move || crate::utils::run::request_airdrop(&rpc_url, pubkey, amount)).await,
    )
}

/// Turn a blocking `fetch` result into an HTTP JSON response — the shared shape
/// for every read endpoint. Distinguishes fetch errors (500 + message) from
/// blocking-pool errors.
fn respond<T: serde::Serialize>(
    req: &HttpRequest,
    result: Result<eyre::Result<T>, actix_web::error::BlockingError>,
) -> HttpResponse {
    match result {
        Ok(Ok(view)) => json_response(&view, wants_pretty(req)),
        Ok(Err(e)) => HttpResponse::InternalServerError().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(format!("blocking error: {e}")),
    }
}

/// Like `respond`, but for `fetch`es that return `Option` — `None` (account
/// does not exist) becomes a 404 with `{ "found": false }`.
fn respond_opt<T: serde::Serialize>(
    req: &HttpRequest,
    result: Result<eyre::Result<Option<T>>, actix_web::error::BlockingError>,
) -> HttpResponse {
    match result {
        Ok(Ok(Some(view))) => json_response(&view, wants_pretty(req)),
        Ok(Ok(None)) => HttpResponse::NotFound().json(serde_json::json!({ "found": false })),
        Ok(Err(e)) => HttpResponse::InternalServerError().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(format!("blocking error: {e}")),
    }
}

/// True when the request opts into pretty-printed JSON via `?pretty`
/// (bare `?pretty`, `?pretty=1`, etc. all count).
fn wants_pretty(req: &HttpRequest) -> bool {
    req.query_string()
        .split('&')
        .any(|kv| kv.split('=').next() == Some("pretty"))
}

/// Render a value as JSON — compact by default (machine consumers), or
/// pretty-printed when requested. Reusable across endpoints.
fn json_response<T: serde::Serialize>(value: &T, pretty: bool) -> HttpResponse {
    if pretty {
        match serde_json::to_string_pretty(value) {
            Ok(body) => HttpResponse::Ok().content_type("application/json").body(body),
            Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
        }
    } else {
        HttpResponse::Ok().json(value)
    }
}
