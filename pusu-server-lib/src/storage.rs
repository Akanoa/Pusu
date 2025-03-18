use foundationdb::Database;
use std::sync::Arc;

#[derive(Clone)]
pub struct Storage {
    database: Arc<Database>,
}

impl Storage {
    pub fn new(database: Arc<Database>) -> Self {
        Self { database }
    }

    /// Sets a key-value pair in the FoundationDB database.
    ///
    /// # Parameters
    ///
    /// * `key`: A byte slice representing the key to store in the database.
    /// * `value`: A byte slice representing the value associated with the key.
    ///
    /// # Returns
    ///
    /// Returns a `Result` which is `Ok(())` if the operation succeeds, or an error of type `crate::errors::Error`
    /// if the operation fails.
    ///
    /// # Errors
    ///
    /// This method will return an error if the transaction to set the key-value
    /// pair in FoundationDB cannot be completed.
    pub async fn set(&self, key: &[u8], value: &[u8]) -> crate::errors::Result<()> {
        self.database
            .run(|trx, _| async move {
                trx.set(key, value);
                Ok(())
            })
            .await?;
        Ok(())
    }

    /// Retrieves the value associated with the given key from the FoundationDB database.
    ///
    /// # Parameters
    ///
    /// * `key`: A byte slice representing the key to retrieve from the database.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing either:
    /// - `Ok(Some(Vec<u8>))`: If the key exists in the database, the value is returned as a `Vec<u8>`.
    /// - `Ok(None)`: If the key does not exist in the database.
    /// - `Err`: If there is an error during database communication.
    ///
    /// # Errors
    ///
    /// This method will return an error if the transaction to retrieve the value
    /// from the FoundationDB database cannot be completed.
    pub async fn get(&self, key: &[u8]) -> crate::errors::Result<Option<Vec<u8>>> {
        let value = self
            .database
            .run(|trx, _| async move { Ok(trx.get(key, true).await?) })
            .await?;
        let value = value.map(|v| v.to_vec());
        Ok(value)
    }

    /// Deletes a key-value pair from the FoundationDB database.
    ///
    /// # Parameters
    ///
    /// * `key`: A byte slice representing the key to be deleted from the database.
    ///
    /// # Returns
    ///
    /// Returns a `Result` which is `Ok(())` if the operation succeeds, or an error of type `crate::errors::Error`
    /// if the operation fails.
    ///
    /// # Errors
    ///
    /// This method will return an error if the transaction to delete the key-value
    /// pair from the FoundationDB database cannot be completed.
    pub async fn delete(&self, key: &[u8]) -> crate::errors::Result<()> {
        self.database
            .run(|trx, _| async move {
                trx.clear(key);
                Ok(())
            })
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdb_testcontainer::get_db_once;

    #[tokio::test]
    async fn test_database() {
        let _guard = get_db_once().await;
        let storage = Storage::new(_guard.clone());

        storage
            .set(b"key", b"value")
            .await
            .expect("Unable to set key");
        let result = storage.get(b"key").await.expect("Unable to get key");
        assert_eq!(result, Some(b"value".to_vec()));
        storage.delete(b"key").await.expect("Unable to delete key");
        let result = storage
            .get(b"key")
            .await
            .expect("Unable to get key after delete");
        assert!(result.is_none());
    }
}
