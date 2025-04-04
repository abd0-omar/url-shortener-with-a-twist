use axum::response::{Html, IntoResponse};
use rinja_axum::Template;

#[derive(Template)]
#[template(path = "index.html")]
struct FormBaseTemplate {
    title: String,
}

pub async fn index() -> impl IntoResponse {
    let template = FormBaseTemplate {
        title: String::from("url-shortener"),
    };
    Html(template.render().unwrap())
}
