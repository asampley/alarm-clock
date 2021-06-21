use std::path::PathBuf;

use crate::note::MidiNote;
use crate::circuit::BlinkRate;

#[derive(Debug)]
pub enum EventMessage {
	Button(ButtonEvent),
	Song(SongEvent),
}

#[derive(Debug)]
pub enum ButtonEvent {
	Press(u8),
	Release(u8),
}

#[derive(Debug)]
pub enum SongEvent {
	Start(String),
	End(String),
}

impl From<SongEvent> for EventMessage {
	fn from(from: SongEvent) -> Self {
		EventMessage::Song(from)
	}
}

impl From<ButtonEvent> for EventMessage {
	fn from(from: ButtonEvent) -> Self {
		EventMessage::Button(from)
	}
}

#[derive(Debug)]
pub enum BuzzerMessage {
	Clear,
	Note { on: bool, note: MidiNote },
}

#[derive(Debug)]
pub enum PlayerMessage {
	Play(PathBuf),
	Stop,
}

#[derive(Debug)]
pub enum AlphanumMessage {
	Static([char; 4]),
	Loop(String),
	Time,
	Empty,
	Blink(BlinkRate),
}
