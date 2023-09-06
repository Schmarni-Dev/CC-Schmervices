use axum::{
    body::Body,
    extract::{ws::Message, Path, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use libsql_client::{args, Statement};
use serde::Deserialize;
use serde_json::json;

use crate::{
    auth::{AuthUser, RequestType},
    util::{get_displayname_from_valid_auth_token, get_random_string, ApiRequest},
    App,
};

pub fn get_router() -> Router<App, Body> {
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
    match state.transaction_notif_sockets.lock().await.remove(id) {
        Some(mut socket) => {
            _ = socket.send(Message::Text(msg.to_owned())).await;
            _ = socket.close().await;
        }
        None => {},
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
    let can_read_status = state
        .db
        .lock()
        .await
        .execute(Statement::with_args(
            "SELECT 1 FROM transactions WHERE seller = ? OR buyer = ?;",
            args!(&user, &user),
        ))
        .await
        .is_ok_and(|o| {
            o.rows.into_iter().flat_map(|v| v.values).any(|v| match v {
                libsql_client::Value::Integer { value } => value == 1,
                _ => false,
            })
        });
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

    let exits = state
        .db
        .lock()
        .await
        .execute(Statement::with_args(
            "SELECT 1 FROM transactions WHERE id= ? AND buyer = ?;",
            args!(&transaction_id, &user),
        ))
        .await
        .is_ok_and(|o| {
            o.rows.into_iter().flat_map(|v| v.values).any(|v| match v {
                libsql_client::Value::Integer { value } => value == 1,
                _ => false,
            })
        });
    if !exits {
        Err(StatusCode::NOT_FOUND)?;
    }
    state
        .db
        .lock()
        .await
        .execute(Statement::with_args(
            "UPDATE transactions SET accepted = 1 WHERE id = ? AND buyer=?;",
            args!(&transaction_id, &user),
        ))
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

    let exits = state
        .db
        .lock()
        .await
        .execute(Statement::with_args(
            "SELECT 1 FROM transactions WHERE id= ? AND buyer = ?;",
            args!(&transaction_id, &user),
        ))
        .await
        .is_ok_and(|o| {
            o.rows.into_iter().flat_map(|v| v.values).any(|v| match v {
                libsql_client::Value::Integer { value } => value == 1,
                _ => false,
            })
        });
    if !exits {
        Err(StatusCode::NOT_FOUND)?;
    }
    state
        .db
        .lock()
        .await
        .execute(Statement::with_args(
            "UPDATE transactions SET accepted = 2 WHERE id = ? AND buyer=?;",
            args!(&transaction_id, &user),
        ))
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
    let id = get_random_string(8);
    state
        .db
        .lock()
        .await
        .execute(Statement::with_args(
            "INSERT INTO transactions VALUES (?,?,?,?,?,0,?)",
            args!(
                &id,
                &data.buyer,
                &user,
                &data.name,
                data.amount,
                chrono::Utc::now().timestamp()
            ),
        ))
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
    match req_type {
        crate::util::RequestTypeEnum::Html => Err(StatusCode::UNSUPPORTED_MEDIA_TYPE.into()),
        crate::util::RequestTypeEnum::Json => {
            let name = get_displayname_from_valid_auth_token(&state, &data.request_token).await;
            let name = name.ok_or(StatusCode::NOT_FOUND.into_response())?;

            Ok(Json(name))
        }
    }
}
