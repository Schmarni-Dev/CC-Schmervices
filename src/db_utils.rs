use anyhow::Ok;
use libsql_client::{Client, Statement};

use crate::App;

pub async fn get_strings_from_db(
    app: &App,
    statement: impl Into<Statement> + Send,
) -> anyhow::Result<Vec<String>> {
    let data = app
        .db
        .lock()
        .await
        .execute(statement)
        .await?
        .rows
        .iter()
        .flat_map(|row| row.values.iter())
        .filter_map(|value| match value {
            libsql_client::Value::Text { value } => Some(value.to_owned()),
            _ => None,
        })
        .collect::<Vec<String>>();
    Ok(data)
}
pub async fn get_ints_from_db(
    app: &App,
    statement: impl Into<Statement> + Send,
) -> anyhow::Result<Vec<i64>> {
    let data = app
        .db
        .lock()
        .await
        .execute(statement)
        .await?
        .rows
        .iter()
        .flat_map(|row| row.values.iter())
        .filter_map(|value| match value {
            libsql_client::Value::Integer { value } => Some(value.to_owned()),
            _ => None,
        })
        .collect::<Vec<i64>>();
    Ok(data)
}
pub async fn get_displayname_from_username(app: &App, username: &str) -> anyhow::Result<String> {
    get_string_from_db(
        app,
        Statement::with_args(
            "SELECT display_name FROM users WHERE username = ?;",
            &[username],
        ),
    )
    .await
    .map_err(|_| anyhow::anyhow!("no Display Name?!"))
}

pub async fn get_string_from_db(
    db: &App,
    statement: impl Into<Statement> + Send,
) -> anyhow::Result<String> {
    match get_strings_from_db(db, statement).await?.first() {
        Some(value) => Ok(value.to_string()),
        None => anyhow::bail!("no string"),
    }
}

pub async fn setup_db(db: &Client) -> anyhow::Result<()> {
    db.execute(
        "
        CREATE TABLE IF NOT EXISTS system (
            key INTEGER PRIMARY KEY UNIQUE DEFAULT 0,
            visits INTEGER DEFAULT 0
        );
        ",
    )
    .await?;
    db.execute(
        "
        CREATE TABLE IF NOT EXISTS users (
            username TEXT NOT NULL PRIMARY KEY UNIQUE,
            display_name TEXT NOT NULL,
            secret TEXT NOT NULL,
            money INTEGER NOT NULL,
            otp_verified INTEGER NOT NULL
        );
        ",
    )
    .await?;
    db.execute(
        "
        CREATE TABLE IF NOT EXISTS auth_tokens (
            token TEXT NOT NULL PRIMARY KEY UNIQUE,
            username TEXT NOT NULL,
            expire_timestamp INTEGER NOT NULL
        );
        ",
    )
    .await?;
    db.execute(
        "
        CREATE TABLE IF NOT EXISTS transactions (
            id TEXT NOT NULL PRIMARY KEY,
            buyer TEXT NOT NULL,
            seller TEXT NOT NULL,
            name TEXT NOT NULL,
            amount INTEGER NOT NULL,
            accepted INTEGER NOT NULL,
            timestamp INTEGER NOT NULL
        );
        ",
    )
    .await?;

    let _ = db.execute("INSERT INTO system VALUES (0,0)").await;
    Ok(())
}
