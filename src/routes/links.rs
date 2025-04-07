use std::sync::Arc;

use axum::{
    Form,
    extract::{Path, State},
    response::{Html, IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose};
use chrono::Utc;
use rand::Rng;
use reqwest::{StatusCode, Url};
use rinja_axum::Template;
use serde::{Deserialize, Serialize};

use crate::startup::AppState;

fn generate_id() -> String {
    let random_number = rand::rng().random_range(0..u32::MAX);
    general_purpose::URL_SAFE_NO_PAD.encode(random_number.to_string())
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LinkId {
    id: String,
}

#[derive(Deserialize, Debug, Serialize, Template)]
#[template(path = "get_link.html")]
pub struct LinkTargetTemplate {
    pub id: String,
}

#[derive(Deserialize, Debug, Serialize, Template)]
#[template(path = "redirect.html")]
pub struct LinkRedirectionTemplate {
    pub id: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct LinkTarget {
    pub target_url: String,
}

#[tracing::instrument(
    name = "Redirecting to the target url link",
    skip(requested_link, app_state)
)]
pub async fn link_access_page(
    State(app_state): State<Arc<AppState>>,
    Path(requested_link): Path<String>,
) -> Result<impl IntoResponse, LinkError> {
    let link = sqlx::query_as!(LinkId, "SELECT id FROM links WHERE id = $1", requested_link)
        .fetch_optional(&app_state.pool)
        .await?
        .ok_or(LinkError::LinkNotFound)?;
    let template = LinkTargetTemplate { id: link.id };
    Ok(Html(template.render().unwrap()))
}

#[tracing::instrument(name = "Creating a new link", skip(new_link, app_state))]
pub async fn create_link(
    State(app_state): State<Arc<AppState>>,
    Form(new_link): Form<LinkTarget>,
) -> Result<impl IntoResponse, LinkError> {
    let target_url = Url::parse(&new_link.target_url)
        .map_err(|_| LinkError::InvalidUrl(new_link.target_url))?
        .to_string();

    #[allow(clippy::never_loop)]
    for _ in 1..=3 {
        let new_link_id = generate_id();
        let time_now = Utc::now();
        sqlx::query!(
            r#"
                    insert into links(id, target_url, created_at)
                    values ($1, $2, $3)
                "#,
            &new_link_id,
            &target_url,
            &time_now
        )
        .execute(&app_state.pool)
        .await?;

        let new_link = LinkRedirectionTemplate { id: new_link_id };
        return Ok(new_link.render().unwrap());
    }

    Err(LinkError::GenerateUniqueId)
}

// TODO:
// could be better, leave it for now
#[derive(thiserror::Error, Debug)]
pub enum LinkError {
    #[error("Could not persist new short link. Exhausted all retries of generating a unique id")]
    GenerateUniqueId,
    #[error("couldn't insert new link to the database, sqlx error {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("invalid url")]
    InvalidUrl(String),
    #[error("link is not found in the db")]
    LinkNotFound,
}
impl IntoResponse for LinkError {
    fn into_response(self) -> Response {
        match self {
            LinkError::InvalidUrl(s) => {
                tracing::error!("{}", LinkError::InvalidUrl(s));
                StatusCode::BAD_REQUEST
            }
            LinkError::GenerateUniqueId => {
                tracing::error!("{}", LinkError::GenerateUniqueId);
                StatusCode::BAD_REQUEST
            }
            LinkError::SqlxError(e) => {
                tracing::error!("{}", LinkError::SqlxError(e));
                StatusCode::BAD_REQUEST
            }
            LinkError::LinkNotFound => {
                tracing::error!("{}", LinkError::LinkNotFound);
                StatusCode::BAD_REQUEST
            }
        }
        .into_response()
    }
}
