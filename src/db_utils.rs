use crate::{App, DBPool};

pub async fn get_displayname_from_username(app: &App, username: &str) -> eyre::Result<String> {
    let r = sqlx::query!(
        "SELECT display_name FROM users WHERE username = ?;",
        username
    )
    .fetch_one(&mut *app.db.acquire().await?)
    .await?;
    Ok(r.display_name)
}
