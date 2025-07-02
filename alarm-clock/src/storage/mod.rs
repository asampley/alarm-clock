mod hal;
use bincode::{config::{Configuration, Fixint, Limit, LittleEndian}, Decode, Encode};
use defmt::Format;
use hal::STORAGE;

use embedded_storage::{ReadStorage, Storage};
use thiserror::Error;

use crate::{info, Mutex};

// 256 KiB, if the binary gets bigger than that, panic.
const SETTINGS_ADDRESS: u32 = 0x40_000;
const SETTINGS_MAX_SIZE: usize = 1024;

type BincodeConfig = Configuration<LittleEndian, Fixint, Limit<SETTINGS_MAX_SIZE>>;

#[derive(Decode, Encode, Format)]
pub struct Settings {
	pub alarm_song_index: usize,
}

impl Settings {
	const fn new() -> Self {
		Self {
			alarm_song_index: 0,
		}
	}
}

pub static SETTINGS: Mutex<Settings> = Mutex::new(Settings::new());

#[derive(Debug, Error)]
pub enum SaveError {
	#[error("flash storage error: {0:?}")]
	Storage(esp_storage::FlashStorageError),
	#[error("encoding error: {0}")]
	Encode(bincode::error::EncodeError),
}

#[derive(Debug, Error)]
pub enum LoadError {
	#[error("flash storage error: {0:?}")]
	Storage(esp_storage::FlashStorageError),
	#[error("decoding error: {0}")]
	Decode(bincode::error::DecodeError),
}

pub async fn save_settings() -> Result<(), SaveError> {
	let mut buffer = [0; SETTINGS_MAX_SIZE];

	let written = {
		let set = SETTINGS.lock().await;

		info!("Writing settings {:?}", *set);

		bincode::encode_into_slice(
			&*set,
			&mut buffer,
			BincodeConfig::default()
		).map_err(SaveError::Encode)
	}?;

	STORAGE
		.get()
		.lock()
		.await
		.write(SETTINGS_ADDRESS, &buffer[..written])
		.map_err(SaveError::Storage)?;

	info!("Wrote settings");

	Ok(())
}

pub async fn load_settings() -> Result<(), LoadError> {
	let mut buffer = [0; SETTINGS_MAX_SIZE];

	STORAGE
		.get()
		.lock()
		.await
		.read(SETTINGS_ADDRESS, &mut buffer)
		.map_err(LoadError::Storage)?;

	let settings = bincode::decode_from_slice(&buffer, BincodeConfig::default())
		.map_err(LoadError::Decode)?
		.0;

	info!("Loading settings: {:?}", settings);

	*SETTINGS.lock().await = settings;

	info!("Loaded settings");

	Ok(())
}
