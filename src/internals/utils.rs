use std::hash::Hasher;

use rand::{
    RngExt,
    distr::{Distribution, StandardUniform},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CacheState<T, DATA> {
    Ready(T),
    NotReady(DATA),
}

pub fn random_value<T>() -> T
where
    StandardUniform: Distribution<T>,
{
    rand::rng().random()
}
