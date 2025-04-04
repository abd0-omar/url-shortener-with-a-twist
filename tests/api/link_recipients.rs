use crate::helpers::{FormData, spawn_app};
use url_shortener_with_a_twist::routes::LinkTarget;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn add_recipient_returns_a_200_for_valid_form_data() {
    // Arrange
    let app = spawn_app().await;
    let body = FormData {
        name: Some("hamada"),
        email: Some("hamada@yahoo.com"),
    };

    let links_body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };
    let (_, short_id) = app.post_links(links_body).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // Act
    let response = app.post_link_recipeints(body, &short_id).await;

    // Assert
    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn add_recipient_persists_the_new_recipient() {
    // Arrange
    let app = spawn_app().await;
    let body = FormData {
        name: Some("hamada"),
        email: Some("hamada@yahoo.com"),
    };

    let links_body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };
    let (_, short_id) = app.post_links(links_body).await;

    // Act
    app.post_link_recipeints(body, &short_id).await;

    // Assert
    let saved = sqlx::query!("SELECT name, email FROM link_recipients",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved recipient.");
    // TODO:
    // get status from links_tokens
    assert_eq!(saved.name, "hamada");
    assert_eq!(saved.email, "hamada@yahoo.com");
    let query = sqlx::query!("SELECT status FROM links_tokens")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved recipient.");
    assert_eq!(query.status, "pending");
}

#[tokio::test]
async fn add_recipient_sends_a_confirmation_email_for_valid_data() {
    // Arrange
    let app = spawn_app().await;
    let body = FormData {
        name: Some("hamada"),
        email: Some("hamada@yahoo.com"),
    };

    let links_body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };
    let (_, short_id) = app.post_links(links_body).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act
    app.post_link_recipeints(body, &short_id).await;

    // Assert
    // Mock asserts on drop
}

#[tokio::test]
async fn add_recipient_sends_a_confirmation_email_with_a_link() {
    // Arrange
    let app = spawn_app().await;
    let body = FormData {
        name: Some("hamada"),
        email: Some("hamada@yahoo.com"),
    };

    let links_body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };
    let (_, short_id) = app.post_links(links_body).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // Act
    app.post_link_recipeints(body, &short_id).await;

    // Assert
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    // The two links should be identical
    assert_eq!(confirmation_links.html, confirmation_links.plain_text);
}

#[tokio::test]
async fn add_recipient_returns_a_422_when_data_is_missing() {
    // Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        (
            FormData {
                name: Some("le guin"),
                email: None,
            },
            "missing email",
        ),
        (
            FormData {
                name: None,
                email: Some("ursula_le_guin@gmail.com"),
            },
            "missing name",
        ),
        (
            FormData {
                name: None,
                email: None,
            },
            "missing both name and email",
        ),
    ];
    let links_body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };
    let (_, short_id) = app.post_links(links_body).await;

    for (invalid_body, error_message) in test_cases {
        // Act
        let response = app
            .post_link_recipeints(invalid_body.into(), &short_id)
            .await;

        // Assert
        assert_eq!(
            422,
            response.status().as_u16(),
            // Additional customised error message on test failure
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

#[tokio::test]
async fn add_recipient_returns_a_400_when_fields_are_present_but_invalid() {
    // Arrange
    let app = spawn_app().await;
    let test_cases = vec![
        (
            FormData {
                name: Some(""),
                email: Some("hamada@yahoo.com"),
            },
            "empty name",
        ),
        (
            FormData {
                name: Some("hamada"),
                email: Some(""),
            },
            "empty email",
        ),
        (
            FormData {
                name: Some("hamada"),
                email: Some("definitely-not-blitzcrank-an-email"),
            },
            "invalid email",
        ),
    ];

    let links_body = LinkTarget {
        target_url: String::from("https://www.example.com"),
    };
    let (_, short_id) = app.post_links(links_body).await;

    for (body, description) in test_cases {
        // Act
        let response = app.post_link_recipeints(body, &short_id).await;

        // Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 400 Bad Request when the payload was {}.",
            description
        );
    }
}
