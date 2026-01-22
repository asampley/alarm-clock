#[cfg(target_arch = "xtensa")]
pub use xtensa::*;

#[cfg(target_arch = "xtensa")]
mod xtensa {
	use embassy_sync::lazy_lock::LazyLock;
	use esp_hal::peripherals::FLASH;

	use crate::Mutex;

	pub type Storage = esp_storage::FlashStorage<'static>;

	pub static STORAGE: LazyLock<Mutex<Storage>> = LazyLock::new(||
		Mutex::new(Storage::new(unsafe { FLASH::steal() }).multicore_auto_park())
	);
}
