#![no_std]
#![feature(impl_trait_in_assoc_type)]

use defmt_rtt as _;
use esp_backtrace as _;

pub use defmt::{error, info, trace, warn};

use embassy_executor::{SpawnError, Spawner};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::lazy_lock::LazyLock;

use embassy_time::Duration;

use esp_hal::gpio::Pin;

use message::{ButtonDirection, ButtonFunction};
use synth::Synth;
use thiserror::Error;

pub mod borrow;

pub mod circuit;
use circuit::hal::{Alphanum, Button, Buzzer};

pub mod tweaks;
use tweaks::Config;

pub mod message;
use message::{AlphanumMessage, EventMessage, PlayerMessage, SongEvent, SynthMessage};

pub mod midi_dir;
use midi_dir::Midi;
use midi_dir::MIDI_DIR;

pub mod selector;

pub mod synth;

pub mod states;
use states::{
	ConcreteState, State, StateAlarmSongSet, StateAlarmTimeSet, StateClock, StateClockSet, StateId,
	StateModeSelect, StatePlay,
};

pub mod tasks;
use tasks::alarm::alarm_task;
use tasks::alphanum::alphanum_task;
use tasks::buzzer::update_buzzer;
use tasks::input::poll_input;
use tasks::player::midi_player;

pub mod time;
use time::ClockTime;

type Channel<T, const CAP: usize> = embassy_sync::channel::Channel<CriticalSectionRawMutex, T, CAP>;
type Mutex<T> = embassy_sync::mutex::Mutex<CriticalSectionRawMutex, T>;
type Watch<T, const CAP: usize> = embassy_sync::watch::Watch<CriticalSectionRawMutex, T, CAP>;

// an instant that marks midnight
static ALARM_TIME: Watch<ClockTime, 1> = Watch::new();
static ALARM_SONG: Mutex<Midi> = Mutex::new(MIDI_DIR[0]);
static CONFIG: LazyLock<Config> = LazyLock::new(|| {
	serde_json_core::from_str(include_str!("../tweaks.json"))
		.unwrap()
		.0
});

const MIDI_NOTE_CAPACITY: usize = 64;

static MIDI_NOTE_CHANNEL: Channel<SynthMessage, MIDI_NOTE_CAPACITY> = Channel::new();
static EVENT_CHANNEL: Channel<EventMessage, 1> = Channel::new();
static PLAYER_CHANNEL: Channel<PlayerMessage, 1> = Channel::new();
static ALPHANUM_CHANNEL: Channel<AlphanumMessage, 1> = Channel::new();

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
	let buzzer = Buzzer::from((p.GPIO14.degrade(), Synth::new(config.synth_config.clone())));

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

	// start task to update buzzer
	spawner.spawn(update_buzzer(MIDI_NOTE_CHANNEL.receiver(), buzzer))?;

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
	spawner.spawn(alarm_task(PLAYER_CHANNEL.sender()))?;

	let mut state_id = StateId::Clock;

	loop {
		info!("Entering state {:?}", state_id);

		let mut state: ConcreteState = match state_id {
			StateId::Clock => {
				StateClock::new(ALPHANUM_CHANNEL.sender(), PLAYER_CHANNEL.sender()).into()
			}
			StateId::ModeSelect => StateModeSelect::new(ALPHANUM_CHANNEL.sender()).into(),
			StateId::ClockSet => StateClockSet::new(ALPHANUM_CHANNEL.sender()).into(),
			StateId::AlarmTime => StateAlarmTimeSet::new(ALPHANUM_CHANNEL.sender()).into(),
			StateId::AlarmSong => {
				StateAlarmSongSet::new(ALPHANUM_CHANNEL.sender(), PLAYER_CHANNEL.sender()).into()
			}
			StateId::Play => {
				StatePlay::new(ALPHANUM_CHANNEL.sender(), PLAYER_CHANNEL.sender()).into()
			}
		};

		state.init().await;

		let event_receiver = EVENT_CHANNEL.receiver();

		state_id = loop {
			let msg = event_receiver.receive().await;

			match &msg {
				EventMessage::Song(event) => match event {
					SongEvent::Start(name) => {
						info!("Now playing {:?}", name);
					}
					SongEvent::End(name) => {
						info!("Stopped playing {:?}", name);
					}
				},
				_ => (),
			}

			if let Some(next_state) = state.event(msg).await {
				break next_state;
			}
		};

		state.finish().await;
	}
}
