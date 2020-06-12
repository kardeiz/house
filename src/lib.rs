/// Error management
pub mod err {

    /// Error container
    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("Database error: `{0}`")]
        Sled(#[from] sled::Error),
        #[cfg(feature = "bincode")]
        #[error("De/serialization error: `{0}`")]
        Bincode(#[from] bincode::Error),
        #[cfg(feature = "serde_cbor")]
        #[error("De/serialization error: `{0}`")]
        CBOR(#[from] serde_cbor::Error),
        #[error("Error: `{0}`")]
        Custom(Box<str>),
    }

    impl From<sled::transaction::TransactionError<Error>> for Error {
        fn from(t: sled::transaction::TransactionError<Error>) -> Self {
            match t {
                sled::transaction::TransactionError::Abort(t) => t,
                sled::transaction::TransactionError::Storage(t) => Error::Sled(t),
            }
        }
    }

    impl From<Error> for sled::transaction::ConflictableTransactionError<Error> {
        fn from(t: Error) -> Self {
            sled::transaction::ConflictableTransactionError::Abort(t)
        }
    }

    /// Create a custom error.
    pub fn custom<T: std::fmt::Display>(t: T) -> Error {
        Error::Custom(t.to_string().into_boxed_str())
    }

    pub type Result<T> = std::result::Result<T, Error>;
}

mod utils {

    use std::convert::TryInto;

    #[cfg(feature = "bincode")]
    pub fn serialize<T: ?Sized + serde::Serialize>(value: &T) -> crate::err::Result<Vec<u8>> {
        Ok(bincode::serialize(value)?)
    }

    #[cfg(feature = "bincode")]
    pub fn deserialize<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> crate::err::Result<T> {
        Ok(bincode::deserialize(bytes)?)
    }

    pub fn u64_to_bytes(value: u64) -> [u8; 8] {
        u64::to_be_bytes(value)
    }

    pub fn bytes_to_u64(value: &[u8]) -> crate::err::Result<u64> {
        Ok(u64::from_be_bytes(value.try_into().map_err(crate::err::custom)?))
    }
}

pub mod query;

use sled::transaction::Transactional;
use std::marker::PhantomData;

pub struct Store<T> {
    pub db: sled::Db,
    pub tree: sled::Tree,
    pub meta: sled::Tree,
    pub marker: PhantomData<fn(T)>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Object<T> {
    pub id: u64,
    pub inner: T,
}

impl<T> std::ops::Deref for Object<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> std::ops::DerefMut for Object<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: query::Queryable + serde::Serialize + serde::de::DeserializeOwned> Store<T> {
    pub fn create(&self, inner: &T) -> err::Result<u64> {
        let id = self.db.generate_id()?;

        &[&self.tree, &self.meta].transaction(|trees| {
            let id_bytes = utils::u64_to_bytes(id);

            let tree = &trees[0];
            let meta = &trees[1];

            let serialized_inner = utils::serialize(inner)?;

            let new_terms =
                inner.query_terms().into_iter().map(|t| t.flatten_with_id(id)).collect::<Vec<_>>();

            let serialized_new_terms = utils::serialize(&new_terms)?;

            tree.insert(&id_bytes, serialized_inner)?;

            meta.insert(
                query::TERMS_PREFIX.into_iter().chain(&id_bytes).copied().collect::<Vec<_>>(),
                serialized_new_terms,
            )?;

            for term in new_terms {
                meta.insert(term, sled::IVec::default())?;
            }

            Ok(())
        })?;

        Ok(id)
    }

    pub fn update(&self, object: &Object<T>) -> err::Result<()> {
        self.update_multi(std::slice::from_ref(object))
    }

    pub fn update_multi(&self, objects: &[Object<T>]) -> err::Result<()> {
        &[&self.tree, &self.meta].transaction(|trees| {
            let tree = &trees[0];
            let meta = &trees[1];

            for Object { id, inner } in objects {

                let id_bytes = utils::u64_to_bytes(*id);

                let serialized_inner = utils::serialize(inner)?;

                let new_terms =
                    inner.query_terms().into_iter().map(|t| t.flatten_with_id(*id)).collect::<Vec<_>>();

                let serialized_new_terms = utils::serialize(&new_terms)?;

                let mut batch = sled::Batch::default();

                if let Some(serialized_prev_terms) = meta.insert(
                    query::TERMS_PREFIX.into_iter().chain(&id_bytes).copied().collect::<Vec<_>>(),
                    serialized_new_terms,
                )? {
                    let prev_terms: Vec<Vec<u8>> = utils::deserialize(&serialized_prev_terms)?;
                    for term in prev_terms {
                        batch.remove(term);
                    }
                }

                for term in new_terms {
                    batch.insert(term, sled::IVec::default());
                }

                meta.apply_batch(&batch)?;
            }

            Ok(())
        })?;
        Ok(())
    }

    pub fn all(&self) -> err::Result<Vec<Object<T>>> {
        
        Ok(self
            .tree
            .iter()
            .flatten()
            .map(|(k, v)| {
                Ok(Object {
                    id: utils::bytes_to_u64(k.as_ref())?,
                    inner: utils::deserialize(&v)?,
                })
            })
            .collect::<err::Result<Vec<_>>>()?)
    }

    pub fn find(&self, id: u64) -> err::Result<Option<Object<T>>> {
        Ok(self
            .tree
            .get(utils::u64_to_bytes(id))?
            .map(|bytes| utils::deserialize(&bytes))
            .transpose()?
            .map(|inner| Object { id, inner }))
    }

    pub fn filter<Q: query::Query>(&self, query: Q) -> err::Result<query::Results<T>> {
        let matching_ids = query.matching_ids(self)?;
        Ok(query::Results { matching_ids, store: self })
    }

}
