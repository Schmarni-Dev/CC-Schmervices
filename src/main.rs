pub mod api;
mod db_utils;
pub mod util;
use auth::AuthUser;
use db_utils::*;
use sqlx::SqlitePool;
use tower_http::services::ServeDir;

use std::{collections::HashMap, env, sync::Arc};

use axum::{
    extract::{ws::WebSocket, State},
    http::header,
    response::{Html, Response},
    routing::{get, post},
    Router,
};
use leptos::{ssr::render_to_string as render, *};
use tokio::sync::Mutex;

use totp_rs::{Secret, TOTP};

mod auth;

pub type DBPool = SqlitePool;
#[derive(Clone)]
pub struct App {
    db: Arc<DBPool>,
    transaction_notif_sockets: Arc<Mutex<HashMap<String, WebSocket>>>,
}
use util::*;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    run().await
}
async fn run() -> eyre::Result<()> {
    color_eyre::install()?;
    let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;
    let state = App {
        db: Arc::new(pool),
        transaction_notif_sockets: Arc::new(Mutex::new(HashMap::new())),
    };
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
        .nest_service("/lua", ServeDir::new("lua"))
        .with_state(state);

    // run it with hyper on localhost:3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await.unwrap();
    eyre::Ok(())
}

async fn register_form() -> Response {
    render_html(|| view! {<RegisterForm/>})
}
async fn login_form() -> Response {
    render_html(|| view! {<LoginForm/>})
}

#[component]
pub fn base_64_image(base64: String, alt: String) -> impl IntoView {
    view! { <img src=format!("data:image/png;base64,{}",base64) alt=alt/>}
}

fn get_otp(secret: Secret, username: &str) -> eyre::Result<TOTP> {
    Ok(TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("Schmervices".to_string()),
        username.to_string(),
    )?)
}

#[component]
fn login_form() -> impl IntoView {
    view! {
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
fn register_form() -> impl IntoView {
    view! {
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
    let mut conn = state.db.acquire().await.ok()?;
    sqlx::query!(
        "UPDATE system
             SET visits = visits + 1
             WHERE key = 0;"
    )
    .execute(&mut *conn)
    .await
    .ok()?;

    let data = sqlx::query!(
        "SELECT visits
         FROM system
         WHERE key = 0;"
    )
    .fetch_one(&mut *conn)
    .await
    .ok()?;
    data.visits
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
    let html = render(move || {
        let greeting = display_name.map(|name| {
            move || {
                view! {cx, <h1>"Hello "{name.clone()}</h1>}
            }
        });

        view! {
            <head>
                <script type="text/javascript" src="https://unpkg.com/htmx.org@1.9.4"></script>
                <meta charset="UTF-8"></meta>
                <meta name="viewport" content="width=device-width, initial-scale=1.0"></meta>
                <link href="/css" rel="stylesheet"></link>
            </head>
            <body>
                {greeting}
                <button hx-post="/register_form" hx-swap="outerHTML" class="button">
                    Signup
                </button>
                <button hx-post="/login_form" hx-swap="outerHTML" class="button">
                    Login
                </button>
                <button hx-post="/logout" hx-swap="afterend" class="button">
                    Logout
                </button>
            <footer>Visits: {visits} </footer>
            </body>
        }
    });

    Html::from("<!DOCTYPE html>\n".to_owned() + &html)
}
