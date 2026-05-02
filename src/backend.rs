use deadpool_postgres::{Config, Pool, Runtime};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use tracing::trace;

use crate::types::{DatabaseError, DatabaseOperation, DatabaseResult, DatabaseValue, PAGE_SIZE};

static SCHEMA_SETUP: &str =
    "CREATE TABLE IF NOT EXISTS records (namespace TEXT NOT NULL, partition TEXT NOT NULL, range TEXT NOT NULL, data TEXT NOT NULL);";
static UNIQUE_INDEX_SETUP: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS unique_record_idx ON records (namespace, partition, range) INCLUDE (data);";

pub static POINT_QUERY: &str =
    "SELECT data FROM records WHERE namespace = $1 AND partition = $2 AND range = $3";
pub static RANGE_QUERY: &str =
    "SELECT range, data FROM records WHERE namespace = $1 AND partition = $2 AND range > $3 ORDER BY range LIMIT $4";
pub static INSERT_STATEMENT: &str =
    "INSERT INTO records (namespace, partition, range, data) VALUES ($1, $2, $3, $4) ON CONFLICT (namespace, partition, range) DO UPDATE SET data = EXCLUDED.data WHERE records.data IS DISTINCT FROM EXCLUDED.data";
pub static DELETE_STATEMENT: &str =
    "DELETE FROM records WHERE namespace = $1 AND partition = $2 AND range = $3";
pub static DATABASE_URL_ENV: &str = "DATABASE_URL";

#[derive(Clone, Debug)]
pub struct BackendWrapper {
    pool: Pool,
}

impl BackendWrapper {
    pub async fn new() -> Self {
        let url = std::env::var(DATABASE_URL_ENV)
            .expect("DATABASE_URL environment variable must be set");
        let mut cfg = Config::new();
        cfg.url = Some(url);
        let tls = MakeTlsConnector::new(
            TlsConnector::new().expect("Failed to build native-tls connector"),
        );
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1), tls)
            .expect("Failed to build Postgres pool");

        let client = pool
            .get()
            .await
            .expect("Failed to acquire Postgres connection");
        client
            .batch_execute(SCHEMA_SETUP)
            .await
            .expect("Schema setup failed");
        client
            .batch_execute(UNIQUE_INDEX_SETUP)
            .await
            .expect("Unique index setup failed");

        BackendWrapper { pool }
    }

    pub async fn run_operation(&self, operation: DatabaseOperation) -> DatabaseResult {
        let value = match operation {
            DatabaseOperation::Insert(ns, pk, sk, data) => self
                .insert(&ns, &pk, &sk, &data)
                .await
                .map(|_| DatabaseValue::NoValue)?,
            DatabaseOperation::Delete(ns, pk, sk) => self
                .delete(&ns, &pk, &sk)
                .await
                .map(|_| DatabaseValue::NoValue)?,
            DatabaseOperation::RangeQuery(ns, pk, start) => self
                .query_range(&ns, &pk, &start)
                .await
                .map(DatabaseValue::MultipleValues)?,
            DatabaseOperation::PointQuery(ns, pk, sk) => self
                .query_point(&ns, &pk, &sk)
                .await
                .map(DatabaseValue::SingleValue)?,
        };
        Ok(value)
    }

    async fn query_range(
        &self,
        ns: &str,
        pk: &str,
        start: &str,
    ) -> Result<Vec<(String, String)>, DatabaseError> {
        trace!("Range query pk: {} start: {:?}", pk, start);
        let limit: i64 = PAGE_SIZE as i64;
        let client = self.pool.get().await?;
        let stmt = client.prepare_cached(RANGE_QUERY).await?;
        let rows = client.query(&stmt, &[&ns, &pk, &start, &limit]).await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get(0), row.get(1)))
            .collect())
    }

    async fn query_point(
        &self,
        ns: &str,
        pk: &str,
        sk: &str,
    ) -> Result<Option<String>, DatabaseError> {
        trace!("Point query pk: {} sk: {}", pk, sk);
        let client = self.pool.get().await?;
        let stmt = client.prepare_cached(POINT_QUERY).await?;
        let row = client.query_opt(&stmt, &[&ns, &pk, &sk]).await?;
        Ok(row.map(|r| r.get(0)))
    }

    async fn insert(
        &self,
        ns: &str,
        pk: &str,
        sk: &str,
        data: &str,
    ) -> Result<(), DatabaseError> {
        trace!("Inserting record with pk: {} and sk: {}", pk, sk);
        let client = self.pool.get().await?;
        let stmt = client.prepare_cached(INSERT_STATEMENT).await?;
        client.execute(&stmt, &[&ns, &pk, &sk, &data]).await?;
        Ok(())
    }

    async fn delete(&self, ns: &str, pk: &str, sk: &str) -> Result<(), DatabaseError> {
        trace!("Deleting record with pk: {} and sk: {}", pk, sk);
        let client = self.pool.get().await?;
        let stmt = client.prepare_cached(DELETE_STATEMENT).await?;
        client.execute(&stmt, &[&ns, &pk, &sk]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires a running Postgres instance"]
    async fn test_query_no_records() {
        let database = BackendWrapper::new().await;
        let operation = DatabaseOperation::RangeQuery(
            String::from("ns"),
            String::from("pk"),
            String::new(),
        );
        let result = database.run_operation(operation).await.unwrap();
        assert!(matches!(result, DatabaseValue::MultipleValues(_)));
    }
}
