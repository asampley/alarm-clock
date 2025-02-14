#![no_std]
#![feature(impl_trait_in_assoc_type)]
#![feature(never_type)]
#![feature(precise_capturing_in_traits)]

use defmt_rtt as _;
use esp_backtrace as _;

pub use defmt::{debug, error, info, trace, warn};

use embassy_executor::{SpawnError, Spawner};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::lazy_lock::LazyLock;

use embassy_time::Duration;

use esp_hal::gpio::Pin;

use futures_lite::FutureExt;

use tasks::sensors::sensor_task;
use thiserror::Error;

pub mod circuit;
use circuit::hal::{Alphanum, Button, Buzzer, Dht11};

pub mod tweaks;
use tweaks::Config;

pub mod message;
use message::TimerMessage;
use message::{AlphanumMessage, EventMessage, PlayerMessage, SensorMessage, SynthMessage};
use message::{ButtonDirection, ButtonFunction};

mod midi_dir;
use midi_dir::Midi;
use midi_dir::MIDI_DIR;

mod selector;

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
use tasks::alphanum::alphanum_task;
use tasks::buzzer::update_buzzer;
use tasks::input::poll_input;
use tasks::player::midi_player;
use tasks::timer::timer_task;

mod time;
use time::ClockTime;

mod util;

type Channel<T, const CAP: usize> = embassy_sync::channel::Channel<CriticalSectionRawMutex, T, CAP>;
type Sender<T, const CAP: usize> =
	embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, T, CAP>;
type Receiver<T, const CAP: usize> =
	embassy_sync::channel::Receiver<'static, CriticalSectionRawMutex, T, CAP>;
type Mutex<T> = embassy_sync::mutex::Mutex<CriticalSectionRawMutex, T>;
type Watch<T, const CAP: usize> = embassy_sync::watch::Watch<CriticalSectionRawMutex, T, CAP>;

// an instant that marks midnight
static ALARM_SONG: Mutex<Midi> = Mutex::new(MIDI_DIR[0]);
static CONFIG: LazyLock<Config> = LazyLock::new(|| {
	serde_json_core::from_str(include_str!("../tweaks.json"))
		.unwrap()
		.0
});

const MIDI_NOTE_CAPACITY: usize = 64;
const SYNTH_NOTES: usize = 32;

static ALPHANUM_CHANNEL: Channel<AlphanumMessage, 1> = Channel::new();
static EVENT_CHANNEL: Channel<EventMessage, 1> = Channel::new();
static MIDI_NOTE_CHANNEL: Channel<SynthMessage, MIDI_NOTE_CAPACITY> = Channel::new();
static PLAYER_CHANNEL: Channel<PlayerMessage, 1> = Channel::new();
static SENSOR_CHANNEL: Channel<SensorMessage, 1> = Channel::new();
static TIMER_CHANNEL: Channel<TimerMessage, 1> = Channel::new();

#[derive(Debug, Error)]
pub enum Error {
	#[error("failed to spawn task")]
	SpawnError(SpawnError),
	#[error("pin communication failed")]
	EspHalI2c(esp_hal::i2c::master::Error),
}

impl From<SpawnError> for Error {
	fn from(value: SpawnError) -> Self {
		Self::SpawnError(value)
	}
}

impl From<esp_hal::i2c::master::Error> for Error {
	fn from(value: esp_hal::i2c::master::Error) -> Self {
		Self::EspHalI2c(value)
	}
}

pub async fn startup(spawner: Spawner) -> Result<(), Error> {
	let config = CONFIG.get();

	let p = esp_hal::init({
		let mut config = esp_hal::Config::default();

		config.cpu_clock = esp_hal::clock::CpuClock::max();

		config
	});

	let timer0 = esp_hal::timer::systimer::SystemTimer::new(p.SYSTIMER);
	esp_hal_embassy::init(timer0.alarm0);

	// create buzzer controller
	let buzzer = Buzzer::from(p.GPIO14.degrade());
	let synth = Synth::new(config.synth_config.clone());

	let button_bounce_time = Duration::from_millis(config.button_bounce_ms);

	// create button pollers
	let buttons = [
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
	.into_iter()
	.map(|(f, p)| (f, Button::from((p, button_bounce_time))));

	// create alphanum controller
	let mut alphanum = Alphanum::new_esp_hal(p.I2C0.into(), p.GPIO11.into(), p.GPIO12.into())?;
	alphanum.set_brightness(config.brightness).await?;
	alphanum.ascii_uppercase(config.ascii_uppercase);

	let humid_temp = Dht11::from(p.GPIO13.degrade());

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

	// start display task
	spawner.spawn(alphanum_task(alphanum, ALPHANUM_CHANNEL.receiver()))?;

	// start alarm task
	spawner.spawn(alarm_task(EVENT_CHANNEL.sender()))?;

	// start timer task
	spawner.spawn(timer_task(TIMER_CHANNEL.receiver(), EVENT_CHANNEL.sender()))?;

	// start sensors task
	spawner.spawn(sensor_task(
		humid_temp,
		SENSOR_CHANNEL.receiver(),
		EVENT_CHANNEL.sender(),
	))?;

	let mut state_transition = StateTransition::Clock;

	let event_receiver = EVENT_CHANNEL.receiver();

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
			StateTransition::Alarm => {
				StateAlarm::new(ALPHANUM_CHANNEL.sender(), PLAYER_CHANNEL.sender()).into()
			}
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
				StateSensors::new(ALPHANUM_CHANNEL.sender(), SENSOR_CHANNEL.sender()).into()
			}
		};

		state_transition = process_state(state, &event_receiver).await;
	}
}

async fn process_state(
	mut state: impl State,
	event_receiver: &Receiver<EventMessage, 1>,
) -> StateTransition {
	state.init().await;

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

	state.finish().await;

	state_transition
}
