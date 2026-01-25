use defmt::Format;

use embassy_time::Instant;

use heapless::{String, Vec};

use midly::MidiMessage;
use midly::num::u4;

use crate::circuit::alphanum::{BlinkRate, Char};
use crate::circuit::bmp::BmpReading;
use crate::circuit::dht::Dht11Reading;
use crate::midi_dir::Midi;
use crate::util::Calf;

#[derive(Format)]
pub enum EventMessage {
	Button(ButtonEvent),
	Song(SongEvent),
	Timer(TimerEvent),
	Sensor(SensorEvent),
	Alarm,
}

#[derive(Format)]
pub enum ButtonEvent {
	Press(ButtonFunction),

	Release(ButtonFunction),
}

impl From<ButtonEvent> for EventMessage {
	fn from(from: ButtonEvent) -> Self {
		EventMessage::Button(from)
	}
}

#[derive(Copy, Clone, Eq, Format, PartialEq)]
pub enum ButtonFunction {
	Direction(ButtonDirection),
	Select,
}

#[derive(Copy, Clone, Eq, Format, PartialEq)]
pub enum ButtonDirection {
	Prev,
	Next,
}

#[derive(Format)]
pub enum SongEvent {
	Start(&'static str),
	End(&'static str),
}

impl From<SongEvent> for EventMessage {
	fn from(from: SongEvent) -> Self {
		EventMessage::Song(from)
	}
}

#[derive(Format)]
pub enum TimerEvent {
	Start(Instant),
	End,
}

impl From<TimerEvent> for EventMessage {
	fn from(from: TimerEvent) -> Self {
		EventMessage::Timer(from)
	}
}

#[derive(Format)]
pub enum SensorEvent {
	Dht(Dht11Reading),
	DhtError,
	Bmp(BmpReading),
	BmpError,
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

#[derive(Format)]
pub enum TimerMessage {
	Seconds(u64),
	Remove(Instant),
}

#[derive(Debug)]
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
