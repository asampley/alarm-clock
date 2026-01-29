#![no_std]
#![feature(impl_trait_in_assoc_type)]
#![feature(never_type)]

use defmt::Debug2Format;
use defmt_rtt as _;

pub use defmt::{debug, error, info, trace, warn};

use embassy_executor::{SpawnError, Spawner};

use embassy_sync::lazy_lock::LazyLock;

use embassy_time::Duration;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::i2c::{self, I2c as SyncI2c};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::i2c::I2c as AsyncI2c;
use futures_lite::FutureExt;

use thiserror::Error;

pub mod circuit;
use circuit::{alphanum::Alphanum, bmp::Bmp180, button::Button, buzzer::Buzzer, dht::Dht11};

pub mod tweaks;
use tweaks::Config;

pub mod channel;
use channel::event::ButtonFunction;
use channel::{
	AlphanumChannel, Channel, EventChannel, MidiNoteChannel, PlayerChannel, PubSubChannel,
	SensorPubSub, TimerChannel,
};

mod midi_dir;
use midi_dir::MIDI_DIR;

mod selector;

pub mod storage;

pub mod synth;
use synth::Synth;

mod states;
use states::{
	ConcreteState, State, StateAlarm, StateAlarmSongSet, StateAlarmTimeSet, StateClock,
	StateClockSet, StateMainMenu, StatePlay, StateSensors, StateTimer, StateTimerMenu,
	StateTimerRunning, StateTimerSet, StateTransition,
};

pub mod tasks;
use tasks::alarm::alarm_task;
use tasks::player::midi_player;
use tasks::timer::timer_task;

mod time;
use time::ClockTime;

use crate::storage::{LOAD_HAL, SAVE_HAL, load_settings}; 
use crate::tasks::buzzer::UpdateBuzzerTask;
use crate::tasks::dht::DhtTask;
use crate::tasks::i2c::I2cTask;
use crate::tasks::input::PollInputTask;

mod util;

// an instant that marks midnight
static CONFIG: LazyLock<Config> = LazyLock::new(|| {
	serde_json_core::from_str(include_str!("../tweaks.json"))
		.unwrap()
		.0
});

const MIDI_NOTE_CAPACITY: usize = 64;
pub const SYNTH_NOTES: usize = 32;

static ALPHANUM_CHANNEL: AlphanumChannel = Channel::new();
static EVENT_CHANNEL: EventChannel = Channel::new();
static MIDI_NOTE_CHANNEL: MidiNoteChannel = Channel::new();
static PLAYER_CHANNEL: PlayerChannel = Channel::new();
static SENSOR_CHANNEL: SensorPubSub = PubSubChannel::new();
static TIMER_CHANNEL: TimerChannel = Channel::new();

#[derive(Debug, Error)]
pub enum Error<I2c> {
	#[error("failed to spawn task")]
	SpawnError(SpawnError),
	#[error("pin communication failed")]
	I2c(I2c),
}

impl<I2c> From<SpawnError> for Error<I2c> {
	fn from(value: SpawnError) -> Self {
		Self::SpawnError(value)
	}
}

pub struct StartupConfig<P1, P2, P3, P4, T1, T2, T3, T4> where
	P1: OutputPin,
	P2: InputPin + Wait,
	P3: InputPin + OutputPin,
	P4: AsyncI2c + SyncI2c,
{
	pub buzzer_pin: P1,
	pub button_pins: [(ButtonFunction, P2); 3],
	pub dht_pin: P3,
	pub i2c: P4,
	pub update_buzzer: UpdateBuzzerTask<T1, P1>,
	pub poll_input: PollInputTask<T2, P2>,
	pub dht_task: DhtTask<T3, P3>,
	pub i2c_task: I2cTask<T4, P4>,
	pub save_settings: Option<fn(bytes: &[u8]) -> Result<(), ()>>,
	pub load_settings: Option<fn(bytes: &mut [u8]) -> Result<(), ()>>,
}

