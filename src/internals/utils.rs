#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CacheState<T, DATA> {
    Ready(T),
    NotReady(DATA),
}
