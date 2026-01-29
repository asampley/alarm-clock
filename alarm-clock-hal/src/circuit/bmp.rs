use defmt::Format;

use embassy_time::{Duration, Timer};

use embedded_hal::i2c::I2c as SyncI2c;
use embedded_hal_async::i2c::I2c as AsyncI2c;

use crate::circuit::alphanum::{Char, DOT, char_to_alphanum, digit_to_alphanum};
use crate::info;

const CONTROL_ADDRESS: u8 = 0xF4;
const CONTROL_TEMPERATURE: u8 = 0x2E;
const CONTROL_PRESSURE: [u8; 4] = [
	0x34, // oversampling setting 0
	0x74, // oversampling setting 1
	0xB4, // oversampling setting 2
	0xF4, // oversampling setting 3
];

const VALUE_START_ADDRESS: u8 = 0xF6;

const WAIT_TEMPERATURE: Duration = Duration::from_millis(5);
const WAIT_PRESSURE: [Duration; 4] = [
	Duration::from_millis(5),  // oversampling setting 0
	Duration::from_millis(8),  // oversampling setting 1
	Duration::from_millis(14), // oversampling setting 2
	Duration::from_millis(26), // oversampling setting 3
];

pub struct Bmp180 {
	address: u8,
	cal: Calibration,
}

pub struct Temperature(i64);
pub struct Pressure(i64);

#[derive(Format)]
pub struct BmpReading {
	pub temperature: Temperature,
	pub pressure: Pressure,
}

impl Format for Temperature {
	fn format(&self, fmt: defmt::Formatter<'_>) {
		defmt::write!(fmt, "{}.{}C", self.0 / 10, self.0 % 10);
	}
}

impl Format for Pressure {
	fn format(&self, fmt: defmt::Formatter<'_>) {
		defmt::write!(fmt, "{}Pa", self.0);
	}
}

impl Temperature {
	pub fn format_alphanum(&self) -> [Char; 4] {
		[
			digit_to_alphanum(self.0, 2),
			digit_to_alphanum(self.0, 1) | DOT,
			digit_to_alphanum(self.0, 0),
			char_to_alphanum('C'),
		]
	}
}

impl Pressure {
	pub fn format_alphanum(&self) -> [Char; 4] {
		[
			digit_to_alphanum(self.0, 5),
			digit_to_alphanum(self.0, 4),
			digit_to_alphanum(self.0, 3),
			char_to_alphanum('k'),
		]
	}
}

#[derive(Format)]
struct Calibration {
	ac1: i16,
	ac2: i16,
	ac3: i16,
	ac4: u16,
	ac5: u16,
	ac6: u16,
	b1: i16,
	b2: i16,
	mb: i16,
	mc: i16,
	md: i16,
}

impl Bmp180 {
	pub fn with_address<I2c: SyncI2c>(address: u8, i2c: &mut I2c) -> Result<Self, I2c::Error> {
		Ok(Self {
			address,
			cal: Self::read_calibration_data_sync(address, i2c)?,
		})
	}

	pub fn new<I2c: SyncI2c>(i2c: &mut I2c) -> Result<Self, I2c::Error> {
		Self::with_address(0x77, i2c)
	}

	fn read_calibration_data_sync<I2c: SyncI2c>(address: u8, i2c: &mut I2c) -> Result<Calibration, I2c::Error> {
		let mut buffer = [0; 22];
		SyncI2c::write_read(i2c, address, &[0xAA], &mut buffer)?;

		let calibration = Calibration {
			ac1: i16::from_be_bytes([buffer[0], buffer[1]]),
			ac2: i16::from_be_bytes([buffer[2], buffer[3]]),
			ac3: i16::from_be_bytes([buffer[4], buffer[5]]),
			ac4: u16::from_be_bytes([buffer[6], buffer[7]]),
			ac5: u16::from_be_bytes([buffer[8], buffer[9]]),
			ac6: u16::from_be_bytes([buffer[10], buffer[11]]),
			b1: i16::from_be_bytes([buffer[12], buffer[13]]),
			b2: i16::from_be_bytes([buffer[14], buffer[15]]),
			mb: i16::from_be_bytes([buffer[16], buffer[17]]),
			mc: i16::from_be_bytes([buffer[18], buffer[19]]),
			md: i16::from_be_bytes([buffer[20], buffer[21]]),
		};

		info!("{}", calibration);

		Ok(calibration)
	}

	async fn write_control<I2c: AsyncI2c>(&self, i2c: &mut I2c, value: u8) -> Result<(), I2c::Error> {
		AsyncI2c::write(i2c, self.address, &[CONTROL_ADDRESS, value]).await
	}

	/// Pressure calculation depends on temperature, just read them both
	pub async fn read<I2c: AsyncI2c>(&self, i2c: &mut I2c, oss: u8) -> Result<BmpReading, I2c::Error> {
		let oss = core::cmp::max(3, oss) as usize;

		self.write_control(i2c, CONTROL_TEMPERATURE).await?;

		Timer::after(WAIT_TEMPERATURE).await;
		let mut buffer = [0; 2];
		AsyncI2c::write_read(i2c, self.address, &[VALUE_START_ADDRESS], &mut buffer).await?;
		let ut = i64::from(i16::from_be_bytes(buffer));

		self.write_control(i2c, CONTROL_PRESSURE[oss]).await?;

		let ac1 = i64::from(self.cal.ac1);
		let ac2 = i64::from(self.cal.ac2);
		let ac3 = i64::from(self.cal.ac3);
		let ac4 = u64::from(self.cal.ac4);
		let b1 = i64::from(self.cal.b1);
		let b2 = i64::from(self.cal.b2);
		let mc = i64::from(self.cal.mc);
		let md = i64::from(self.cal.md);

		let x1 = (ut - i64::from(self.cal.ac6)) * i64::from(self.cal.ac5) / (1 << 15);
		let x2 = mc * (1 << 11) / (x1 + md);
		let b5 = x1 + x2;
		let t = (b5 + 8) / (1 << 4);

		let temperature = Temperature(t);

		Timer::after(WAIT_PRESSURE[oss]).await;
		let mut buffer = [0; 8];
		AsyncI2c::write_read(i2c, self.address, &[VALUE_START_ADDRESS], &mut buffer[5..]).await?;
		let up: i64 = i64::from_be_bytes(buffer) >> (8 - oss);

		let b6 = b5 - 4000;
		let x1 = (b2 * (b6 * b6 / (1 << 12))) / (1 << 11);
		let x2 = ac2 * b6 / (1 << 11);
		let x3 = x1 + x2;
		let b3 = (((ac1 * 4 + x3) << oss) + 2) / 4;
		let x1 = ac3 * b6 / (1 << 13);
		let x2 = (b1 * (b6 * b6 / (1 << 12))) / (1 << 16);
		let x3 = ((x1 + x2) + 2) / 4;
		let b4 = ac4 * (x3 + 32768) as u64 / (1 << 15);
		let b7 = (up as u64 - b3 as u64) * (50000 >> oss);
		let p = if b7 < 0x80000000 {
			(b7 * 2) / b4
		} else {
			(b7 / b4) * 2
		} as i64;
		let x1 = (p / (1 << 8)) * (p / (1 << 8));
		let x1 = (x1 * 3038) / (1 << 16);
		let x2 = (-7357 * p) / (1 << 16);
		let p = p + (x1 + x2 + 3791) / (1 << 4);

		let pressure = Pressure(p);

		info!("Temperature reading: {}", temperature);
		info!("Pressure reading: {}", pressure);

		Ok(BmpReading {
			temperature,
			pressure,
		})
	}
}
