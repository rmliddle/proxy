use backend::BackendWrapper;

use types::DatabaseOperation;

pub use types::DatabaseError;
pub use types::DatabaseResult;
pub use types::Response;

mod backend;
mod types;

#[derive(Clone, Debug)]
pub struct Database {
    connection: BackendWrapper,
}

impl Database {
    pub async fn new() -> Self {
        Self {
            connection: BackendWrapper::new().await,
        }
    }

    pub async fn query(&self, ns: &str, pk: &str, start: &str) -> Response {
        let operation = DatabaseOperation::RangeQuery(ns.to_string(), pk.to_string(), start.to_string());
        self.run(operation).await
    }

    pub async fn get(&self, ns: &str, pk: &str, sk: &str) -> Response {
        let operation = DatabaseOperation::PointQuery(ns.to_string(), pk.to_string(), sk.to_string());
        self.run(operation).await
    }

    pub async fn insert(&self, ns: &str, pk: &str, sk: &str, data: &str) -> Response {
        let operation = DatabaseOperation::Insert(ns.to_string(), pk.to_string(), sk.to_string(), data.to_string());
        self.run(operation).await
    }

    pub async fn delete(&self, ns: &str, pk: &str, sk: &str) -> Response {
        let operation = DatabaseOperation::Delete(ns.to_string(), pk.to_string(), sk.to_string());
        self.run(operation).await
    }

    async fn run(&self, operation: DatabaseOperation) -> Response {
        Response::from(self.connection.run_operation(operation).await)
    }
}
