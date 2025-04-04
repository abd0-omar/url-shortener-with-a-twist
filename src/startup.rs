use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::Request,
    response::Response,
    routing::{get, post},
    serve::Serve,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::net::TcpListener;
use tower_http::{services::ServeFile, trace::TraceLayer};
use tracing::{Span, info, info_span};
use uuid::Uuid;

use crate::{
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes::{
        access_link, add_recipient, confirm, create_link, health_check, index, link_access_page,
    },
};

pub struct ApplicationBaseUrl(pub String);

pub struct AppState {
    pub pool: PgPool,
    pub email_client: EmailClient,
    pub base_url: ApplicationBaseUrl,
}

pub async fn run(
    listener: TcpListener,
    pool: PgPool,
    email_client: EmailClient,
    base_url: String,
) -> anyhow::Result<Serve<TcpListener, Router, Router>> {
    // Wrapped in an Arc pointer to allow cheap cloning of AppState across handlers.
    // This prevents unnecessary cloning of EmailClient, which has two String fields,
    // since cloning an Arc is negligible.
    let app_state = Arc::new(AppState {
        pool,
        email_client,
        base_url: ApplicationBaseUrl(base_url),
    });
    let app = Router::new()
        .route("/", get(index))
        .route("/health_check", get(health_check))
        .route("/create", post(create_link))
        .route("/{id}", get(link_access_page))
        .route("/link_recipients/{id}", post(add_recipient))
        .route("/get_link/{id}", post(access_link))
        .route("/link_recipients/confirm", get(confirm))
        .nest_service("/templates", ServeFile::new("templates/output.css"))
        .with_state(app_state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let request_id = Uuid::new_v4();
                    info_span!(
                        "http_request",
                        method = ?request.method(),
                        uri = ?request.uri(),
                        version = ?request.version(),
                        request_id = ?request_id,
                    )
                })
                .on_response(|response: &Response, latency: Duration, span: &Span| {
                    let status = response.status();
                    let headers = response.headers();
                    span.record("status", &status.as_u16());
                    info!(parent: span, ?status, ?headers, ?latency, "Response sent");
                }),
        );

    Ok(axum::serve(listener, app))
}

pub struct Application {
    port: u16,
    server: Serve<TcpListener, Router, Router>,
}

impl Application {
    // build is the one that invokes the `run()` function
    // then any fn invokes `run_until_stopped`
    pub async fn build(configuration: Settings) -> anyhow::Result<Self> {
        let connection_pool = get_connection_pool(&configuration.database);

        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email address.");
        let timeout = configuration.email_client.timeout();
        let email_client = EmailClient::new(
            sender_email,
            configuration.email_client.base_url,
            configuration.email_client.authorization_token,
            timeout,
        );

        let listener = TcpListener::bind(format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        ))
        .await?;
        let port = listener.local_addr()?.port();

        let server = run(
            listener,
            connection_pool,
            email_client,
            configuration.application.base_url,
        )
        .await
        .unwrap();

        Ok(Self { server, port })
    }

    pub async fn run_until_stopped(self) -> anyhow::Result<()> {
        Ok(self.server.await?)
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new().connect_lazy_with(configuration.connect_options())
}
