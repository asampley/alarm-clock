use core::fmt;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};
use embassy_sync::channel::Sender;
use embassy_sync::lazy_lock::LazyLock;

use enum_dispatch::enum_dispatch;
use heapless::Vec;

use crate::borrow::Calf;
use crate::midi_dir::Midi;
use crate::time::set_time;
use crate::{ClockTime, ALARM_SONG, ALARM_TIME, MIDI_DIR};

use crate::selector::{BinarySelector, LinearSelector, Selector};

use crate::message::{
	AlphanumMessage, ButtonDirection, ButtonEvent, ButtonFunction, EventMessage, PlayerMessage,
	SongEvent,
};

static TIMES: LazyLock<Vec<ClockTime, { 24 * 60 }>> = LazyLock::new(|| {
	(0..24 * 60)
		.map(|i| ClockTime::new(i))
		.collect::<Vec<_, { 24 * 60 }>>()
});

async fn send_time<M: RawMutex, const SIZE: usize>(
	alphanum_sender: &mut Sender<'_, M, AlphanumMessage, SIZE>,
	time: ClockTime,
) {
	alphanum_sender
		.send(AlphanumMessage::Static(Calf::Owned(time.as_chars())))
		.await
}

#[enum_dispatch]
#[allow(async_fn_in_trait)]
pub trait State {
	async fn init(&mut self) {}

	async fn finish(&mut self) {}

	async fn event(&mut self, event: EventMessage) -> Option<StateId> {
		match event {
			EventMessage::Button(button) => match button {
				ButtonEvent::Press(x) => self.button_press(x).await,
				ButtonEvent::Release(x) => self.button_release(x).await,
			},
			EventMessage::Song(song) => match song {
				SongEvent::Start(name) => self.song_start(name).await,
				SongEvent::End(name) => self.song_end(name).await,
			},
		}
	}

	async fn button_press(&mut self, _button_function: ButtonFunction) -> Option<StateId> {
		None
	}

	async fn button_release(&mut self, _button_function: ButtonFunction) -> Option<StateId> {
		None
	}

	async fn song_start(&mut self, _name: &'static str) -> Option<StateId> {
		None
	}

	async fn song_end(&mut self, _name: &'static str) -> Option<StateId> {
		None
	}
}

#[derive(Clone, Copy, Debug, defmt::Format)]
pub enum StateId {
	Clock,
	ModeSelect,
	ClockSet,
	AlarmTime,
	AlarmSong,
	Play,
}

#[enum_dispatch(State)]
pub enum ConcreteState {
	StateClock(StateClock),
	StateModeSelect(StateModeSelect),
	StateClockSet(StateClockSet),
	StateAlarmTime(StateAlarmTimeSet),
	StateAlarmSong(StateAlarmSongSet),
	StatePlay(StatePlay),
}

impl StateId {
	fn as_str(&self) -> &'static str {
		use StateId::*;

		match self {
			Clock => "Clock",
			ModeSelect => "Mode Select",
			ClockSet => "Clock Set",
			AlarmTime => "Alarm Time",
			AlarmSong => "Alarm Song",
			Play => "Play",
		}
	}
}

impl fmt::Display for StateId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

pub struct StateClock {
	alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
	player_sender: Sender<'static, CriticalSectionRawMutex, PlayerMessage, 1>,
}

impl StateClock {
	pub fn new(
		alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
		player_sender: Sender<'static, CriticalSectionRawMutex, PlayerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
		}
	}
}

impl State for StateClock {
	async fn init(&mut self) {
		self.alphanum_sender.send(AlphanumMessage::Time).await;
	}

	async fn button_press(&mut self, button_function: ButtonFunction) -> Option<StateId> {
		self.player_sender.send(PlayerMessage::Stop).await;

		match button_function {
			ButtonFunction::Select => Some(StateId::ModeSelect),
			_ => None,
		}
	}
}

pub struct StateModeSelect {
	alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
	mode_selector: LinearSelector<'static, StateId>,
}

impl StateModeSelect {
	pub fn new(
		alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			mode_selector: LinearSelector::new(&[
				StateId::Clock,
				StateId::ClockSet,
				StateId::AlarmTime,
				StateId::AlarmSong,
				StateId::Play,
			]),
		}
	}
}

impl State for StateModeSelect {
	async fn init(&mut self) {
		self.alphanum_sender
			.send(AlphanumMessage::Loop(self.mode_selector.curr().as_str()))
			.await
	}

	async fn button_press(&mut self, button_function: ButtonFunction) -> Option<StateId> {
		match button_function {
			ButtonFunction::Select => Some(*self.mode_selector.curr()),
			ButtonFunction::Direction(dir) => {
				let state = match dir {
					ButtonDirection::Prev => self.mode_selector.decr(),
					ButtonDirection::Next => self.mode_selector.incr(),
				};

				self.alphanum_sender
					.send(AlphanumMessage::Loop(state.as_str()))
					.await;

				None
			}
		}
	}
}

pub struct StateClockSet {
	alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
	time_selector: BinarySelector<'static, ClockTime>,
}

