use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{ Duration, Instant };

use crate::{ TIME_ZERO, ALARM_TIME, ALARM_SONG, ClockTime };

use crate::selector::{ BinarySelector, LinearSelector, Selector };

use crate::message::{
	AlphanumMessage,
	PlayerMessage,
	EventMessage,
	ButtonEvent,
	SongEvent,
};

fn send_time(alphanum_sender: &mut Sender<AlphanumMessage>, time: ClockTime) {
	alphanum_sender.send(AlphanumMessage::Static(time.as_chars()))
		.expect("Unable to send selected time to alphanum");
}

fn set_time(time: ClockTime) {
	*TIME_ZERO.write() = Instant::now() - Duration::from(time)
}


pub trait State {
	fn init(&mut self) {}

	fn finish(&mut self) {}

	fn event(&mut self, event: EventMessage) -> Option<StateId> {
		match event {
			EventMessage::Button(button) => match button {
				ButtonEvent::Press(x) => self.button_press(x),
				ButtonEvent::Release(x) => self.button_release(x),
			}
			EventMessage::Song(song) => match song {
				SongEvent::Start(name) => self.song_start(name),
				SongEvent::End(name) => self.song_end(name),
			}
		}
	}

	fn button_press(&mut self, _button_id: u8) -> Option<StateId> {
		None
	}

	fn button_release(&mut self, _button_id: u8) -> Option<StateId> {
		None
	}

	fn song_start(&mut self, _name: String) -> Option<StateId> {
		None
	}

	fn song_end(&mut self, _name: String) -> Option<StateId> {
		None
	}
}

#[derive(Clone, Copy, Debug)]
pub enum StateId {
	Clock,
	ModeSelect,
	ClockSet,
	AlarmTime,
	AlarmSong,
	Play,
	Bad,
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
			Bad => "Bad",
		}
	}
}

impl fmt::Display for StateId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

pub struct StateClock {
	alphanum_sender: Sender<AlphanumMessage>,
	player_sender: Sender<PlayerMessage>,
}

impl StateClock {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage>,
		player_sender: Sender<PlayerMessage>,
	) -> Self {
		Self { alphanum_sender, player_sender }
	}
}

impl State for StateClock {
	fn init(&mut self) {
		self.alphanum_sender.send(AlphanumMessage::Time).unwrap();
	}

	fn button_press(&mut self, button_id: u8) -> Option<StateId> {
		match button_id {
			0 | 1 => {
				self.player_sender.send(PlayerMessage::Stop)
					.expect("Unable to stop alarm");

				None
			}
			2 => Some(StateId::ModeSelect),
			_ => None,
		}
	}
}

pub struct StateModeSelect {
	alphanum_sender: Sender<AlphanumMessage>,
	mode_selector: LinearSelector<StateId>,
}

