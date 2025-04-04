use crate::helpers::{FormData, spawn_app};
use url_shortener_with_a_twist::routes::LinkTarget;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    // Arrange
    let app = spawn_app().await;

    // Act
    let response = reqwest::get(&format!("{}/link_recipients/confirm", app.address))
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn the_link_returned_by_add_recipient_returns_a_200_if_called() {
    // Arrange
    let app = spawn_app().await;
    let body = FormData {
        name: Some("hamada"),
        email: Some("hamada@yahoo.com"),
    };

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let links_body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };
    let (_, short_id) = app.post_links(links_body).await;

    app.post_link_recipeints(body, &short_id).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    // Act
    let response = reqwest::get(confirmation_links.html).await.unwrap();

    // Assert
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn clicking_on_the_confirmation_link_confirms_a_recipient() {
    // Arrange
    let app = spawn_app().await;
    let body = FormData {
        name: Some("hamada"),
        email: Some("hamada@yahoo.com"),
    };

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let links_body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };
    let (_, short_id) = app.post_links(links_body).await;

    app.post_link_recipeints(body, &short_id).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    // Act
    let query = sqlx::query!("Select status From links_tokens")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved recipient.");
    assert_eq!(query.status, "pending");
    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // Assert
    let saved = sqlx::query!("SELECT email, name FROM link_recipients",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved recipient.");

    assert_eq!(saved.name, "hamada");
    assert_eq!(saved.email, "hamada@yahoo.com");
    let query = sqlx::query!("Select status From links_tokens")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved recipient.");
    assert_eq!(query.status, "confirmed");
}
