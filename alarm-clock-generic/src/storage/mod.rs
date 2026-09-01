use bincode::{
	Decode, Encode,
	config::{Configuration, Fixint, Limit, LittleEndian},
};
use defmt_or_log::Debug2Format;
use embassy_sync::once_lock::OnceLock;

use thiserror::Error;

use crate::{
	LoadSettings, SaveSettings,
	channel::{RwLock, RwLockReadGuard},
};
use crate::{error, info, warn};

pub const SETTINGS_MAX_SIZE: usize = 1024;

pub static SAVE_HAL: OnceLock<SaveSettings> = OnceLock::new();
pub static LOAD_HAL: OnceLock<LoadSettings> = OnceLock::new();

type BincodeConfig = Configuration<LittleEndian, Fixint, Limit<SETTINGS_MAX_SIZE>>;

#[derive(Debug, Decode, Encode)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
	#[error("storage error")]
	Storage(),
	#[error("encoding error: {0}")]
	Encode(bincode::error::EncodeError),
}

#[derive(Debug, Error)]
pub enum LoadError {
	#[error("storage error")]
	Storage(),
	#[error("decoding error: {0}")]
	Decode(bincode::error::DecodeError),
}

pub async fn save_settings() -> Result<(), SaveError> {
	let Some(save) = SAVE_HAL.try_get() else {
		warn!("No hal save implementation set");
		return Ok(());
	};

	let mut buffer = [0; SETTINGS_MAX_SIZE];

	let written = {
		let settings = SETTINGS.read().await;

		info!("Saving settings {:?}", *settings);

		bincode::encode_into_slice(&*settings, &mut buffer, BincodeConfig::default())
			.map_err(SaveError::Encode)
	}?;

	save(&buffer[..written]).or(Err(SaveError::Storage()))?;

	Ok(())
}

pub async fn load_settings() -> Result<(), LoadError> {
	let Some(load) = LOAD_HAL.try_get() else {
		warn!("No hal save implementation set");
		return Ok(());
	};

	let mut buffer = [0; SETTINGS_MAX_SIZE];

	load(&mut buffer).or(Err(LoadError::Storage()))?;

	{
		let mut settings = SETTINGS.write().await;

		*settings = bincode::decode_from_slice(&buffer, BincodeConfig::default())
			.map_err(LoadError::Decode)?
			.0;

		info!("Loaded settings: {:?}", &*settings);
	}

	Ok(())
}
