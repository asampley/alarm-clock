mod hal;

use std::io::Read;
use std::path::PathBuf;

use alarm_clock_generic::channel::event::{ButtonDirection, ButtonFunction};
use alarm_clock_generic::channel::{AlphanumReceiver, EventSender, MidiNoteReceiver, SensorSubscriber};
use alarm_clock_generic::circuit::alphanum::Alphanum;
use alarm_clock_generic::circuit::bmp::Bmp180;
use alarm_clock_generic::circuit::button::Button;
use alarm_clock_generic::circuit::buzzer::Buzzer;
use alarm_clock_generic::circuit::dht::Dht11;
pub use alarm_clock_generic::startup;
use alarm_clock_generic::synth::Synth;
use alarm_clock_generic::{Error as HalError, SYNTH_NOTES, StartupConfig};

use clap::Parser;

use log::{error, info};

use embassy_executor::Spawner;
use linux_embedded_hal::gpio_cdev::{self, Chip, LineRequestFlags};
use linux_embedded_hal::i2cdev::linux::LinuxI2CError;
use linux_embedded_hal::{CdevPin, I2cdev};
use serde::Deserialize;
use thiserror::Error;

use self::hal::{I2c, Pin};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
	#[arg(default_value = "/etc/alarm-clock/config.toml")]
	config: PathBuf,
}

#[derive(Debug, Error)]
pub enum Error {
	#[error("io error")]
	Io(#[from] std::io::Error),
	#[error("config error")]
	Toml(#[from] toml::de::Error),
	#[error("cdev error")]
	Cdev(#[from] gpio_cdev::Error),
	#[error("i2c error")]
	I2c(#[from] LinuxI2CError),
	#[error("startup error")]
	Hal(#[from] HalError<<I2c as embedded_hal::i2c::ErrorType>::Error>),
}

#[derive(Deserialize)]
struct Config {
	gpio_chip: PathBuf,
	i2c_device: PathBuf,
	lines: LineConfig,
}

#[derive(Deserialize)]
struct LineConfig {
	buzzer: u32,
	select: u32,
	prev: u32,
	next: u32,
	dht: u32,
}

pub async fn run(spawner: Spawner) -> Result<(), Error> {
	use LineRequestFlags as LRF;

	let args = Args::parse();

	let config: Config = toml::from_str(&std::fs::read_to_string(&args.config)?)?;

	let mut chip = Chip::new(config.gpio_chip)?;

	// create buzzer controller
	let buzzer_pin = CdevPin::new(chip.get_line(config.lines.buzzer)?.request(LRF::OUTPUT, 0, "alarm_clock_buzzer")?)?.into();

	// create button pollers
	let button_pins = [
		(
			ButtonFunction::Select,
			CdevPin::new(chip.get_line(config.lines.select)?.request(LRF::INPUT, 0, "alarm_clock_select")?)?.into(),
		),
		(
			ButtonFunction::Direction(ButtonDirection::Prev),
			CdevPin::new(chip.get_line(config.lines.prev)?.request(LRF::INPUT, 0, "alarm_clock_prev")?)?.into(),
		),
		(
			ButtonFunction::Direction(ButtonDirection::Next),
			CdevPin::new(chip.get_line(config.lines.next)?.request(LRF::INPUT, 0, "alarm_clock_next")?)?.into(),
		),
	];

	let i2c = I2cdev::new(config.i2c_device)?.into();

	let dht_pin = CdevPin::new(chip.get_line(config.lines.dht)?.request(
		LRF::INPUT | LRF::OUTPUT | LRF::OPEN_DRAIN,
		1,
		"alarm_clock_dht",
	)?)?
	.into();

	startup(
		StartupConfig {
			buzzer_pin,
			button_pins,
			dht_pin,
			i2c,
			update_buzzer,
			poll_input,
			dht_task,
			i2c_task,
			save_settings: Some(save_settings),
			load_settings: Some(load_settings),
		},
		spawner,
	)
	.await?;

	Ok(())
}

#[embassy_executor::task]
async fn update_buzzer(
	note_receiver: MidiNoteReceiver,
	buzzer: Buzzer<Pin>,
	synth: Synth<SYNTH_NOTES>,
) {
	alarm_clock_generic::tasks::buzzer::update_buzzer(note_receiver, buzzer, synth).await
}

#[embassy_executor::task(pool_size = 3)]
async fn poll_input(event_sender: EventSender, button: Button<Pin>, function: ButtonFunction) {
	alarm_clock_generic::tasks::input::poll_input(event_sender, button, function).await
}

#[embassy_executor::task]
async fn i2c_task(
	i2c: I2c,
	alphanum: Alphanum,
	bmp: Bmp180,
	event_sender: EventSender,
	alphanum_receiver: AlphanumReceiver,
	sensor_subscriber: SensorSubscriber<'static>,
) {
	alarm_clock_generic::tasks::i2c::i2c_task(
		i2c,
		alphanum,
		bmp,
		event_sender,
		alphanum_receiver,
		sensor_subscriber,
	)
	.await
}

#[embassy_executor::task]
async fn dht_task(
	humid_temp: Dht11<Pin>,
	sensor_subscriber: SensorSubscriber<'static>,
	event_sender: EventSender,
) {
	alarm_clock_generic::tasks::dht::dht_task(humid_temp, sensor_subscriber, event_sender).await
}

const SETTINGS_FILE: &str = "settings";

fn save_settings(bytes: &[u8]) -> Result<(), ()> {
	info!("Saving settings to '{}'", SETTINGS_FILE);

	std::fs::write(SETTINGS_FILE, bytes)
		.inspect_err(|e| error!("Error saving settings: {:?}", e))
		.map_err(|_| ())
}

fn load_settings(bytes: &mut [u8]) -> Result<(), ()> {
	info!("Loading settings from '{}'", SETTINGS_FILE);

	std::fs::File::open(SETTINGS_FILE)
		.inspect_err(|e| error!("Error loading settings: {:?}", e))
		.map_err(|_| ())?
		.read(bytes)
		.inspect_err(|e| error!("Error loading settings: {:?}", e))
		.map_err(|_| ())
		.map(|_| ())
}
