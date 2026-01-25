use defmt::Format;

use embassy_time::{Duration, Instant};
use embedded_hal::digital::{ErrorType, InputPin, OutputPin};

use crate::{debug, warn};

const START_SIGNAL_DURATION: Duration = Duration::from_millis(20);
const START_SIGNAL_FINISH_WAIT: Duration = Duration::from_micros(40);
const BIT_0_UP: Duration = Duration::from_micros(26);
const BIT_1_UP: Duration = Duration::from_micros(70);
const BIT_X_UP_MID: Duration =
	Duration::from_ticks((BIT_0_UP.as_ticks() + BIT_1_UP.as_ticks()) / 2);
const BIT_TIMEOUT: Duration = Duration::from_micros(1000);
const ACKNOWLEDGE_TIMEOUT: Duration = Duration::from_millis(10);

/// Required to be concrete for embassy tasks
pub type Pin = impl InputPin + OutputPin + ErrorType<Error: defmt::Format>;
pub type Error = <Pin as ErrorType>::Error;

#[derive(Format)]
pub enum SyncError {
	Pin(Error),
	Timeout,
}

pub struct Dht {
	pin: Pin,
}

pub struct Dht11(Dht);

impl Dht11 {
	/// expects an open drain pin
	pub fn new(mut pin: Pin) -> Self {
		pin.set_high().unwrap();

		Self(Dht { pin })
	}

	pub fn read(&mut self) -> Result<Dht11Reading, SyncError> {
		Ok(self.0.read_bytes().map(Self::parse)?)
	}

	fn parse(bytes: [u8; 5]) -> Dht11Reading {
		let humidity = bytes[0];
		let temperature = bytes[2];

		let checksum = bytes[4];

		if bytes[0..4]
			.iter()
			.fold(0_u8, |v, next| v.wrapping_add(*next))
			!= checksum
		{
			warn!("Checksum didn't match");
		}

		Dht11Reading {
			humidity,
			temperature,
		}
	}
}

#[derive(Copy, Clone, Format)]
pub struct Dht11Reading {
	pub humidity: u8,
	pub temperature: u8,
}

impl Dht {
	pub fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N], SyncError> {
		let mut output = [0; N];

		debug!("start signal");
		self.start_signal().map_err(SyncError::Pin)?;

		// wait for acknowledgement
		let timeout = Instant::now() + ACKNOWLEDGE_TIMEOUT;
		self.wait_for_high(timeout)?;
		self.wait_for_low(timeout)?;

		debug!("start acknowledged");

		for byte in &mut output {
			*byte = self.read_byte()?;
		}

		self.pin.set_high().map_err(SyncError::Pin)?;

		Ok(output)
	}

	fn start_signal(&mut self) -> Result<(), Error> {
		self.pin.set_low()?;
		embassy_time::block_for(START_SIGNAL_DURATION);
		self.pin.set_high()?;

		embassy_time::block_for(START_SIGNAL_FINISH_WAIT);

		Ok(())
	}

	fn read_byte(&mut self) -> Result<u8, SyncError> {
		let mut output = 0;

		for bit in (0..8).rev() {
			output |= (self.read_bit()? as u8) << bit;
		}

		Ok(output)
	}

	// Assumes the pin is already low.
	fn read_bit(&mut self) -> Result<bool, SyncError> {
		let timeout = Instant::now() + BIT_TIMEOUT;

		self.wait_for_high(timeout)?;

		let start = Instant::now();

		self.wait_for_low(timeout)?;

		Ok(Instant::now() - start > BIT_X_UP_MID)
	}

	fn timeout(timeout: Instant) -> Result<(), SyncError> {
		if Instant::now() > timeout {
			Err(SyncError::Timeout)
		} else {
			Ok(())
		}
	}

	fn wait_for_high(&mut self, timeout: Instant) -> Result<(), SyncError> {
		while self.pin.is_low().map_err(SyncError::Pin)? {
			Self::timeout(timeout)?
		}

		Ok(())
	}

	fn wait_for_low(&mut self, timeout: Instant) -> Result<(), SyncError> {
		while self.pin.is_high().map_err(SyncError::Pin)? {
			Self::timeout(timeout)?
		}

		Ok(())
	}
}