impl StateClockSet {
	pub fn new(
		alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			time_selector: BinarySelector::new(&TIMES.get()),
		}
	}
}

impl State for StateClockSet {
	async fn init(&mut self) {
		send_time(&mut self.alphanum_sender, *self.time_selector.curr()).await;
	}

	async fn button_press(&mut self, button_function: ButtonFunction) -> Option<StateId> {
		match button_function {
			ButtonFunction::Select => {
				set_time(*self.time_selector.curr()).await;

				Some(StateId::Clock)
			}
			ButtonFunction::Direction(dir) => {
				let time = match dir {
					ButtonDirection::Prev => self.time_selector.decr(),
					ButtonDirection::Next => self.time_selector.incr(),
				};

				send_time(&mut self.alphanum_sender, *time).await;

				None
			}
		}
	}
}

pub struct StateAlarmTimeSet {
	alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
	time_selector: BinarySelector<'static, ClockTime>,
}

impl StateAlarmTimeSet {
	pub fn new(
		alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			time_selector: BinarySelector::new(&TIMES.get()),
		}
	}
}

impl State for StateAlarmTimeSet {
	async fn init(&mut self) {
		send_time(&mut self.alphanum_sender, *self.time_selector.curr()).await;
	}

	async fn button_press(&mut self, button_function: ButtonFunction) -> Option<StateId> {
		match button_function {
			ButtonFunction::Select => {
				ALARM_TIME.sender().send(*self.time_selector.curr());

				Some(StateId::Clock)
			}
			ButtonFunction::Direction(dir) => {
				let time = match dir {
					ButtonDirection::Prev => self.time_selector.decr(),
					ButtonDirection::Next => self.time_selector.incr(),
				};

				send_time(&mut self.alphanum_sender, *time).await;

				None
			}
		}
	}
}

pub struct StateAlarmSongSet {
	alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
	player_sender: Sender<'static, CriticalSectionRawMutex, PlayerMessage, 1>,
	midi_selector: LinearSelector<'static, Midi>,
}

impl StateAlarmSongSet {
	pub fn new(
		alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
		player_sender: Sender<'static, CriticalSectionRawMutex, PlayerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
			midi_selector: LinearSelector::new(&MIDI_DIR),
		}
	}
}

impl State for StateAlarmSongSet {
	async fn init(&mut self) {
		let midi_file = self.midi_selector.curr();

		self.player_sender
			.send(PlayerMessage::Play(*midi_file))
			.await;
	}

	async fn button_press(&mut self, button_function: ButtonFunction) -> Option<StateId> {
		match button_function {
			ButtonFunction::Select => {
				*ALARM_SONG.lock().await = *self.midi_selector.curr();

				self.player_sender.send(PlayerMessage::Stop).await;

				Some(StateId::Clock)
			}
			ButtonFunction::Direction(dir) => {
				let midi_file = match dir {
					ButtonDirection::Prev => self.midi_selector.decr(),
					ButtonDirection::Next => self.midi_selector.incr(),
				};

				self.player_sender
					.send(PlayerMessage::Play(*midi_file))
					.await;

				None
			}
		}
	}

	async fn song_start(&mut self, name: &'static str) -> Option<StateId> {
		self.alphanum_sender.send(AlphanumMessage::Loop(name)).await;

		None
	}

	async fn song_end(&mut self, _name: &'static str) -> Option<StateId> {
		self.alphanum_sender
			.send(AlphanumMessage::Static(Calf::Borrowed("Play")))
			.await;

		None
	}
}

pub struct StatePlay {
	alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
	player_sender: Sender<'static, CriticalSectionRawMutex, PlayerMessage, 1>,
	midi_selector: LinearSelector<'static, Midi>,
}

impl StatePlay {
	pub fn new(
		alphanum_sender: Sender<'static, CriticalSectionRawMutex, AlphanumMessage, 1>,
		player_sender: Sender<'static, CriticalSectionRawMutex, PlayerMessage, 1>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
			midi_selector: LinearSelector::new(&MIDI_DIR),
		}
	}
}

impl State for StatePlay {
	async fn init(&mut self) {
		let midi_file = self.midi_selector.curr();

		self.player_sender
			.send(PlayerMessage::Play(*midi_file))
			.await;
	}

	async fn button_press(&mut self, button_function: ButtonFunction) -> Option<StateId> {
		match button_function {
			ButtonFunction::Select => {
				self.player_sender.send(PlayerMessage::Stop).await;

				Some(StateId::Clock)
			}
			ButtonFunction::Direction(dir) => {
				let midi_file = match dir {
					ButtonDirection::Prev => self.midi_selector.decr(),
					ButtonDirection::Next => self.midi_selector.incr(),
				};

				self.player_sender
					.send(PlayerMessage::Play(*midi_file))
					.await;

				None
			}
		}
	}

	async fn song_start(&mut self, name: &'static str) -> Option<StateId> {
		self.alphanum_sender.send(AlphanumMessage::Loop(name)).await;

		None
	}

	async fn song_end(&mut self, _name: &'static str) -> Option<StateId> {
		self.alphanum_sender
			.send(AlphanumMessage::Loop("Play"))
			.await;

		None
	}
}