impl StateModeSelect {
	pub fn new(alphanum_sender: Sender<AlphanumMessage>) -> Self {
		Self {
			alphanum_sender,
			mode_selector: LinearSelector::new(vec![
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
	fn init(&mut self) {
		self.alphanum_sender.send(AlphanumMessage::Loop(self.mode_selector.curr().to_string())).unwrap();
	}

	fn button_press(&mut self, button_id: u8) -> Option<StateId> {
		match button_id {
			0 | 1 => {
				let state = if button_id == 0 {
					self.mode_selector.incr()
				} else {
					self.mode_selector.decr()
				};

				self.alphanum_sender.send(AlphanumMessage::Loop(state.to_string())).unwrap();

				None
			}
			2 => Some(*self.mode_selector.curr()),
			_ => None,
		}
	}
}

pub struct StateClockSet {
	alphanum_sender: Sender<AlphanumMessage>,
	time_selector: BinarySelector<ClockTime>,
}

impl StateClockSet {
	pub fn new(alphanum_sender: Sender<AlphanumMessage>) -> Self {
		Self {
			alphanum_sender,
			time_selector: BinarySelector::new(
				(0..24*60)
					.map(|i| ClockTime::new(i))
					.collect::<Vec<_>>()
			),
		}
	}
}

impl State for StateClockSet {
	fn init(&mut self) {
		send_time(&mut self.alphanum_sender, *self.time_selector.curr());
	}

	fn button_press(&mut self, button_id: u8) -> Option<StateId> {
		match button_id {
			0 | 1 => {
				let time = if button_id == 0 {
					self.time_selector.incr()
				} else {
					self.time_selector.decr()
				};

				send_time(&mut self.alphanum_sender, *time);

				None
			}
			2 => {
				set_time(*self.time_selector.curr());

				Some(StateId::Clock)
			}
			_ => None,
		}
	}
}

pub struct StateAlarmTimeSet {
	alphanum_sender: Sender<AlphanumMessage>,
	time_selector: BinarySelector<ClockTime>,
}

impl StateAlarmTimeSet {
	pub fn new(alphanum_sender: Sender<AlphanumMessage>) -> Self {
		Self {
			alphanum_sender,
			time_selector: BinarySelector::new(
				(0..24*60)
					.map(|i| ClockTime::new(i))
					.collect::<Vec<_>>()
			),
		}
	}
}

impl State for StateAlarmTimeSet {
	fn init(&mut self) {
		send_time(&mut self.alphanum_sender, *self.time_selector.curr());
	}

	fn button_press(&mut self, button_id: u8) -> Option<StateId> {
		match button_id {
			0 | 1 => {
				let time = if button_id == 0 {
					self.time_selector.incr()
				} else {
					self.time_selector.decr()
				};

				send_time(&mut self.alphanum_sender, *time);

				None
			}
			2 => {
				*ALARM_TIME.write() = Some(*self.time_selector.curr());

				Some(StateId::Clock)
			}
			_ => None,
		}
	}
}

pub struct StateAlarmSongSet {
	alphanum_sender: Sender<AlphanumMessage>,
	player_sender: Sender<PlayerMessage>,
	midi_selector: LinearSelector<PathBuf>,
}

impl StateAlarmSongSet {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage>,
		player_sender: Sender<PlayerMessage>,
		midi_files: Vec<PathBuf>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
			midi_selector: LinearSelector::new(midi_files.clone()),
		}
	}
}

impl State for StateAlarmSongSet {
	fn button_press(&mut self, button_id: u8) -> Option<StateId> {
		match button_id {
			0 | 1 => {
				let midi_file = if button_id == 0 {
					self.midi_selector.incr()
				} else {
					self.midi_selector.decr()
				};

				self.player_sender.send(PlayerMessage::Play(midi_file.clone()))
					.expect("Unable to send midi file name");

				None
			}
			2 => {
				*ALARM_SONG.write() = Some(self.midi_selector.curr().clone());

				self.player_sender.send(PlayerMessage::Stop)
					.expect("Unable to stop currently playing");

				Some(StateId::Clock)
			}
			_ => None,
		}
	}

	fn song_start(&mut self, name: String) -> Option<StateId> {
		println!("Now playing {:?}", name);
		self.alphanum_sender.send(AlphanumMessage::Loop(name)).unwrap();

		None
	}

	fn song_end(&mut self, name: String) -> Option<StateId> {
		println!("Stopped playing {:?}", name);
		self.alphanum_sender.send(AlphanumMessage::Loop("Play".to_string())).unwrap();

		None
	}
}

pub struct StatePlay {
	alphanum_sender: Sender<AlphanumMessage>,
	player_sender: Sender<PlayerMessage>,
	midi_selector: LinearSelector<PathBuf>,
}

impl StatePlay {
	pub fn new(
		alphanum_sender: Sender<AlphanumMessage>,
		player_sender: Sender<PlayerMessage>,
		midi_files: Vec<PathBuf>,
	) -> Self {
		Self {
			alphanum_sender,
			player_sender,
			midi_selector: LinearSelector::new(midi_files.clone()),
		}
	}
}

impl State for StatePlay {
	fn button_press(&mut self, button_id: u8) -> Option<StateId> {
		match button_id {
			0 | 1 => {
				let midi_file = if button_id == 0 {
					self.midi_selector.incr()
				} else {
					self.midi_selector.decr()
				};

				self.player_sender.send(PlayerMessage::Play(midi_file.clone()))
					.expect("Unable to send midi file name");

				None
			}
			2 => {
				self.player_sender.send(PlayerMessage::Stop)
					.expect("Unable to stop currently playing");

				Some(StateId::Clock)
			}
			_ => None,
		}
	}

	fn song_start(&mut self, name: String) -> Option<StateId> {
		println!("Now playing {:?}", name);
		self.alphanum_sender.send(AlphanumMessage::Loop(name)).unwrap();

		None
	}

	fn song_end(&mut self, name: String) -> Option<StateId> {
		println!("Stopped playing {:?}", name);
		self.alphanum_sender.send(AlphanumMessage::Loop("Play".to_string())).unwrap();

		None
	}
}
