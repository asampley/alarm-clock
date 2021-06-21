use std::convert::TryInto;
use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{ Duration, Instant };

use crate::TIME_ZERO;

use crate::selector::{ BinarySelector, LinearSelector, Selector };

use crate::message::{
	AlphanumMessage,
	PlayerMessage,
	EventMessage,
	ButtonEvent,
	SongEvent,
};

pub type StateFn<'a> = Box<dyn FnMut(EventMessage) -> Option<State> + 'a>;

#[derive(Clone, Copy, Debug)]
pub enum State {
	Time,
	ModeSelect,
	TimeSet,
	AlarmTime,
	AlarmSong,
	Play,
	Bad,
}

impl fmt::Display for State {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		use State::*;

		let name = match self {
			Time => "Time",
			ModeSelect => "Mode Select",
			TimeSet => "Time Set",
			AlarmTime => "Alarm Time",
			AlarmSong => "Alarm Song",
			Play => "Play",
			Bad => "Bad",
		};

		write!(f, "{}", name)
	}
}

pub fn state_time<'a>(
	alphanum_sender: &'a mut Sender<AlphanumMessage>
) -> StateFn<'a> {
	use EventMessage::*;
	use ButtonEvent::*;

	alphanum_sender.send(AlphanumMessage::Time).unwrap();

	Box::new(move |msg| match msg {
		Button(button) => match button {
			Press(x) => match x {
				2 => Some(State::ModeSelect),
				_ => None,
			}
			Release(_) => None,
		}
		Song(_) => None
	})
}

pub fn state_mode_select<'a>(
	alphanum_sender: &'a mut Sender<AlphanumMessage>
) -> StateFn<'a> {
	let mut mode_selector = LinearSelector::new(vec![
		State::TimeSet,
		State::AlarmTime,
		State::AlarmSong,
		State::Play,
	]);

	let mut state = *mode_selector.curr();
	alphanum_sender.send(AlphanumMessage::Text(state.to_string())).unwrap();

	Box::new(move |msg| match msg {
		EventMessage::Button(button) => match button {
			ButtonEvent::Press(x) => match x {
				0 | 1 => {
					state = *if x == 0 {
						mode_selector.incr()
					} else {
						mode_selector.decr()
					};

					alphanum_sender.send(AlphanumMessage::Text(state.to_string())).unwrap();

					None
				}
				2 => Some(state),
				_ => None,
			}
			ButtonEvent::Release(_) => None,
		}
		EventMessage::Song(_) => None,
	})
}

pub fn state_time_set<'a>(
	alphanum_sender: &'a mut Sender<AlphanumMessage>,
) -> StateFn<'a> {
	let mut time_selector = BinarySelector::new((0..24*60).collect::<Vec<_>>());

	let time = *time_selector.curr();

	let (hours, minutes) = (time / 60, time % 60);

	alphanum_sender.send(AlphanumMessage::Static(
		format!("{:02}{:02}", hours, minutes).chars().collect::<Vec<_>>().try_into().unwrap()
	)).expect("Unable to send selected time to alphanum");

	Box::new(move |msg| match msg {
		EventMessage::Button(button) => match button {
			ButtonEvent::Press(x) => match x {
				0 | 1 => {
					let time = if x == 0 {
						time_selector.incr()
					} else {
						time_selector.decr()
					};

					let (hours, minutes) = (time / 60, time % 60);

					alphanum_sender.send(AlphanumMessage::Static(
						format!("{:02}{:02}", hours, minutes).chars().collect::<Vec<_>>().try_into().unwrap()
					)).expect("Unable to send selected time to alphanum");

					None
				}
				2 => {
					*TIME_ZERO.write().unwrap()
						= Instant::now() - Duration::from_secs(time_selector.curr() * 60);

					Some(State::Time)
				}
				_ => None,
			}
			ButtonEvent::Release(_) => None,
		}
		EventMessage::Song(_) => None,
	})
}

