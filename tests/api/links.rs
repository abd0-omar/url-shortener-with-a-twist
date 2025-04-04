use reqwest::StatusCode;
use url_shortener_with_a_twist::routes::LinkTarget;
use wiremock::{
    Mock, ResponseTemplate,
    matchers::{method, path},
};

use crate::helpers::{FormData, spawn_app};

#[tokio::test]
async fn create_link_returns_200_for_valid_url() {
    // Arrange
    let app = spawn_app().await;
    let body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };

    // Act
    let (response, _) = app.post_links(body).await;

    // Assert
    let saved = sqlx::query!("SELECT target_url FROM links")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved link.");

    assert_eq!(saved.target_url, "https://www.example.com/");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn create_link_returns_400_for_invalid_url() {
    // Arrange
    let app = spawn_app().await;
    let body = LinkTarget {
        target_url: String::from("definetly-not-a-valid-url"),
    };
    // Act

    let (response, _) = app.post_links(body).await;

    // Assert
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn give_access_to_target_link_200() {
    // Arrange
    let app = spawn_app().await;

    let body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let (_, link_id) = app.post_links(body).await;

    let body = FormData {
        name: Some("johnny"),
        email: Some("depp@yahoo.com"),
    };

    app.post_link_recipeints(body, &link_id).await;

    let body = FormData {
        name: Some("johnny"),
        email: Some("depp@yahoo.com"),
    };

    let saved = sqlx::query!("SELECT id, target_url FROM links")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved link.");

    // Act

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);
    let token = confirmation_links
        .html
        .query_pairs()
        .next()
        .unwrap()
        .1
        .to_string();

    // confirm it's status, manually
    let _query = sqlx::query!(
        "update links_tokens set status = 'confirmed' where link_token  = $1",
        token
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to update link token status.");

    let response = reqwest::Client::builder()
        .build()
        .unwrap()
        .post(&format!("{}/get_link/{}", &app.address, saved.id))
        .form(&body)
        .send()
        .await
        .expect("Failed to send request");

    // Assert
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn redirect_returns_400_for_nonexistent_link() {
    let app = spawn_app().await;
    let response = reqwest::Client::new()
        .get(&format!("{}/nonexistent", &app.address))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
