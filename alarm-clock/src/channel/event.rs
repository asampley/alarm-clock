use defmt::Format;

use embassy_time::Instant;

use crate::circuit::bmp::BmpReading;
use crate::circuit::dht::Dht11Reading;

#[derive(Format)]
pub enum ButtonEvent {
	Press(ButtonFunction),

	Release(ButtonFunction),
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

#[derive(Format)]
pub enum TimerEvent {
	Start(Instant),
	End,
}

#[derive(Format)]
pub enum SensorEvent {
	Dht(Dht11Reading),
	DhtError,
	Bmp(BmpReading),
	BmpError,
}
