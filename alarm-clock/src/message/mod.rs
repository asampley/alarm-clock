use defmt::Format;
use embassy_time::Instant;
use heapless::String;
use midly::num::u4;
use midly::MidiMessage;

use crate::circuit::alphanum::BlinkRate;
use crate::circuit::dht::Dht11Reading;
use crate::midi_dir::Midi;
use crate::util::Calf;

#[derive(Debug, Format)]
pub enum EventMessage {
	Button(ButtonEvent),
	Song(SongEvent),
	Timer(TimerEvent),
	Sensor(SensorEvent),
	Alarm,
}

#[derive(Debug, Format)]
pub enum ButtonEvent {
	Press(ButtonFunction),
	Release(ButtonFunction),
}

impl From<ButtonEvent> for EventMessage {
	fn from(from: ButtonEvent) -> Self {
		EventMessage::Button(from)
	}
}

#[derive(Copy, Clone, Debug, Eq, Format, PartialEq)]
pub enum ButtonFunction {
	Direction(ButtonDirection),
	Select,
}

#[derive(Copy, Clone, Debug, Eq, Format, PartialEq)]
pub enum ButtonDirection {
	Prev,
	Next,
}

#[derive(Debug, Format)]
pub enum SongEvent {
	Start(&'static str),
	End(&'static str),
}

impl From<SongEvent> for EventMessage {
	fn from(from: SongEvent) -> Self {
		EventMessage::Song(from)
	}
}

#[derive(Debug, Format)]
pub enum TimerEvent {
	Start(Instant),
	End,
}

impl From<TimerEvent> for EventMessage {
	fn from(from: TimerEvent) -> Self {
		EventMessage::Timer(from)
	}
}

#[derive(Debug, Format)]
pub enum SensorEvent {
	Dht(Dht11Reading),
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
pub enum TimerMessage {
	Seconds(u64),
	Remove(Instant),
}

#[derive(Debug)]
pub enum AlphanumMessage {
	Static(Calf<'static, String<4>>),
	Loop(&'static str),
	Empty,
	Blink(BlinkRate),
}

#[derive(Debug)]
pub enum SensorMessage {
	Update,
}
