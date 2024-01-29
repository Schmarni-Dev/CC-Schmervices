use std::{fmt::Debug, ops::Add};

use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts, State},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, RequestPartsExt, Router,
};
use axum_extra::extract::{cookie::Cookie, CookieJar};
use chrono::Duration;
use leptos::*;
use serde_json::json;
use totp_rs::Secret;

use crate::{
    db_utils::get_displayname_from_username,
    get_otp, render_html,
    util::{err_handle, get_requested_type, render_html_into_body, ApiRequest, RequestTypeEnum},
    App, Base64Image, LoginForm, RegisterForm,
};

pub const AUTH_IDENT: &str = "Money-Auth-Key";
pub fn get_auth_token_lifetime() -> Duration {
    Duration::weeks(1)
}

pub fn get_router() -> Router<App> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
}

#[derive(serde::Deserialize, Debug)]
pub struct LoginData {
    username: String,
    otp: i32,
}

#[derive(serde::Deserialize, Debug)]
pub struct RegisterData {
    username: String,
    display_name: String,
}

async fn logout(
    State(state): State<App>,
    cookie_jar: Option<CookieJar>,
    RequestType(req_type): RequestType,
    AuthUser(auth_data): AuthUser,
) -> Response {
    let err = err_handle!(
        req_type,
        |err: String| Json(json!({"error":err})).into_response(),
        |err: String| render_html(|| view! {
            <div class="text-red-600">{err}</div>
        })
    );
    let (_, token) = match auth_data {
        Some(d) => d,
        None => return err("Logout From what exactly?".to_string()),
    };
    let mut conn = match state.db.acquire().await {
        Ok(v) => v,
        Err(e) => return err(e.to_string()),
    };
    // TODO: Send Error to client
    let _ = sqlx::query!("DELETE FROM auth_tokens WHERE token = ?", token)
        .execute(&mut *conn)
        .await;
    if let (RequestTypeEnum::Html, Some(jar)) = (req_type, cookie_jar) {
        let jar = jar.remove(Cookie::from(AUTH_IDENT));
        return (StatusCode::OK, jar).into_response();
    }

    StatusCode::OK.into_response()
}
async fn gen_token_and_store_in_db(app: &App, username: &str) -> eyre::Result<String> {
    let token = Secret::generate_secret().to_encoded().to_string();
    let expire_stamp = (chrono::Utc::now() + get_auth_token_lifetime()).timestamp();

    sqlx::query!(
        "INSERT INTO auth_tokens VALUES (?,?,?)",
        token,
        username,
        expire_stamp
    )
    .execute(&mut *app.db.acquire().await?)
    .await?;
    Ok(token)
}

