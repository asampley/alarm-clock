#[cfg(target_arch = "xtensa")]
pub use xtensa::*;

#[cfg(target_arch = "xtensa")]
mod xtensa {
	use embassy_sync::lazy_lock::LazyLock;
	use esp_hal::peripherals::FLASH;

	use crate::Mutex;

	pub type Storage = esp_storage::FlashStorage<'static>;
	pub type StorageError = esp_storage::FlashStorageError;

	pub static STORAGE: LazyLock<Mutex<Storage>> = LazyLock::new(||
		Mutex::new({
			#[allow(unused_mut)]
			let mut storage = Storage::new(unsafe { FLASH::steal() });
			
			#[cfg(feature = "esp32s3")] {
				storage = storage.multicore_auto_park();
			}

			storage
		})
	);
}
