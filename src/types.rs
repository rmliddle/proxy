use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tracing::warn;

pub static PAGE_SIZE: usize = 100;

#[derive(Clone, Debug)]
pub enum DatabaseOperation {
    //(Namespace, Partition Key, Range Key, Data)
    Insert(String, String, String, String),

    //(Namespace, Partition Key, Range Key)
    Delete(String, String, String),

    //(Namespace, Partition Key, Start cursor — empty string starts at the beginning)
    RangeQuery(String, String, String),

    //(Namespace, Partition Key, Range Key)
    PointQuery(String, String, String),
}

#[derive(Clone, Debug)]
pub enum DatabaseValue {
    NoValue,
    SingleValue(Option<String>),
    MultipleValues(Vec<(String, String)>),
}

pub type DatabaseResult = Result<DatabaseValue, DatabaseError>;

#[derive(Clone, Debug)]
pub enum DatabaseError {
    StorageError,
    ValueError,
}

impl From<tokio_postgres::Error> for DatabaseError {
    fn from(value: tokio_postgres::Error) -> Self {
        warn!("Postgres error: {}", value);
        DatabaseError::StorageError
    }
}

impl From<deadpool_postgres::PoolError> for DatabaseError {
    fn from(value: deadpool_postgres::PoolError) -> Self {
        warn!("Postgres pool error: {}", value);
        DatabaseError::StorageError
    }
}

impl From<serde_json::Error> for DatabaseError {
    fn from(value: serde_json::Error) -> Self {
        warn!("Error serializing/deserializing data: {}", value);
        DatabaseError::ValueError
    }
}

pub struct Response(DatabaseResult);

impl Response {
    pub fn into_inner(self) -> DatabaseResult {
        self.0
    }
}

impl From<DatabaseResult> for Response {
    fn from(value: DatabaseResult) -> Self {
        Self(value)
    }
}

impl IntoResponse for Response {
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            Err(err) => err.into_response(),
            Ok(value) => value.into_response(),
        }
    }
}

impl IntoResponse for DatabaseValue {
    fn into_response(self) -> axum::response::Response {
        match self {
            DatabaseValue::NoValue => StatusCode::OK.into_response(),
            DatabaseValue::SingleValue(option) => SingleValueResponse::from(option).into_response(),
            DatabaseValue::MultipleValues(values) => {
                MultiValueResponse::from(values).into_response()
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct SingleValueResponse {
    result: Option<String>,
}

impl From<Option<String>> for SingleValueResponse {
    fn from(result: Option<String>) -> Self {
        Self { result }
    }
}

impl IntoResponse for SingleValueResponse {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct MultiValueResponse {
    result: Vec<String>,
    next: Option<String>,
}

impl IntoResponse for MultiValueResponse {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

impl From<Vec<(String, String)>> for MultiValueResponse {
    fn from(rows: Vec<(String, String)>) -> Self {
        let next = if rows.len() == PAGE_SIZE {
            rows.last().map(|(range, _)| range.clone())
        } else {
            None
        };
        let result = rows.into_iter().map(|(_, data)| data).collect();
        Self { result, next }
    }
}

impl IntoResponse for DatabaseError {
    fn into_response(self) -> axum::response::Response {
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}
