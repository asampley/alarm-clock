use defmt::Format;

use embassy_time::Instant;

use heapless::{String, Vec};

use midly::MidiMessage;
use midly::num::u4;

use crate::circuit::alphanum::{BlinkRate, Char};
use crate::midi_dir::Midi;
use crate::util::Calf;

use super::event::*;

#[derive(Format)]
pub enum EventMessage {
	Button(ButtonEvent),
	Song(SongEvent),
	Timer(TimerEvent),
	Sensor(SensorEvent),
	Alarm,
}

impl From<ButtonEvent> for EventMessage {
	fn from(from: ButtonEvent) -> Self {
		EventMessage::Button(from)
	}
}

impl From<SongEvent> for EventMessage {
	fn from(from: SongEvent) -> Self {
		EventMessage::Song(from)
	}
}

impl From<TimerEvent> for EventMessage {
	fn from(from: TimerEvent) -> Self {
		EventMessage::Timer(from)
	}
}

#[derive(Format)]
pub enum SynthMessage {
	Midi {
		#[defmt(Debug2Format)]
		channel: u4,
		#[defmt(Debug2Format)]
		message: MidiMessage
	},
	Clear,
}

#[derive(Format)]
pub enum PlayerMessage {
	Loop(Midi),
	Play(Midi),
	Stop,
}

#[derive(Format)]
pub enum TimerMessage {
	Seconds(u64),
	Remove(Instant),
}

#[derive(Format)]
pub enum AlphanumMessage {
	Static(Calf<'static, String<4>>),
	StaticRendered([Char; 4]),
	Loop(&'static str),
	// arbitrary limit of 256
	LoopRendered(Vec<Char, 256>),
	Empty,
	Blink(BlinkRate),
}

#[derive(Clone, Copy, Format)]
pub enum SensorMessage {
	Update,
}
