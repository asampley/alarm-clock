#[cfg(target_arch = "xtensa")]
pub use xtensa::*;

#[cfg(target_arch = "xtensa")]
mod xtensa {
    use embassy_sync::lazy_lock::LazyLock;

    use crate::Mutex;

	pub type Storage = esp_storage::FlashStorage;

	pub static STORAGE: LazyLock<Mutex<Storage>> = LazyLock::new(Default::default);
}