async fn login(
    State(state): State<App>,
    cookie_jar: Option<CookieJar>,
    RequestType(req_type): RequestType,
    ApiRequest(data): ApiRequest<LoginData>,
) -> Response {
    let err = err_handle!(
        req_type,
        |err: String| Json(json!({"error":err})).into_response(),
        |err: String| render_html(|| view! {
            <div class="text-red-600">{err}</div>
            <LoginForm/>
        })
    );
    let username = data.username.to_lowercase();
    let mut conn = match state.db.acquire().await {
        Ok(v) => v,
        Err(e) => return err(e.to_string()),
    };
    let r = sqlx::query!("SELECT secret FROM users WHERE username = ?;", username)
        .fetch_one(&mut *conn)
        .await;
    let otp_secret = match r {
        Ok(secret) => secret.secret,
        Err(_) => return err("User Not Found".to_owned()),
    };
    let otp_secret = Secret::Encoded(otp_secret);

    let otp = get_otp(otp_secret, &username).unwrap();
    let correct = otp.check_current(&data.otp.to_string()).unwrap();
    let display_name = get_displayname_from_username(&state, &username)
        .await
        .unwrap();
    if !correct {
        return err("Incorect Passcode? maybe? idk".to_string());
    }
    match req_type {
        RequestTypeEnum::Html => {
            let cookie_jar = match cookie_jar {
                Some(j) => j,
                None => return StatusCode::BAD_REQUEST.into_response(),
            };
            if cookie_jar.get(AUTH_IDENT).is_some() {
                return err("already logged in!".to_string());
            }
            let token = match gen_token_and_store_in_db(&state, &username).await {
                Ok(token) => token,
                Err(_) => {
                    return err("Could not insert new token into db!".to_string());
                }
            };
            let mut cookie = Cookie::new(AUTH_IDENT, token.clone());
            cookie.set_http_only(Some(true));
            cookie.set_same_site(Some(axum_extra::extract::cookie::SameSite::Lax));

            let cookie_jar = cookie_jar.add(cookie);
            (
                cookie_jar,
                render_html_into_body(move || {
                    view! {
                        <p>Hi {display_name}</p>
                        <p>otp correct: {correct}</p>
                    }
                }),
            )
                .into_response()
        }
        RequestTypeEnum::Json => {
            let token = match gen_token_and_store_in_db(&state, &username).await {
                Ok(token) => token,
                Err(_) => {
                    return err("Could not insert new token into db!".to_string());
                }
            };
            Json(json!({"auth_token":token})).into_response()
        }
    }
}
async fn register(
    State(state): State<App>,
    RequestType(req_type): RequestType,
    ApiRequest(data): ApiRequest<RegisterData>,
) -> Response {
    let err = err_handle!(
        req_type,
        |err: String| Json(json!({"error":err})).into_response(),
        |err: String| render_html(|| view! {
            <div class="text-red-600">{err}</div>
            <br/>
            <RegisterForm/>
        })
    );

    if data.username.contains(':') {
        return err("Username Contains Forbidden \':\' Symbol".to_owned());
    };
    let username = data.username.to_lowercase();
    let secret = Secret::generate_secret();
    let otp = get_otp(secret, &username).unwrap();
    let qr_code = otp.get_qr_base64().unwrap();
    let secret = otp.get_secret_base32();
    let mut conn = match state.db.acquire().await {
        Ok(v) => v,
        Err(e) => return err(e.to_string()),
    };
    match sqlx::query!(
        "INSERT INTO users VALUES (?,?,?,1000,FALSE);",
        otp.account_name,
        data.display_name,
        secret
    )
    .execute(&mut *conn)
    .await
    {
        Ok(_) => render_html(|| {
            view! {
                <div>
                    <Base64Image base64=qr_code alt="Qr Code".to_string()/>
                    <p>OTP Secret:{secret}</p>
                </div>
            }
        }),
        Err(_) => render_html(|| {
            view! {
                Error while inserting user into Database
                <RegisterForm />
            }
        }),
    }
}
#[async_trait]
impl<S> FromRequestParts<S> for App
where
    Self: FromRef<S>, // <---- added this line
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::from_ref(state)) // <---- added this line
    }
}

pub struct AuthUser(pub Option<(String, String)>);

async fn check_db_for_auth_token(token: &str, db: &App) -> Option<String> {
    let mut conn = db.db.acquire().await.ok()?;
    let now = chrono::Utc::now().timestamp();
    let out = sqlx::query!(
        "SELECT username FROM auth_tokens WHERE token = ? AND expire_timestamp > ?;",
        token,
        now
    )
    .fetch_one(&mut *conn)
    .await
    .ok()?;
    let new = chrono::Utc::now()
        .add(get_auth_token_lifetime())
        .timestamp();
    let _ = sqlx::query!(
        "UPDATE auth_tokens SET expire_timestamp = ? WHERE token = ?;",
        token,
        new
    )
    .execute(&mut *conn)
    .await;
    Some(out.username)
}
pub struct RequestType(pub RequestTypeEnum);
#[async_trait]
impl<S> FromRequestParts<S> for RequestType
where
    S: Send + Sync,
{
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(RequestType(
            get_requested_type(&parts.headers).ok_or(StatusCode::UNSUPPORTED_MEDIA_TYPE)?,
        ))
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    App: FromRef<S>,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = parts
            .extract_with_state::<App, _>(state)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let out = match get_requested_type(&parts.headers) {
            Some(RequestTypeEnum::Html) => {
                let cookie_jar = CookieJar::from_headers(&parts.headers);
                let out = match cookie_jar.get(AUTH_IDENT) {
                    Some(cookie) => {
                        let user = check_db_for_auth_token(cookie.value(), &app_state).await;
                        user.map(|u| (u, cookie.value().to_string()))
                    }
                    None => None,
                };
                AuthUser(out)
            }
            Some(RequestTypeEnum::Json) => {
                let user = match parts.headers.get(AUTH_IDENT).and_then(|v| v.to_str().ok()) {
                    Some(token) => check_db_for_auth_token(token, &app_state)
                        .await
                        .map(|user| (user, token.to_owned())),
                    None => None,
                };
                return Ok(AuthUser(user));
            }
            None => return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE),
        };
        Ok(out)
    }
}
