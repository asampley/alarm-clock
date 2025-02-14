use defmt::Format;

use embassy_time::{Duration, Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::digital::Wait;

use crate::{debug, warn};

const START_SIGNAL_DURATION: Duration = Duration::from_millis(20);
const BIT_0_UP: Duration = Duration::from_micros(26);
const BIT_1_UP: Duration = Duration::from_micros(70);
const BIT_X_UP_MID: Duration =
	Duration::from_ticks((BIT_0_UP.as_ticks() + BIT_1_UP.as_ticks()) / 2);

pub struct Dht<Pin: InputPin + OutputPin + Wait> {
	pin: Pin,
}

pub struct Dht11<Pin: InputPin + OutputPin + Wait>(Dht<Pin>);

impl<Pin: InputPin + OutputPin + Wait> Dht11<Pin> {
	/// expects an open drain pin
	pub fn new(mut pin: Pin) -> Self {
		pin.set_high().unwrap();

		Self(Dht { pin })
	}

	pub async fn read(&mut self) -> Result<Dht11Reading, Pin::Error> {
		let bytes = self.0.read_bytes::<5>().await?;

		let humidity = u16::from_be_bytes(bytes[0..2].try_into().unwrap());
		let temperature = u16::from_be_bytes(bytes[2..4].try_into().unwrap());
		let checksum = bytes[4];

		if bytes.iter().fold(0_u8, |v, next| v.wrapping_add(*next)) != checksum {
			warn!("Checksum didn't match");
		}

		Ok(Dht11Reading {
			humidity,
			temperature,
		})
	}

	pub fn read_sync(&mut self) -> Result<Dht11Reading, Pin::Error> {
		let bytes = self.0.read_bytes_sync::<5>()?;

		let humidity = u16::from_be_bytes(bytes[0..2].try_into().unwrap());
		let temperature = u16::from_be_bytes(bytes[2..4].try_into().unwrap());
		let checksum = bytes[4];

		if bytes.iter().fold(0_u8, |v, next| v.wrapping_add(*next)) != checksum {
			warn!("Checksum didn't match");
		}

		Ok(Dht11Reading {
			humidity,
			temperature,
		})
	}
}

#[derive(Copy, Clone, Debug, Format)]
pub struct Dht11Reading {
	pub humidity: u16,
	pub temperature: u16,
}

impl<Pin: InputPin + OutputPin + Wait> Dht<Pin> {
	pub async fn read_bytes<const N: usize>(&mut self) -> Result<[u8; N], Pin::Error> {
		let mut output = [0; N];

		self.start_signal().await?;

		debug!("Sent start signal to dht");

		debug!("Pin {}", if self.pin.is_high()? { "high" } else { "low" });

		self.pin.wait_for_rising_edge().await?;

		for i in 0..N {
			output[i] = self.read_byte().await?;

			debug!("Read byte {:?}", output[i]);
		}

		self.pin.set_high()?;

		Ok(output)
	}

	pub fn read_bytes_sync<const N: usize>(&mut self) -> Result<[u8; N], Pin::Error> {
		let mut output = [0; N];

		self.start_signal_sync()?;

		debug!("Sent start signal to dht");

		debug!("Pin {}", if self.pin.is_high()? { "high" } else { "low" });

		while self.pin.is_high()? {}
		while self.pin.is_low()? {}

		for i in 0..N {
			output[i] = self.read_byte_sync()?;

			debug!("Read byte {:?}", output[i]);
		}

		self.pin.set_high()?;

		Ok(output)
	}

	async fn start_signal(&mut self) -> Result<(), Pin::Error> {
		self.pin.set_low()?;
		Timer::after(START_SIGNAL_DURATION).await;
		self.pin.set_high()?;

		Ok(())
	}

	fn start_signal_sync(&mut self) -> Result<(), Pin::Error> {
		self.pin.set_low()?;
		embassy_time::block_for(START_SIGNAL_DURATION);
		self.pin.set_high()?;

		Ok(())
	}

	async fn read_byte(&mut self) -> Result<u8, Pin::Error> {
		let mut output = 0;

		for bit in 0..8 {
			output |= (self.read_bit().await? as u8) << bit;
		}

		Ok(output)
	}

	fn read_byte_sync(&mut self) -> Result<u8, Pin::Error> {
		let mut output = 0;

		for bit in 0..8 {
			output |= (self.read_bit_sync()? as u8) << bit;
		}

		Ok(output)
	}

	async fn read_bit(&mut self) -> Result<bool, Pin::Error> {
		self.pin.wait_for_rising_edge().await?;
		let start = Instant::now();
		self.pin.wait_for_falling_edge().await?;

		Ok(Instant::now() - start < BIT_X_UP_MID)
	}

	fn read_bit_sync(&mut self) -> Result<bool, Pin::Error> {
		while self.pin.is_high()? {}
		while self.pin.is_low()? {}
		let start = Instant::now();
		while self.pin.is_high()? {}

		Ok(Instant::now() - start < BIT_X_UP_MID)
	}
}
