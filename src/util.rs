use axum::{
    async_trait,
    extract::{FromRequest, Request},
    http::{header::ACCEPT, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Form, Json,
};
use leptos::{ssr::render_to_string, IntoView};
use rand::seq::IteratorRandom;
use serde::de::DeserializeOwned;

pub struct ApiRequest<T: DeserializeOwned>(pub T);

#[derive(Clone, Copy)]
pub enum RequestTypeEnum {
    Html,
    Json,
}

pub fn get_random_string(length: u8) -> String {
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".chars();
    (0..length)
        .map(|_| letters.clone().choose(&mut rand::thread_rng()).unwrap())
        .collect()
}

pub async fn get_displayname_from_valid_auth_token(app: &App, token: &str) -> Option<String> {
    sqlx::query!(
        "SELECT display_name FROM auth_tokens INNER JOIN users USING(username) WHERE token = ? ;",
        token
    )
    .fetch_one(&mut *app.db.acquire().await.ok()?)
    .await
    .ok()
    .map(|r| r.display_name)
    // w.map_err(|err| {println!("join err: {}",err);err}).ok()
}

macro_rules! err_handle {
    ($req_type:expr,$json:expr,$html:expr) => {
        match $req_type {
            RequestTypeEnum::Json => $json,
            RequestTypeEnum::Html => $html,
        }
    };
}
pub(crate) use err_handle;

use crate::App;
pub fn render_html<F, N>(f: F) -> Response
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    render_html_into_body(f).into_response()
}
pub fn render_html_into_body<F, N>(f: F) -> Html<String>
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    Html::from(render_to_string(f).to_string())
}

pub fn get_requested_type(headers: &HeaderMap<HeaderValue>) -> Option<RequestTypeEnum> {
    let matchable = headers.get(ACCEPT).map(|x| x.to_str().unwrap_or_default());
    let htmx = headers.get("HX-Request").map(|x| x.as_bytes());
    match matchable {
        Some(header) if header.contains("application/json") => Some(RequestTypeEnum::Json),
        //  Fuking stupid hack !!!Risky!!!
        Some(header) if header.contains("custom/ws") => Some(RequestTypeEnum::Json),

        Some(header) if header.contains("text/html") => Some(RequestTypeEnum::Html),
        Some("*/*") => match htmx {
            Some(b"true") => Some(RequestTypeEnum::Html),
            _ => None,
        },
        _ => None,
    }
}

#[async_trait]
impl<S, T> FromRequest<S> for ApiRequest<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Ok(match get_requested_type(req.headers()) {
            Some(RequestTypeEnum::Json) => {
                let data = Json::<T>::from_request(req, state).await;
                let out = data
                    .map_err(|err| {
                        println!("Json Err: {}", err);
                        StatusCode::EXPECTATION_FAILED
                    })?
                    .0;
                ApiRequest(out)
            }
            Some(RequestTypeEnum::Html) => {
                let data = Form::<T>::from_request(req, state).await;
                let out = data.map_err(|_| StatusCode::BAD_REQUEST)?.0;
                ApiRequest(out)
            }

            _ => return Err(StatusCode::BAD_REQUEST),
        })
    }
}
