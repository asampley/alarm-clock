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
use gpiocdev::{ line::Bias, line::Drive, line::Value };
use linux_embedded_hal::i2cdev::linux::LinuxI2CError;
use linux_embedded_hal::I2cdev;
use gpiocdev_embedded_hal::{ OutputPin, InputPin };
use serde::Deserialize;
use thiserror::Error;

use crate::linux::hal::{FlexPin, I2c};

type AsyncInputPin = gpiocdev_embedded_hal::async_io::InputPin;

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
	#[error("gpio error")]
	Gpio(#[from] gpiocdev::Error),
	#[error("gpio hal error")]
	GpioHal(#[from] gpiocdev_embedded_hal::Error),
	#[error("i2c error")]
	I2c(#[from] LinuxI2CError),
	#[error("startup error")]
	Hal(#[from] HalError<<I2c as embedded_hal::i2c::ErrorType>::Error>),
}

#[derive(Deserialize)]
struct Config {
	i2c_device: PathBuf,
	device_config: DeviceConfig,
}

#[derive(Deserialize)]
struct GpioConfig {
	gpio_chip: PathBuf,
	line: u32,
	internal_pull_resistor: bool,
}

#[derive(Deserialize)]
struct DeviceConfig {
	buzzer: GpioConfig,
	select: GpioConfig,
	prev: GpioConfig,
	next: GpioConfig,
	dht: GpioConfig,
}

pub async fn run(spawner: Spawner) -> Result<(), Error> {
	let args = Args::parse();

	let config: Config = toml::from_str(&std::fs::read_to_string(&args.config)?)?;

	// create buzzer controller
	let buzzer_pin = OutputPin::try_from({
		let gpio_config = config.device_config.buzzer;

		gpiocdev::Request::builder()
			.on_chip(gpio_config.gpio_chip)
			.with_line(gpio_config.line)
			.as_output(Value::Inactive)
			.request()?
	})?;

	let button_pin_req = |c: &GpioConfig| {
		let mut builder = gpiocdev::Request::builder();

		builder
			.on_chip(&c.gpio_chip)
			.with_line(c.line);

		if c.internal_pull_resistor {
			builder.with_bias(Bias::PullUp);
		}

		builder.as_input().request()
	};

	// create button pollers
	let button_pins = [
		(
			ButtonFunction::Select,
			AsyncInputPin::from(InputPin::try_from(button_pin_req(&config.device_config.select)?)?),
		),
		(
			ButtonFunction::Direction(ButtonDirection::Prev),
			AsyncInputPin::from(InputPin::try_from(button_pin_req(&config.device_config.prev)?)?),
		),
		(
			ButtonFunction::Direction(ButtonDirection::Next),
			AsyncInputPin::from(InputPin::try_from(button_pin_req(&config.device_config.next)?)?),
		),
	];

	let i2c = I2cdev::new(config.i2c_device)?.into();

	let dht_pin = FlexPin::from(OutputPin::try_from({
		let gpio_config = config.device_config.dht;
		let mut builder = gpiocdev::Request::builder();

		builder
			.on_chip(gpio_config.gpio_chip)
			.with_line(gpio_config.line);

		if gpio_config.internal_pull_resistor {
			builder.with_bias(Bias::PullUp);
		}

		builder.with_drive(Drive::OpenDrain).request()?
	})?);

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
	buzzer: Buzzer<OutputPin>,
	synth: Synth<SYNTH_NOTES>,
) {
	alarm_clock_generic::tasks::buzzer::update_buzzer(note_receiver, buzzer, synth).await
}

#[embassy_executor::task(pool_size = 3)]
async fn poll_input(event_sender: EventSender, button: Button<AsyncInputPin>, function: ButtonFunction) {
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
	humid_temp: Dht11<FlexPin>,
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
