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

pub fn find_value_inbetween<T: PartialOrd>(
    mut values: impl ExactSizeIterator<Item = T>,
    value: T,
) -> Option<(T, usize)> {
    let mut idx = 0;
    let mut lhs = values.next()?;
    for rhs in values {
        idx += 1;
        if value > lhs && value <= rhs {
            return Some((lhs, idx));
        }
        lhs = rhs;
    }
    None
}
