use anyhow::Result;
use async_trait::async_trait;
use borsh::{
    BorshDeserialize,
    BorshSerialize,
};
use futures::Stream;

#[async_trait]
pub(crate) trait StateRead: Send + Sync {
    /// Gets a value from the verifiable key-value store.
    async fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<str>,
        V: BorshDeserialize;

    /// Gets a value from the non-verifiable key-value store.
    async fn nonverifiable_get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]>,
        V: BorshDeserialize;

    /// Retrieves all keys (but not values) matching a prefix from the verifiable key-value store.
    fn prefix_keys<K>(&self, prefix: K) -> impl Stream<Item = Result<String>> + Send + 'static
    where
        K: AsRef<str>;

    /// Retrieves all key-value pairs for keys matching a prefix from the non-verifiable key-value
    /// store.
    fn nonverifiable_prefix<K, V>(
        &self,
        prefix: K,
    ) -> impl Stream<Item = Result<(Vec<u8>, V)>> + Send + 'static
    where
        K: AsRef<[u8]>,
        V: BorshDeserialize + Send + 'static;
}

impl<'a, S: StateRead + Send + Sync> StateRead for &'a mut S {
    async fn get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<str>,
        V: BorshDeserialize,
    {
        (**self).get(key).await
    }

    async fn nonverifiable_get<K, V>(&self, key: K) -> Result<Option<V>>
    where
        K: AsRef<[u8]>,
        V: BorshDeserialize,
    {
        (**self).nonverifiable_get(key).await
    }

    fn prefix_keys<K>(&self, prefix: K) -> impl Stream<Item = Result<String>> + Send + 'static
    where
        K: AsRef<str>,
    {
        (**self).prefix_keys(prefix)
    }

    fn nonverifiable_prefix<K, V>(
        &self,
        prefix: K,
    ) -> impl Stream<Item = Result<(Vec<u8>, V)>> + Send + 'static
    where
        K: AsRef<[u8]>,
        V: BorshDeserialize + Send + 'static,
    {
        (**self).nonverifiable_prefix(prefix)
    }
}

pub(crate) trait StateWrite: StateRead + Send + Sync {
    /// Puts the given value into the verifiable key-value store under the given key.
    fn put<K, V>(&mut self, key: K, value: &V) -> Result<()>
    where
        K: Into<String>,
        V: BorshSerialize;

    /// Deletes a key-value from the verifiable key-value store.
    fn delete<K: Into<String>>(&mut self, key: K);

    /// Puts the given value into the non-verifiable key-value store under the given key.
    fn nonverifiable_put<K, V>(&mut self, key: K, value: &V) -> Result<()>
    where
        K: Into<Vec<u8>>,
        V: BorshSerialize;

    /// Deletes a key-value from the non-verifiable key-value store.
    fn nonverifiable_delete<K: Into<Vec<u8>>>(&mut self, key: K);
}

impl<'a, S: StateWrite + Send + Sync> StateWrite for &'a mut S {
    fn put<K, V>(&mut self, key: K, value: &V) -> Result<()>
    where
        K: Into<String>,
        V: BorshSerialize,
    {
        (**self).put(key, value)
    }

    fn delete<K: Into<String>>(&mut self, key: K) {
        (**self).delete(key)
    }

    fn nonverifiable_put<K, V>(&mut self, key: K, value: &V) -> Result<()>
    where
        K: Into<Vec<u8>>,
        V: BorshSerialize,
    {
        (**self).nonverifiable_put(key, value)
    }

    fn nonverifiable_delete<K: Into<Vec<u8>>>(&mut self, key: K) {
        (**self).nonverifiable_delete(key)
    }
}
