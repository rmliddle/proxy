use std::env;

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{middleware, response, Router};
use proxy::{Database, Response};
use serde::Deserialize;
use tracing::info;

const API_KEY_ENV_VAR: &str = "API_KEY";

// Caps the `data` payload so that an index tuple in `unique_record_idx`
// (namespace, partition, range) INCLUDE (data) stays under Postgres's ~2712-byte B-tree limit.
// Leaves ~600 bytes of headroom for the three key columns + per-tuple overhead.
const MAX_DATA_BYTES: usize = 2048;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Starting application");

    let database = Database::new().await;

    let key = env::var(API_KEY_ENV_VAR).expect("API_KEY env var must be set");
    let data = Router::new()
        .route("/put/:pk/:sk", post(put_record))
        .route("/get/:pk/:sk", get(get_record))
        .route("/delete/:pk/:sk", delete(delete_record))
        .route("/query/:pk", get(query_records))
        .with_state(database)
        .route_layer(middleware::from_fn_with_state(key, authenticate));

    let app = Router::new()
        .route("/", get(get_health))
        .nest("/:ns", data);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    info!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[axum::debug_handler]
async fn get_health() -> impl IntoResponse {
    (StatusCode::OK, "Success")
}

#[axum::debug_handler]
async fn put_record(
    Path((ns, pk, sk)): Path<(String, String, String)>,
    State(database): State<Database>,
    body: String,
) -> Result<Response, (StatusCode, String)> {
    if body.len() > MAX_DATA_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "data field exceeds maximum size of {} bytes (received {} bytes)",
                MAX_DATA_BYTES,
                body.len()
            ),
        ));
    }
    Ok(database.insert(&ns, &pk, &sk, &body).await)
}

#[axum::debug_handler]
async fn get_record(
    Path((ns, pk, sk)): Path<(String, String, String)>,
    State(database): State<Database>,
) -> Response {
    database.get(&ns, &pk, &sk).await
}

#[axum::debug_handler]
async fn delete_record(
    Path((ns, pk, sk)): Path<(String, String, String)>,
    State(database): State<Database>,
) -> Response {
    database.delete(&ns, &pk, &sk).await
}

#[derive(Deserialize)]
struct QueryParams {
    #[serde(default)]
    start: String,
}

#[axum::debug_handler]
async fn query_records(
    Path((ns, pk)): Path<(String, String)>,
    Query(QueryParams { start }): Query<QueryParams>,
    State(database): State<Database>,
) -> Response {
    database.query(&ns, &pk, &start).await
}

async fn authenticate(
    State(key): State<String>,
    req: Request,
    next: Next,
) -> Result<response::Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    if !valid_header(key, auth_header) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(req).await)
}

fn valid_header(key: String, header: Option<&str>) -> bool {
    if let Some(value) = header {
        return value == key;
    }
    return false;
}
