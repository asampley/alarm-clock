#![cfg(target_arch = "xtensa")]

use alarm_clock_hal::channel::event::{ButtonDirection, ButtonFunction};
use alarm_clock_hal::channel::{AlphanumReceiver, EventSender, MidiNoteReceiver, SensorSubscriber};
use alarm_clock_hal::circuit::alphanum::Alphanum;
use alarm_clock_hal::circuit::bmp::Bmp180;
use alarm_clock_hal::circuit::button::Button;
use alarm_clock_hal::circuit::buzzer::Buzzer;
use alarm_clock_hal::circuit::dht::Dht11;
pub use alarm_clock_hal::startup;
use alarm_clock_hal::storage::SETTINGS_MAX_SIZE;
use alarm_clock_hal::synth::Synth;
use alarm_clock_hal::{Error, SYNTH_NOTES, StartupConfig};
use alarm_clock_hal::{error, info};

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::{CriticalSectionMutex, Mutex};
use embassy_sync::lazy_lock::LazyLock;
use embedded_hal::i2c;
use embedded_storage::{ReadStorage, Storage};
use esp_hal::Async;
use esp_hal::gpio::{DriveMode, Flex, Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::i2c::master::I2c;
use esp_hal::peripherals::FLASH;

// import alone enables backtrace
use esp_backtrace as _;
use esp_storage::FlashStorage;

esp_bootloader_esp_idf::esp_app_desc!();

pub static STORAGE: LazyLock<CriticalSectionMutex<FlashStorage>> = LazyLock::new(|| {
	Mutex::new({
		#[allow(unused_mut)]
		let mut storage = FlashStorage::new(unsafe { FLASH::steal() });

		#[cfg(any(xtensa = "esp32s3", xtensa = "esp32"))]
		{
			storage = storage.multicore_auto_park();
		}

		storage
	})
});

type ButtonPin = Input<'static>;
type BuzzerPin = Output<'static>;
type DhtPin = Flex<'static>;
type I2cPin = I2c<'static, Async>;
type I2cPinError = <I2cPin as i2c::ErrorType>::Error;

pub async fn run(spawner: Spawner) -> Result<(), Error<I2cPinError>> {
	use esp_hal::gpio::Pin;

	let p =
		esp_hal::init(esp_hal::Config::default().with_cpu_clock(esp_hal::clock::CpuClock::max()));

	esp_rtos::start(esp_hal::timer::timg::TimerGroup::new(p.TIMG0).timer0);

	// create buzzer controller
	let buzzer_pin = Output::new(p.GPIO14, Level::Low, OutputConfig::default());

	// create button pollers
	let button_pins = [
		(ButtonFunction::Select, p.GPIO1.degrade()),
		(
			ButtonFunction::Direction(ButtonDirection::Prev),
			p.GPIO3.degrade(),
		),
		(
			ButtonFunction::Direction(ButtonDirection::Next),
			p.GPIO2.degrade(),
		),
	]
	.map(|(f, p)| (f, Input::new(p, InputConfig::default().with_pull(Pull::Up))));

	let i2c = esp_hal::i2c::master::I2c::new(p.I2C0, Default::default())
		.unwrap()
		.with_sda(p.GPIO4)
		.with_scl(p.GPIO12)
		.into_async();

	let dht_pin = Output::new(
		p.GPIO13,
		Level::High,
		OutputConfig::default()
			.with_drive_mode(DriveMode::OpenDrain)
			.with_pull(Pull::Up),
	)
	.into_flex();

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
	.await
}

#[embassy_executor::task]
async fn update_buzzer(
	note_receiver: MidiNoteReceiver,
	buzzer: Buzzer<BuzzerPin>,
	synth: Synth<SYNTH_NOTES>,
) {
	alarm_clock_hal::tasks::buzzer::update_buzzer(note_receiver, buzzer, synth).await
}

#[embassy_executor::task(pool_size = 3)]
async fn poll_input(
	event_sender: EventSender,
	button: Button<ButtonPin>,
	function: ButtonFunction,
) {
	alarm_clock_hal::tasks::input::poll_input(event_sender, button, function).await
}

#[embassy_executor::task]
async fn i2c_task(
	i2c: I2cPin,
	alphanum: Alphanum,
	bmp: Bmp180,
	event_sender: EventSender,
	alphanum_receiver: AlphanumReceiver,
	sensor_subscriber: SensorSubscriber<'static>,
) {
	alarm_clock_hal::tasks::i2c::i2c_task(
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
	humid_temp: Dht11<DhtPin>,
	sensor_subscriber: SensorSubscriber<'static>,
	event_sender: EventSender,
) {
	alarm_clock_hal::tasks::dht::dht_task(humid_temp, sensor_subscriber, event_sender).await
}

// position settings at the end of the storage space
fn settings_address(storage: &FlashStorage) -> u32 {
	if core::mem::size_of::<u32>() >= core::mem::size_of::<usize>() {
		(storage.capacity() - SETTINGS_MAX_SIZE) as u32
	} else {
		core::cmp::min(
			u32::MAX as usize - SETTINGS_MAX_SIZE,
			storage.capacity() - SETTINGS_MAX_SIZE,
		) as u32
	}
}

fn save_settings(bytes: &[u8]) -> Result<(), ()> {
	// SAFETY: not used reentrantly
	unsafe {
		STORAGE.get().lock_mut(|storage| {
			let address = settings_address(storage);

			info!("Saving settings to 0x{:x}", address);

			storage
				.write(address, bytes)
				.inspect_err(|e| error!("Error saving settings: {:?}", e))
				.map_err(|_| ())
		})
	}
}

fn load_settings(bytes: &mut [u8]) -> Result<(), ()> {
	// SAFETY: not used reentrantly
	unsafe {
		STORAGE.get().lock_mut(|storage| {
			let address = settings_address(storage);

			info!("Loading settings from 0x{:x}", address);

			storage
				.read(address, bytes)
				.inspect_err(|e| error!("Error loading settings: {:?}", e))
				.map_err(|_| ())
		})
	}
}
