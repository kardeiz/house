use crate::{err, utils, Store, Object};

use std::collections::HashSet;

pub(crate) const TERMS_PREFIX: &'static [u8] = b"__house__/terms/";

pub struct Term<'a> {
    pub field: &'a str,
    pub value: &'a [u8],
}

impl<'a> Term<'a> {
    pub(crate) fn flatten_with_id(self, id: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.field.len() + self.value.len() + 8);
        out.extend(self.field.as_bytes());
        out.extend(self.value);
        out.extend(&utils::u64_to_bytes(id));
        out
    }
}

pub trait Queryable {
    fn query_terms(&self) -> Vec<Term>;
}

pub trait Query {
    fn matching_ids<T>(&self, store: &Store<T>) -> err::Result<HashSet<u64>>;
}

pub struct StrEquals<'a>(pub &'a str, pub &'a str);

impl<'a> Query for StrEquals<'a> {
    fn matching_ids<T>(&self, store: &Store<T>) -> err::Result<HashSet<u64>> {

        let prefix = self.0.as_bytes().into_iter().chain(self.1.as_bytes()).copied().collect::<Vec<_>>();

        let prefix_len = prefix.len();

        let mut out = HashSet::new();

        for key in store.meta.scan_prefix(prefix).keys() {
            let key = key?;
            if let Ok(id) = utils::bytes_to_u64(&key[prefix_len..]) {
                out.insert(id);
            }
        }

        Ok(out)
    }
}

pub struct Results<'a, T> {
    pub(crate) store: &'a Store<T>,
    pub(crate) matching_ids: HashSet<u64>,
}

impl<'a, T: Queryable + serde::Serialize + serde::de::DeserializeOwned> Results<'a, T> {

    pub fn first(self) -> err::Result<Option<Object<T>>> {
        let Self { store, matching_ids } = self;
        Ok(matching_ids.into_iter().next()
            .map(|id| store.find(id) )
            .transpose()?
            .and_then(|x| x))
    }

    pub fn all(self) -> err::Result<Vec<Object<T>>> {
        let Self { store, matching_ids } = self;
        let mut out = Vec::with_capacity(matching_ids.len());

        for id in matching_ids.into_iter() {
            if let Some(obj) = store.find(id)? {
                out.push(obj);
            }
        }

        Ok(out)
    }

}