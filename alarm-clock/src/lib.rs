#![no_std]
#![feature(impl_trait_in_assoc_type)]
#![feature(type_alias_impl_trait)]
#![feature(never_type)]

use defmt::Debug2Format;
use defmt_rtt as _;

pub use defmt::{debug, error, info, trace, warn};

use embassy_executor::{SpawnError, Spawner};

use embassy_sync::lazy_lock::LazyLock;

use futures_lite::FutureExt;

mod hal;

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

mod storage;

mod synth;
use synth::Synth;

mod states;
use states::{
	ConcreteState, State, StateAlarm, StateAlarmSongSet, StateAlarmTimeSet, StateClock,
	StateClockSet, StateMainMenu, StatePlay, StateSensors, StateTimer, StateTimerMenu,
	StateTimerRunning, StateTimerSet, StateTransition,
};

mod tasks;
use tasks::alarm::alarm_task;
use tasks::buzzer::update_buzzer;
use tasks::dht::dht_task;
use tasks::i2c::{I2c, i2c_task};
use tasks::input::poll_input;
use tasks::player::midi_player;
use tasks::timer::timer_task;

mod time;
use time::ClockTime;

use crate::storage::load_settings;

mod util;

// an instant that marks midnight
static CONFIG: LazyLock<Config> = LazyLock::new(|| {
	serde_json_core::from_str(include_str!("../tweaks.json"))
		.unwrap()
		.0
});

const MIDI_NOTE_CAPACITY: usize = 64;
const SYNTH_NOTES: usize = 32;

static ALPHANUM_CHANNEL: AlphanumChannel = Channel::new();
static EVENT_CHANNEL: EventChannel = Channel::new();
static MIDI_NOTE_CHANNEL: MidiNoteChannel = Channel::new();
static PLAYER_CHANNEL: PlayerChannel = Channel::new();
static SENSOR_CHANNEL: SensorPubSub = PubSubChannel::new();
static TIMER_CHANNEL: TimerChannel = Channel::new();

#[derive(Debug, Error)]
pub enum Error {
	#[error("failed to spawn task")]
	SpawnError(SpawnError),
	#[error("pin communication failed")]
	I2c(tasks::i2c::Error),
}

impl From<SpawnError> for Error {
	fn from(value: SpawnError) -> Self {
		Self::SpawnError(value)
	}
}

struct Devices {
	buzzer: Buzzer,
	buttons: [(ButtonFunction, Button); 3],
	humid_temp: Dht11,
	i2c: I2c,
}

pub async fn startup(spawner: Spawner) -> Result<(), Error> {
	let config = CONFIG.get();
	let synth = Synth::new(config.synth_config.clone());

	let Devices {
		buzzer,
		buttons,
		humid_temp,
		mut i2c,
	} = hal::setup_hardware(config)?;

	let mut alphanum = Alphanum::new(&mut i2c).map_err(Error::I2c)?;
	alphanum
		.set_brightness(&mut i2c, config.brightness)
		.await
		.map_err(Error::I2c)?;
	alphanum.ascii_uppercase(config.ascii_uppercase);

	let bmp = Bmp180::new(&mut i2c).map_err(Error::I2c)?;

	// load settings
	match load_settings().await {
		Ok(()) => (),
		Err(e) => error!("Failed to load settings: {:?}", Debug2Format(&e)),
	}

	// start task to update buzzer
	spawner.spawn(update_buzzer(MIDI_NOTE_CHANNEL.receiver(), buzzer, synth))?;

	// start task to read poll button
	for (function, button) in buttons {
		spawner.spawn(poll_input(EVENT_CHANNEL.sender(), button, function))?;
	}

	// start playing midi file
	spawner.spawn(midi_player(
		PLAYER_CHANNEL.receiver(),
		MIDI_NOTE_CHANNEL.sender(),
		EVENT_CHANNEL.sender(),
	))?;

	// start i2c task
	spawner.spawn(i2c_task(
		i2c,
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
	spawner.spawn(dht_task(
		humid_temp,
		SENSOR_CHANNEL.subscriber().unwrap(),
		EVENT_CHANNEL.sender(),
	))?;

	let mut state_transition = StateTransition::Clock;

	let event_receiver = EVENT_CHANNEL.receiver();
	let sensor_publisher = SENSOR_CHANNEL.publisher().unwrap();

	loop {
		info!(
			"Entering state {:?}",
			defmt::Debug2Format(&state_transition)
		);

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
