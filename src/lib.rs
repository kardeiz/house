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
}

use std::marker::PhantomData;
use sled::transaction::Transactional;

pub struct Store<T> {
    db: sled::Db,
    tree: sled::Tree,
    meta: sled::Tree,
    marker: PhantomData<fn(T)>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Object<T> {
    pub id: u64,
    pub inner: T,
}

// impl<T: Storable> Object<T> {
//     fn query_terms(&self) -> Vec<Vec<u8>> {
//         self.inner.query_terms()
//             .into_iter()
//             .map(|t| t.flatten(self.id) )
//             .collect()
//     }
// }


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

pub struct QueryTerm<'a> {
    key: &'a [u8],
    val: &'a [u8],
}

impl<'a> QueryTerm<'a> {
    pub fn flatten(self, id: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.key.len() + self.val.len() + 8);
        out.extend(self.key);
        out.extend(self.val);
        out.extend(&utils::u64_to_bytes(id));
        out
    }
}

pub trait Storable: serde::Serialize + serde::de::DeserializeOwned {
    fn query_terms(&self) -> Vec<QueryTerm>;
}

impl<T: Storable> Store<T> {
    pub fn create(&self, inner: &T) -> err::Result<u64> {
        let id = self.db.generate_id()?;

        (&self.tree, &self.meta).transaction(|(tree, meta)| {
            let serialized_inner = utils::serialize(inner)?;

            let query_terms = inner.query_terms()
                .into_iter()
                .map(|t| t.flatten(id) )
                .collect::<Vec<_>>();

            let serialized_query_terms = utils::serialize(&query_terms)?;

            tree.insert(&id.to_le_bytes(), serialized_inner)?;

            // meta.insert(b"")

            Ok(())

        }).map_err(|_| err::custom("Transaction error"))?;

        Ok(id)
    }
}




#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
