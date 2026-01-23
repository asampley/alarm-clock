mod hal;

use bincode::{config::{Configuration, Fixint, Limit, LittleEndian}, Decode, Encode};
use defmt::{Debug2Format, Format};
use hal::STORAGE;

use embedded_storage::{ReadStorage, Storage};
use thiserror::Error;

use crate::{error, info, RwLock, RwLockReadGuard};

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

static SETTINGS: RwLock<Settings> = RwLock::new(Settings::new());

pub async fn settings() -> RwLockReadGuard<'static, Settings> {
	SETTINGS.read().await
}

pub async fn modify_settings(f: impl FnOnce(&mut Settings)) {
	f(&mut *SETTINGS.write().await);

	match save_settings().await {
		Ok(()) => (),
		Err(e) => error!("failed to save settings: {:?}", Debug2Format(&e)),
	};
}

#[derive(Debug, Error)]
pub enum SaveError {
	#[error("storage error: {0:?}")]
	Storage(hal::StorageError),
	#[error("encoding error: {0}")]
	Encode(bincode::error::EncodeError),
}

#[derive(Debug, Error)]
pub enum LoadError {
	#[error("storage error: {0:?}")]
	Storage(hal::StorageError),
	#[error("decoding error: {0}")]
	Decode(bincode::error::DecodeError),
}

// position settings at the end of the storage space
fn settings_address(storage: &impl Storage) -> u32 {
	if core::mem::size_of::<u32>() >= core::mem::size_of::<usize>() {
		(storage.capacity() - SETTINGS_MAX_SIZE) as u32
	} else {
		core::cmp::min(u32::MAX as usize - SETTINGS_MAX_SIZE, storage.capacity() - SETTINGS_MAX_SIZE) as u32
	}
}

pub async fn save_settings() -> Result<(), SaveError> {
	let mut buffer = [0; SETTINGS_MAX_SIZE];

	let written = {
		let set = SETTINGS.read().await;

		info!("Saving settings {:?}", *set);

		bincode::encode_into_slice(
			&*set,
			&mut buffer,
			BincodeConfig::default()
		).map_err(SaveError::Encode)
	}?;

	{
		let mut storage = STORAGE.get().lock().await;

		let address = settings_address(&*storage);

		storage.write(address, &buffer[..written]).map_err(SaveError::Storage)?;

		info!("Saved settings to 0x{:x}", address);
	}

	Ok(())
}

pub async fn load_settings() -> Result<(), LoadError> {
	let mut buffer = [0; SETTINGS_MAX_SIZE];

	{
		let mut storage = STORAGE.get().lock().await;

		let address = settings_address(&*storage);

		storage.read(address, &mut buffer).map_err(LoadError::Storage)?;

		info!("Loading settings from 0x{:x}", address);
	}

	let settings = bincode::decode_from_slice(&buffer, BincodeConfig::default())
		.map_err(LoadError::Decode)?
		.0;

	info!("Loading settings: {:?}", settings);

	*SETTINGS.write().await = settings;

	info!("Loaded settings");

	Ok(())
}
