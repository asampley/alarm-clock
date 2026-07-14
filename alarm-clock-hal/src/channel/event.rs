use embassy_time::Instant;

use crate::circuit::bmp::BmpReading;
use crate::circuit::dht::Dht11Reading;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ButtonEvent {
	Press(ButtonFunction),

	Release(ButtonFunction),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ButtonFunction {
	Direction(ButtonDirection),
	Select,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ButtonDirection {
	Prev,
	Next,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SongEvent {
	Start(&'static str),
	End(&'static str),
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TimerEvent {
	Start(Instant),
	End,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SensorEvent {
	Dht(Dht11Reading),
	DhtError,
	Bmp(BmpReading),
	BmpError,
}
