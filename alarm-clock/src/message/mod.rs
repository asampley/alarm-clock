use heapless::String;
use midly::num::u4;
use midly::MidiMessage;

use crate::borrow::Calf;
use crate::circuit::alphanum::BlinkRate;
use crate::midi_dir::Midi;

#[derive(Debug)]
pub enum EventMessage {
	Button(ButtonEvent),
	Song(SongEvent),
}

#[derive(Debug)]
pub enum ButtonEvent {
	Press(ButtonFunction),
	Release(ButtonFunction),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ButtonFunction {
	Direction(ButtonDirection),
	Select,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ButtonDirection {
	Prev,
	Next,
}

#[derive(Debug)]
pub enum SongEvent {
	Start(&'static str),
	End(&'static str),
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
pub enum SynthMessage {
	Midi { channel: u4, message: MidiMessage },
	Clear,
}

#[derive(Debug)]
pub enum PlayerMessage {
	Loop(Midi),
	Play(Midi),
	Stop,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum AlphanumMessage {
	Static(Calf<'static, String<4>>),
	Loop(&'static str),
	Time,
	Empty,
	Blink(BlinkRate),
}
