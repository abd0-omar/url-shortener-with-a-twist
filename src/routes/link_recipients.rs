use std::sync::Arc;

use axum::{
    Form,
    extract::{Path, State},
    http::Response,
    response::{Html, IntoResponse},
};
use chrono::Utc;
use rand::{Rng, distr::Alphanumeric, rng};
use reqwest::StatusCode;
use rinja_axum::Template;
use serde::Deserialize;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    domain::{NewRecipient, RecipientEmail, RecipientName},
    email_client::EmailClient,
    startup::AppState,
};

#[derive(Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

impl TryFrom<FormData> for NewRecipient {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = RecipientName::parse(value.name)?;
        let email = RecipientEmail::parse(value.email)?;
        Ok(Self { name, email })
    }
}

#[derive(Template)]
#[template(path = "email_not_verified.html")]
struct EmailNotVerified<'a> {
    email: &'a str,
}

#[tracing::instrument(
    name = "Accessing link with credentials",
    skip(form, app_state),
    fields(
        recipient_name = %form.name,
        recipient_email = %form.email
    )
)]
pub async fn access_link(
    State(app_state): State<Arc<AppState>>,
    Path(link_id): Path<String>,
    Form(form): Form<FormData>,
) -> Result<impl IntoResponse, RecipientError> {
    let new_recipient = form.try_into().map_err(RecipientError::InvalidRecipient)?;

    let recipient_id = match get_recipient(&app_state.pool, &new_recipient).await {
        Ok(recipient_id) => recipient_id,
        Err(_) => {
            return Ok(Html(
                EmailNotVerified {
                    email: &new_recipient.email.as_ref(),
                }
                .render()
                .unwrap(),
            )
            .into_response());
        }
    };

    if check_status(&app_state.pool, recipient_id, &link_id).await? == "confirmed" {
        let target_url = get_target_link(&app_state.pool, &link_id).await?;

        Ok(Response::builder()
            .status(StatusCode::SEE_OTHER) // Temporary redirect
            .header("HX-Redirect", target_url) // HTMX redirect header
            .body(axum::body::Body::empty())
            .unwrap())
    } else {
        Ok((StatusCode::UNAUTHORIZED).into_response())
    }
}

#[derive(Template)]
#[template(path = "success_email.html")]
struct SucessEmail {
    recipient_email: String,
}

#[tracing::instrument(
    name = "Adding a new recipient",
    skip(form, app_state),
    fields(
        recipient_name = %form.name,
        recipient_email = %form.email
    )
)]
pub async fn add_recipient(
    State(app_state): State<Arc<AppState>>,
    Path(requested_link): Path<String>,
    Form(form): Form<FormData>,
) -> Result<impl IntoResponse, RecipientError> {
    let new_recipient = form.try_into().map_err(RecipientError::InvalidRecipient)?;

    let mut transaction = app_state.pool.begin().await?;

    // if it's an already duplicated email, check if it's status confirmed with
    // the requested link
    let recipient_id = match insert_recipient(&mut transaction, &new_recipient).await {
        Ok(recipient_id) => recipient_id,
        Err(e) => {
            match &e {
                sqlx::Error::Database(db_err) => {
                    if db_err.is_unique_violation() {
                        let recipient_id = get_recipient(&app_state.pool, &new_recipient).await?;
                        if check_status(&app_state.pool, recipient_id, &requested_link).await?
                            == "confirmed"
                        {
                            return Ok(String::from(
                                "user already confirmed the link, you should verify",
                            )
                            .into_response());
                        } else {
                            // if the recipient has a registered email but  has not
                            // confirmed the link or recieved a link yet, we can proceed
                            // to send him a new confirmation email
                            recipient_id
                        }
                    } else {
                        return Err(RecipientError::SqlxError(e));
                    }
                }
                _ => return Err(RecipientError::SqlxError(e)),
            }
        }
    };

    let link_token = generate_link_token();

    store_token(&mut transaction, recipient_id, &link_token, requested_link).await?;

    transaction.commit().await?;

    let recipient_email = new_recipient.email.as_ref().to_owned();

    send_confirmation_email(
        &app_state.email_client,
        new_recipient,
        &app_state.base_url.0,
        &link_token,
    )
    .await?;

    Ok(Html(SucessEmail { recipient_email }.render().unwrap()).into_response())
}

