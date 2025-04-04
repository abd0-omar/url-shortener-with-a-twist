use std::sync::Arc;

use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
};
use reqwest::StatusCode;
use rinja_axum::Template;
use sqlx::PgPool;

use crate::startup::AppState;

#[derive(serde::Deserialize)]
pub struct Parameters {
    link_token: String,
}

#[tracing::instrument(name = "Confirm a pending recepient", skip(parameters, app_state))]
pub async fn confirm(
    State(app_state): State<Arc<AppState>>,
    parameters: Query<Parameters>,
) -> impl IntoResponse {
    let _link_id = match confirm_recipient(&app_state.pool, &parameters.link_token).await {
        Ok(link) => link,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    Html(EmailVerifiedSuccessTemplate.render().unwrap()).into_response()
}

#[derive(Template)]
#[template(path = "email_verified_success.html")]
struct EmailVerifiedSuccessTemplate;

#[tracing::instrument(name = "Mark link_token as confirmed", skip(link_token, pool))]
pub async fn confirm_recipient(pool: &PgPool, link_token: &str) -> Result<String, sqlx::Error> {
    let link = sqlx::query!(
        r#"UPDATE links_tokens SET status = 'confirmed' WHERE link_token = $1 RETURNING link_id"#,
        link_token
    )
    .fetch_one(pool)
    .await?
    .link_id;

    Ok(link)
}
