/// Port for size caching (persist url→size mapping).
pub trait CachePort: Send + Sync {
    fn get_size(&self, url: &str) -> Option<u64>;
    fn set_size(&self, url: &str, size: u64);
    fn save(&self);
}