pub async fn startup<P1, P2, P3, P4, T1, T2, T3, T4>(
	mut startup_config: StartupConfig<P1, P2, P3, P4, T1, T2, T3, T4>,
	spawner: Spawner
) -> Result<(), Error<P4::Error>> where 
	P1: OutputPin,
	P2: InputPin + Wait,
	P3: InputPin + OutputPin,
	P4: AsyncI2c + SyncI2c + i2c::ErrorType,
{
	let config = CONFIG.get();

	// set save and load operations if set
	startup_config.save_settings.map(|v| SAVE_HAL.get_or_init(|| v));
	startup_config.load_settings.map(|v| LOAD_HAL.get_or_init(|| v));

	let synth = Synth::new(config.synth_config.clone());

	let mut alphanum = Alphanum::new(&mut startup_config.i2c).map_err(Error::I2c)?;
	alphanum
		.set_brightness(&mut startup_config.i2c, config.brightness)
		.await
		.map_err(Error::I2c)?;
	alphanum.ascii_uppercase(config.ascii_uppercase);

	let bmp = Bmp180::new(&mut startup_config.i2c).map_err(Error::I2c)?;

	let button_bounce_time = Duration::from_millis(config.button_bounce_ms);
	let buttons = startup_config.button_pins
		.map(|(f, p)| (f, Button::new(p, button_bounce_time)));

	let buzzer = Buzzer::new(startup_config.buzzer_pin);

	let dht = Dht11::new(startup_config.dht_pin);

	// load settings
	match load_settings().await {
		Ok(()) => (),
		Err(e) => error!("Failed to load settings: {:?}", Debug2Format(&e)),
	};

	// start task to update buzzer
	spawner.spawn((startup_config.update_buzzer)(MIDI_NOTE_CHANNEL.receiver(), buzzer, synth))?;

	// start task to read poll button
	for (function, button) in buttons {
		spawner.spawn((startup_config.poll_input)(EVENT_CHANNEL.sender(), button, function))?;
	}

	// start playing midi file
	spawner.spawn(midi_player(
		PLAYER_CHANNEL.receiver(),
		MIDI_NOTE_CHANNEL.sender(),
		EVENT_CHANNEL.sender(),
	))?;

	// start i2c task
	spawner.spawn((startup_config.i2c_task)(
		startup_config.i2c,
		alphanum,
		bmp,
		EVENT_CHANNEL.sender(),
		ALPHANUM_CHANNEL.receiver(),
		SENSOR_CHANNEL.subscriber().unwrap(),
	))?;

	// start alarm task
	spawner.spawn(alarm_task(
		EVENT_CHANNEL.sender(),
		SENSOR_CHANNEL.publisher().unwrap(),
	))?;

	// start timer task
	spawner.spawn(timer_task(TIMER_CHANNEL.receiver(), EVENT_CHANNEL.sender()))?;

	// start dht task
	spawner.spawn((startup_config.dht_task)(
		dht,
		SENSOR_CHANNEL.subscriber().unwrap(),
		EVENT_CHANNEL.sender(),
	))?;

	let mut state_transition = StateTransition::Clock;

	let event_receiver = EVENT_CHANNEL.receiver();
	let sensor_publisher = SENSOR_CHANNEL.publisher().unwrap();

	loop {
		info!("Entering state {:?}", &state_transition);

		let state: ConcreteState = match state_transition {
			StateTransition::Clock => {
				StateClock::new(ALPHANUM_CHANNEL.sender(), PLAYER_CHANNEL.sender()).into()
			}
			StateTransition::MainMenu => StateMainMenu::new(ALPHANUM_CHANNEL.sender()).into(),
			StateTransition::ClockSet => StateClockSet::new(ALPHANUM_CHANNEL.sender()).into(),
			StateTransition::Alarm => StateAlarm::new(
				ALPHANUM_CHANNEL.sender(),
				PLAYER_CHANNEL.sender(),
				&sensor_publisher,
			)
			.into(),
			StateTransition::AlarmTime => StateAlarmTimeSet::new(ALPHANUM_CHANNEL.sender()).into(),
			StateTransition::AlarmSong => {
				StateAlarmSongSet::new(ALPHANUM_CHANNEL.sender(), PLAYER_CHANNEL.sender()).into()
			}
			StateTransition::Play => {
				StatePlay::new(ALPHANUM_CHANNEL.sender(), PLAYER_CHANNEL.sender()).into()
			}
			StateTransition::Timer => StateTimer::new(
				ALPHANUM_CHANNEL.sender(),
				PLAYER_CHANNEL.sender(),
				TIMER_CHANNEL.sender(),
			)
			.into(),
			StateTransition::TimerSet => {
				StateTimerSet::new(ALPHANUM_CHANNEL.sender(), TIMER_CHANNEL.sender()).into()
			}
			StateTransition::TimerRunning(time) => {
				StateTimerRunning::new(time, ALPHANUM_CHANNEL.sender()).into()
			}
			StateTransition::TimerMenu(x) => {
				StateTimerMenu::new(x, ALPHANUM_CHANNEL.sender(), TIMER_CHANNEL.sender()).into()
			}
			StateTransition::Sensors => {
				StateSensors::new(ALPHANUM_CHANNEL.sender(), &sensor_publisher).into()
			}
		};

		state_transition = process_state(state, &event_receiver).await;
	}
}

async fn process_state(
	mut state: impl State,
	event_receiver: &channel::Receiver<channel::message::EventMessage, 16>,
) -> StateTransition {
	info!("Intializing state");
	state.init().await;

	info!("Entering state event loop");
	let state_transition = loop {
		let msg = event_receiver
			.receive()
			.or(async { state.between_events().await })
			.await;

		info!("Event {:?}", msg);

		if let Some(next_state) = state.event(msg).await {
			break next_state;
		}
	};

	info!("Finishing state");
	state.finish().await;

	state_transition
}