fn generate_link_token() -> String {
    let mut rng = rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[tracing::instrument(
    name = "Send a confirmation email to a new recipient",
    skip(email_client, new_recipient, base_url, link_token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_recipient: NewRecipient,
    base_url: &str,
    link_token: &str,
) -> Result<(), reqwest::Error> {
    // TODO: hard coded port, just for testing locally
    // remove it in production as it's not needed with a real domain
    let confirmation_link = format!(
        "{}:8080/link_recipients/confirm?link_token={}",
        base_url, link_token
    );
    let plain_body = format!(
        "Welcome to the close friends url-shortener!\nVisit {} to verify your credentials to visit the link!",
        confirmation_link
    );
    let html_body = format!(
        "Welcome to to the close friends url-shortener!<br />Click <a href=\"{}\">here</a> to verify your credentials to visit the link!",
        confirmation_link
    );
    email_client
        .send_email(
            new_recipient.email,
            "A friend wants to show you something!",
            &html_body,
            &plain_body,
        )
        .await
}

#[derive(thiserror::Error, Debug)]
pub enum RecipientError {
    #[error("invalid recipient, {0}")]
    InvalidRecipient(String),
    #[error("couldn't insert new_recipient to the database, sqlx error {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("couldn't send email, reqwest error {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("duplicate email")]
    DuplicateEmail,
}

impl IntoResponse for RecipientError {
    fn into_response(self) -> axum::response::Response {
        match self {
            RecipientError::SqlxError(e) => {
                tracing::error!("{}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            RecipientError::InvalidRecipient(e) => {
                tracing::error!("{}", e);
                StatusCode::BAD_REQUEST.into_response()
            }
            RecipientError::ReqwestError(e) => {
                tracing::error!("{}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            RecipientError::DuplicateEmail => {
                tracing::error!("Duplicate email error occurred");
                let html = format!(
                    "<h1>Email already registered</h1><p>Please use a different email, or try to sign in</p>"
                );
                (StatusCode::CONFLICT, Html(html)).into_response()
            }
        }
    }
}

#[tracing::instrument(
    name = "Check if the recipient has confirmed the link before",
    skip(recipient_id, requested_link, pool)
)]
pub async fn check_status(
    pool: &PgPool,
    recipient_id: Uuid,
    requested_link: &str,
) -> Result<String, sqlx::Error> {
    let query = sqlx::query!(
        r#"
    SELECT status FROM links_tokens WHERE recepient_id = $1 AND link_id = $2
            "#,
        recipient_id,
        requested_link
    )
    .fetch_optional(pool)
    .await?
    .map(|record_row| record_row.status)
    .unwrap_or("no status found".to_string());

    Ok(query)
}

#[tracing::instrument(name = "Get target url link", skip(pool))]
pub async fn get_target_link(pool: &PgPool, link_id: &str) -> Result<String, sqlx::Error> {
    let target_url = sqlx::query!(
        r#"
    SELECT target_url FROM links WHERE id = $1
            "#,
        link_id
    )
    .fetch_one(pool)
    .await?
    .target_url;
    Ok(target_url)
}

#[tracing::instrument(
    name = "getting recipient id from the database",
    skip(new_recipient, pool)
)]
pub async fn get_recipient(
    pool: &PgPool,
    new_recipient: &NewRecipient,
) -> Result<Uuid, sqlx::Error> {
    let query = sqlx::query!(
        r#"
    SELECT id FROM link_recipients WHERE email = $1 AND name = $2
            "#,
        new_recipient.email.as_ref(),
        new_recipient.name.as_ref(),
    )
    .fetch_one(pool)
    .await?;
    Ok(query.id)
}

#[tracing::instrument(
    name = "Saving new recipient details in the database",
    skip(new_recipient, transaction)
)]
pub async fn insert_recipient(
    transaction: &mut Transaction<'_, Postgres>,
    new_recipient: &NewRecipient,
) -> Result<Uuid, sqlx::Error> {
    let recipient_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
    INSERT INTO link_recipients (id, email, name, received_link_at)
    VALUES ($1, $2, $3, $4)
            "#,
        recipient_id,
        new_recipient.email.as_ref(),
        new_recipient.name.as_ref(),
        Utc::now()
    );
    transaction.execute(query).await?;
    Ok(recipient_id)
}

#[tracing::instrument(
    name = "Store link token in the database",
    skip(link_token, transaction)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    recipient_id: Uuid,
    link_token: &str,
    requested_link: String,
) -> Result<(), sqlx::Error> {
    // get requested_link id from links table
    // insert into links_tokens table
    let after_week = Utc::now() + chrono::Duration::weeks(1);
    let query = sqlx::query!(
        r#"
    INSERT INTO links_tokens (link_token , recepient_id , link_id , status , expiration_date )
    VALUES ($1, $2, $3, $4, $5)
        "#,
        link_token,
        recipient_id,
        requested_link,
        "pending".to_string(),
        after_week
    );
    transaction.execute(query).await?;
    Ok(())
}
