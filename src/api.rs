use axum::{
    body::Body,
    extract::{ws::Message, Path, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::{query, Acquire};

use crate::{
    auth::{AuthUser, RequestType},
    util::{get_displayname_from_valid_auth_token, get_random_string, ApiRequest},
    App,
};

pub fn get_router() -> Router<App> {
    Router::new()
        .route("/get_displayname", post(get_displayname))
        .route("/request_transaction", post(request_transaction))
        .route("/accept_transaction/:id", post(accept_transaction))
        .route("/reject_transaction/:id", post(reject_transaction))
        .route("/notify_transaction/:id", post(notify_transaction))
}

#[derive(Deserialize, Debug)]
struct RequestTransaction {
    buyer: String,
    name: String,
    amount: i32,
}

struct TransactionData {
    buyer: String,
    seller: String,
    name: String,
    amount: i64,
    /// 0 = waiting, 1 = accepted, 2 = rejected
    accepted: i8,
    timestamp: i64,
}

async fn handle_notify(state: &App, id: &str, accepted: bool) {
    let msg = if accepted {
        "transaction_accepted"
    } else {
        "transaction_rejected"
    };
    if let Some(mut socket) = state.transaction_notif_sockets.lock().await.remove(id) {
        _ = socket.send(Message::Text(msg.to_owned())).await;
        _ = socket.close().await;
    }
}

async fn notify_transaction(
    State(state): State<App>,
    AuthUser(user): AuthUser,
    Path(transaction_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let user = match user {
        Some((name, _)) => name,
        None => Err(StatusCode::UNAUTHORIZED)?,
    };
    let can_read_status = sqlx::query!(
        "SELECT true FROM transactions WHERE seller = ? OR buyer = ?;",
        user,
        user
    )
    .fetch_one(
        &mut *state
            .db
            .acquire()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    )
    .await
    .is_ok_and(|o| o.r#true == 1);
    if !can_read_status {
        Err(StatusCode::UNAUTHORIZED)?;
    }
    let out = ws.on_upgrade(move |socket| async move {
        state
            .transaction_notif_sockets
            .lock()
            .await
            .insert(transaction_id, socket);
    });
    Ok(out)
}

async fn accept_transaction(
    State(state): State<App>,
    RequestType(req_type): RequestType,
    AuthUser(user): AuthUser,
    Path(transaction_id): Path<String>,
) -> Result<Response, StatusCode> {
    let user = match user {
        Some((name, _)) => name,
        None => Err(StatusCode::UNAUTHORIZED)?,
    };
    let mut conn = match state.db.acquire().await {
        Ok(conn) => conn,
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR)?,
    };
    let exsits = sqlx::query!(
        "SELECT true FROM transactions WHERE id= ? AND buyer = ?;",
        transaction_id,
        user
    )
    .fetch_one(&mut *conn)
    .await
    .is_ok_and(|r| r.r#true == 1);

    if !exsits {
        Err(StatusCode::NOT_FOUND)?;
    }
    sqlx::query!(
        "UPDATE transactions SET accepted = 1 WHERE id = ? AND buyer = ?;",
        transaction_id,
        user
    )
    .execute(&mut *conn)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    handle_notify(&state, &transaction_id, true).await;

    Ok(StatusCode::OK.into_response())
}

async fn reject_transaction(
    State(state): State<App>,
    RequestType(req_type): RequestType,
    AuthUser(user): AuthUser,
    Path(transaction_id): Path<String>,
) -> Result<Response, StatusCode> {
    let user = match user {
        Some((name, _)) => name,
        None => Err(StatusCode::UNAUTHORIZED)?,
    };

    let mut conn = match state.db.acquire().await {
        Ok(conn) => conn,
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR)?,
    };
    let exsits = sqlx::query!(
        "SELECT true FROM transactions WHERE id= ? AND buyer = ?;",
        transaction_id,
        user
    )
    .fetch_one(&mut *conn)
    .await
    .is_ok_and(|r| r.r#true == 1);

    if !exsits {
        Err(StatusCode::NOT_FOUND)?;
    }
    sqlx::query!(
        "UPDATE transactions SET accepted = 2 WHERE id = ? AND buyer = ?;",
        transaction_id,
        user
    )
    .execute(&mut *conn)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    handle_notify(&state, &transaction_id, false).await;

    Ok(StatusCode::OK.into_response())
}

async fn request_transaction(
    State(state): State<App>,
    AuthUser(user): AuthUser,
    ApiRequest(data): ApiRequest<RequestTransaction>,
) -> Result<impl IntoResponse, StatusCode> {
    let user = match user {
        Some((name, _)) => name,
        None => Err(StatusCode::UNAUTHORIZED)?,
    };
    let now = chrono::Utc::now().timestamp();
    let id = get_random_string(8);
    sqlx::query!(
        "INSERT INTO transactions VALUES (?,?,?,?,?,0,?)",
        id,
        data.buyer,
        user,
        data.name,
        data.amount,
        now
    )
    .execute(
        &mut *state
            .db
            .acquire()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::OK, Json(id)))
}

#[derive(Deserialize)]
struct UsingToken {
    request_token: String,
}

async fn get_displayname(
    State(state): State<App>,
    RequestType(req_type): RequestType,
    ApiRequest(data): ApiRequest<UsingToken>,
) -> axum::response::Result<Json<String>> {
    println!("TODO: add logging!");
    match req_type {
        crate::util::RequestTypeEnum::Html => Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into()),
        crate::util::RequestTypeEnum::Json => {
            let name = get_displayname_from_valid_auth_token(&state, &data.request_token).await;
            let name = name.ok_or(StatusCode::NOT_FOUND.into_response())?;

            Ok(Json(name))
        }
    }
}