pub fn state_alarm_time_set<'a>(
	alphanum_sender: &'a mut Sender<AlphanumMessage>,
) -> StateFn<'a> {
	let mut time_selector = BinarySelector::new((0..24*60).collect::<Vec<_>>());

	let time = *time_selector.curr();

	let (hours, minutes) = (time / 60, time % 60);

	alphanum_sender.send(AlphanumMessage::Static(
		format!("{:02}{:02}", hours, minutes).chars().collect::<Vec<_>>().try_into().unwrap()
	)).expect("Unable to send selected time to alphanum");

	Box::new(move |msg| match msg {
		EventMessage::Button(button) => match button {
			ButtonEvent::Press(x) => match x {
				0 | 1 => {
					let time = if x == 0 {
						time_selector.incr()
					} else {
						time_selector.decr()
					};

					let (hours, minutes) = (time / 60, time % 60);

					alphanum_sender.send(AlphanumMessage::Static(
						format!("{:02}{:02}", hours, minutes).chars().collect::<Vec<_>>().try_into().unwrap()
					)).expect("Unable to send selected time to alphanum");

					None
				}
				2 => {
					// TODO
					Some(State::Time)
				}
				_ => None,
			}
			ButtonEvent::Release(_) => None,
		}
		EventMessage::Song(_) => None,
	})
}

pub fn state_alarm_song_set<'a>(
	midi_files: &'a Vec<PathBuf>,
	alphanum_sender: &'a mut Sender<AlphanumMessage>,
	player_sender: &'a mut Sender<PlayerMessage>,
) -> StateFn<'a> {
	let mut midi_selector = LinearSelector::new(midi_files.clone());

	Box::new(move |msg| match msg {
		EventMessage::Button(button) => match button {
			ButtonEvent::Press(x) => match x {
				0 | 1 => {
					let midi_file = if x == 0 {
						midi_selector.incr()
					} else {
						midi_selector.decr()
					};

					player_sender.send(PlayerMessage::Play(midi_file.clone()))
						.expect("Unable to send midi file name");

					None
				}
				2 => {
					// TODO set song
					player_sender.send(PlayerMessage::Stop)
						.expect("Unable to stop currently playing");

					Some(State::Time)
				}
				_ => None,
			}
			ButtonEvent::Release(_) => None,
		}
		EventMessage::Song(song) => match song {
			SongEvent::Start(name) => {
				println!("Now playing {:?}", name);
				alphanum_sender.send(AlphanumMessage::Text(name)).unwrap();

				None
			}
			SongEvent::End(name) => {
				println!("Stopped playing {:?}", name);
				alphanum_sender.send(AlphanumMessage::Text("Play".to_owned())).unwrap();

				None
			}
		}
	})
}

pub fn state_play<'a>(
	midi_files: &'a Vec<PathBuf>,
	alphanum_sender: &'a mut Sender<AlphanumMessage>,
	player_sender: &'a mut Sender<PlayerMessage>,
) -> StateFn<'a> {
	let mut midi_selector = LinearSelector::new(midi_files.clone());

	Box::new(move |msg| match msg {
		EventMessage::Button(button) => match button {
			ButtonEvent::Press(x) => match x {
				0 | 1 => {
					let midi_file = if x == 0 {
						midi_selector.incr()
					} else {
						midi_selector.decr()
					};

					player_sender.send(PlayerMessage::Play(midi_file.clone()))
						.expect("Unable to send midi file name");

					None
				}
				2 => {
					player_sender.send(PlayerMessage::Stop)
						.expect("Unable to stop currently playing");

					Some(State::Time)
				}
				_ => None,
			}
			ButtonEvent::Release(_) => None,
		}
		EventMessage::Song(song) => match song {
			SongEvent::Start(name) => {
				println!("Now playing {:?}", name);
				alphanum_sender.send(AlphanumMessage::Text(name)).unwrap();

				None
			}
			SongEvent::End(name) => {
				println!("Stopped playing {:?}", name);
				alphanum_sender.send(AlphanumMessage::Text("Play".to_owned())).unwrap();

				None
			}
		}
	})
}
