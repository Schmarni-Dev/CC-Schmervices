pub mod api;
mod db_utils;
pub mod util;
use auth::AuthUser;
use db_utils::*;

use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{ws::WebSocket, State},
    http::header,
    response::{Html, Response},
    routing::{get, post},
    Router,
};
use leptos::{ssr::render_to_string as render, *};
use libsql_client::Client;
use tokio::sync::Mutex;

use totp_rs::{Secret, TOTP};

mod auth;
#[derive(Clone)]
pub struct App {
    db: Arc<Mutex<Client>>,
    transaction_notif_sockets: Arc<Mutex<HashMap<String, WebSocket>>>,
}
use util::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run().await
}
async fn run() -> anyhow::Result<()> {
    let db = Client::from_env().await?;
    setup_db(&db).await?;
    let state = App {
        db: Arc::new(Mutex::new(db)),
        transaction_notif_sockets: Arc::new(Mutex::new(HashMap::new())),
    };
    // build our application with a single route
    let app = Router::new()
        .route(
            "/css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    include_str!(concat!(env!("OUT_DIR"), "/out.css")),
                )
            }),
        )
        .route("/", get(index))
        .route("/register_form", post(register_form))
        .route("/login_form", post(login_form))
        .nest("/", auth::get_router())
        .nest("/api", api::get_router())
        .with_state(state);

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    anyhow::Ok(())
}

async fn register_form() -> Response {
    render_html(|cx| view! {cx, <RegisterForm/>})
}
async fn login_form() -> Response {
    render_html(|cx| view! {cx, <LoginForm/>})
}

#[component]
pub fn base_64_image(cx: Scope, base64: String, alt: String) -> impl IntoView {
    view! {cx, <img src=format!("data:image/png;base64,{}",base64) alt=alt/>}
}

fn get_otp(secret: Secret, username: &str) -> anyhow::Result<TOTP> {
    Ok(TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("Money Money Program".to_string()),
        username.to_string(),
    )?)
}

#[component]
fn login_form(cx: Scope) -> impl IntoView {
    view! {cx,
        <form hx-post="/login" hx-swap="outerHTML">
            <label>Username: </label>
            <input type="text" name="username"> </input>
            <br/>
            <label>PassCode: </label>
            <input type="number" name="otp"> </input>
            <button>Submit</button>
        </form>

    }
}

#[component]
fn register_form(cx: Scope) -> impl IntoView {
    view! {cx,
        <form hx-post="/register" hx-swap="outerHTML">
            <label>Display Name:</label>
            <input type="text" name="display_name"> </input>
            <br/>
            <label>Username Name:</label>
            <input type="text" name="username"> </input>
            <button>Submit</button>
        </form>

    }
}

async fn increment_and_get_visits(state: &App) -> Option<i64> {
    state
        .db
        .lock()
        .await
        .execute(
            "UPDATE system
             SET visits = visits + 1
             WHERE key = 0;",
        )
        .await
        .ok()?;
    let data = get_ints_from_db(
        state,
        "SELECT visits
         FROM system
         WHERE key = 0;",
    )
    .await
    .unwrap()
    .first()?
    .clone();
    Some(data)
}

async fn index(State(state): State<App>, AuthUser(user): AuthUser) -> Html<String> {
    let visits: i64 = increment_and_get_visits(&state).await.unwrap();
    let display_name = if let Some((username, _)) = user {
        Some(
            get_displayname_from_username(&state, &username)
                .await
                .unwrap_or_default(),
        )
    } else {
        None
    };
    let html = render(move |cx| {
        let greeting = display_name.map(|name| {
            move || {
                view! {cx, <h1>"Hello "{name.clone()}</h1>}
            }
        });

        view! {cx,
            <head>
                <script type="text/javascript" src="https://unpkg.com/htmx.org@1.9.4"></script>
                <meta charset="UTF-8"></meta>
                <meta name="viewport" content="width=device-width, initial-scale=1.0"></meta>
                <link href="/css" rel="stylesheet"></link>
            </head>
            <body>
                {greeting}
                <button hx-post="/register_form" hx-swap="outerHTML" class="border-4">
                    Signup
                </button>
                <button hx-post="/login_form" hx-swap="outerHTML" class="border-4">
                    Login
                </button>
                <button hx-post="/logout" hx-swap="afterend" class="border-4">
                    Logout
                </button>
            <footer>Visits: {visits} </footer>
            </body>
        }
    });

    Html::from("<!DOCTYPE html>\n".to_owned() + &html)
}
