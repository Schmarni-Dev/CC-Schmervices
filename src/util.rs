use axum::{
    async_trait,
    body::HttpBody,
    extract::FromRequest,
    http::{header::ACCEPT, HeaderMap, HeaderValue, Request, StatusCode},
    response::{Html, IntoResponse, Response},
    BoxError, Form, Json,
};
use leptos::{ssr::render_to_string, IntoView, Scope};
use libsql_client::{args, Statement};
use rand::seq::IteratorRandom;
use serde::de::DeserializeOwned;

pub struct ApiRequest<T: DeserializeOwned> (
    pub  T);

#[derive(Clone, Copy)]
pub enum RequestTypeEnum {
    Html,
    Json,
}

pub fn get_random_string(length: u8) -> String {
let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".chars();
    let string = (0..length).into_iter().map(|_| {
        letters.clone().choose(&mut rand::thread_rng()).unwrap()
    }).collect();

    string

}

pub async fn get_displayname_from_valid_auth_token(app: &App, token: &str) -> Option<String> {
    let w = get_string_from_db(app, 
        Statement::with_args(
            "SELECT display_name FROM auth_tokens INNER JOIN users USING(username) WHERE token = ? ;", 
            args!(token)
        )
    ).await;
    w.map_err(|err| {println!("join err: {}",err);err}).ok()
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

use crate::{db_utils::get_string_from_db, App};
pub fn render_html<F, N>(f: F) -> Response
where
    F: FnOnce(Scope) -> N + 'static,
    N: IntoView,
{
    render_html_into_body(f).into_response()
}
pub fn render_html_into_body<F, N>(f: F) -> Html<String>
where
    F: FnOnce(Scope) -> N + 'static,
    N: IntoView,
{
    Html::from(render_to_string(f))
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
impl<S, B, T> FromRequest<S, B> for ApiRequest<T>
where
    T: DeserializeOwned,
    B: HttpBody + Send + 'static + Sync,
    B::Data: Send,
    B::Error: Into<BoxError>,
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        Ok(match get_requested_type(req.headers()) {
            Some(RequestTypeEnum::Json) => {
                let data = Json::<T>::from_request(req, state).await;
                let out = data.map_err(|err|{ println!("Json Err: {}",err) ;StatusCode::EXPECTATION_FAILED})?.0;
                ApiRequest (
                    out
                )
            }
            Some(RequestTypeEnum::Html) => {
                let data = Form::<T>::from_request(req, state).await;
                let out = data.map_err(|_| StatusCode::BAD_REQUEST)?.0;
                ApiRequest (
                    out
                )
            }

            _ => return Err(StatusCode::BAD_REQUEST),
        })
    }
}
