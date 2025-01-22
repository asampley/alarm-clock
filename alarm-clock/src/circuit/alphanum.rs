use embedded_hal_async::i2c::I2c;

#[allow(dead_code)]
const HT16K33_BLINK_CMD: u8 = 0x80; //< I2C register for BLINK setting
#[allow(dead_code)]
const HT16K33_BLINK_DISPLAYON: u8 = 0x01; //< I2C value for steady on
#[allow(dead_code)]
const HT16K33_BLINK_OFF: u8 = 0; //< I2C value for steady off
#[allow(dead_code)]
const HT16K33_BLINK_2HZ: u8 = 1; //< I2C value for 2 Hz blink
#[allow(dead_code)]
const HT16K33_BLINK_1HZ: u8 = 2; //< I2C value for 1 Hz blink
#[allow(dead_code)]
const HT16K33_BLINK_HALFHZ: u8 = 3; //< I2C value for 0.5 Hz blink

#[allow(dead_code)]
const HT16K33_CMD_BRIGHTNESS: u8 = 0xE0; //< I2C register for BRIGHTNESS setting

pub struct Alphanum<Pin> {
	i2c: Pin,
	ascii_uppercase: bool,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum BlinkRate {
	Off,
	HalfHz,
	OneHz,
	TwoHz,
}

impl<Pin: embedded_hal::i2c::I2c> Alphanum<Pin> {
	pub fn new(pin: Pin) -> Result<Self, Pin::Error> {
		let mut val = Self {
			i2c: pin,
			ascii_uppercase: false,
		};

		// turn on oscillator
		val.write_sync(&[0x21])?;

		val.set_brightness_sync(15)?;
		val.blink_rate_sync(BlinkRate::Off)?;

		Ok(val)
	}

	fn write_sync(&mut self, write: &[u8]) -> Result<(), Pin::Error> {
		self.i2c.write(0x70, write)
	}

	pub fn set_brightness_sync(&mut self, brightness: u8) -> Result<(), Pin::Error> {
		self.write_sync(&[HT16K33_CMD_BRIGHTNESS | core::cmp::min(brightness, 15)])
	}

	pub fn blink_rate_sync(&mut self, blink_rate: BlinkRate) -> Result<(), Pin::Error> {
		let blink_rate = match blink_rate {
			BlinkRate::Off => HT16K33_BLINK_OFF,
			BlinkRate::TwoHz => HT16K33_BLINK_2HZ,
			BlinkRate::OneHz => HT16K33_BLINK_1HZ,
			BlinkRate::HalfHz => HT16K33_BLINK_HALFHZ,
		};

		self.write_sync(&[HT16K33_BLINK_CMD | HT16K33_BLINK_DISPLAYON | (blink_rate << 1)])
	}
}

impl<Pin: I2c> Alphanum<Pin> {
	async fn write(&mut self, write: &[u8]) -> Result<(), Pin::Error> {
		self.i2c.write(0x70, write).await
	}

	pub async fn set_brightness(&mut self, brightness: u8) -> Result<(), Pin::Error> {
		self.write(&[HT16K33_CMD_BRIGHTNESS | core::cmp::min(brightness, 15)])
			.await
	}

	pub async fn blink_rate(&mut self, blink_rate: BlinkRate) -> Result<(), Pin::Error> {
		let blink_rate = match blink_rate {
			BlinkRate::Off => HT16K33_BLINK_OFF,
			BlinkRate::TwoHz => HT16K33_BLINK_2HZ,
			BlinkRate::OneHz => HT16K33_BLINK_1HZ,
			BlinkRate::HalfHz => HT16K33_BLINK_HALFHZ,
		};

		self.write(&[HT16K33_BLINK_CMD | HT16K33_BLINK_DISPLAYON | (blink_rate << 1)])
			.await
	}

	pub fn ascii_uppercase(&mut self, ascii_uppercase: bool) {
		self.ascii_uppercase = ascii_uppercase;
	}

	/// display up to 4 chars from `string`
	pub async fn display(&mut self, string: &str) -> Result<(), Pin::Error> {
		let mut bytes = [0_u8; 9];

		for (i, mut c) in string.chars().enumerate().take(4) {
			if self.ascii_uppercase {
				c = c.to_ascii_uppercase()
			}

			let char_bytes = char_to_alphanum(c).to_le_bytes();
			bytes[i * 2 + 1] = char_bytes[0];
			bytes[i * 2 + 2] = char_bytes[1];
		}

		self.write(&bytes).await
	}
}

pub fn timer_with_hand_alphanum(hand: u8) -> u16 {
	const CLOCK: u16 = char_to_alphanum('O');

	CLOCK | match hand {
		0 => 0b_0000_0010_0000_0000,
		1 => 0b_0000_0100_0000_0000,
		2 => 0b_0000_0000_1000_0000,
		3 => 0b_0010_0000_0000_0000, 
		4 => 0b_0001_0000_0000_0000,
		5 => 0b_0000_1000_0000_0000,
		6 => 0b_0000_0000_0100_0000,
		7 => 0b_0000_0001_0000_0000,
		_ => 0,
	}
}

const fn char_to_alphanum(c: char) -> u16 {
	match c {
		' ' => 0b_0000_0000_0000_0000,
		'!' => 0b_0000_0000_0000_0110,
		'"' => 0b_0000_0010_0010_0000,
		'#' => 0b_0001_0010_1100_1110,
		'$' => 0b_0001_0010_1110_1101,
		'%' => 0b_0000_1100_0010_0100,
		'&' => 0b_0010_0011_0101_1101,
		'\'' => 0b_0000_0100_0000_0000,
		'(' => 0b_0010_0100_0000_0000,
		')' => 0b_0000_1001_0000_0000,
		'*' => 0b_0011_1111_1100_0000,
		'+' => 0b_0001_0010_1100_0000,
		',' => 0b_0000_1000_0000_0000,
		'-' => 0b_0000_0000_1100_0000,
		'.' => 0b_0000_0000_0000_0000,
		'/' => 0b_0000_1100_0000_0000,
		'0' => 0b_0000_1100_0011_1111,
		'1' => 0b_0000_0000_0000_0110,
		'2' => 0b_0000_0000_1101_1011,
		'3' => 0b_0000_0000_1000_1111,
		'4' => 0b_0000_0000_1110_0110,
		'5' => 0b_0010_0000_0110_1001,
		'6' => 0b_0000_0000_1111_1101,
		'7' => 0b_0000_0000_0000_0111,
		'8' => 0b_0000_0000_1111_1111,
		'9' => 0b_0000_0000_1110_1111,
		':' => 0b_0001_0010_0000_0000,
		';' => 0b_0000_1010_0000_0000,
		'<' => 0b_0010_0100_0000_0000,
		'=' => 0b_0000_0000_1100_1000,
		'>' => 0b_0000_1001_0000_0000,
		'?' => 0b_0001_0000_1000_0011,
		'@' => 0b_0000_0010_1011_1011,
		'A' => 0b_0000_0000_1111_0111,
		'B' => 0b_0001_0010_1000_1111,
		'C' => 0b_0000_0000_0011_1001,
		'D' => 0b_0001_0010_0000_1111,
		'E' => 0b_0000_0000_1111_1001,
		'F' => 0b_0000_0000_0111_0001,
		'G' => 0b_0000_0000_1011_1101,
		'H' => 0b_0000_0000_1111_0110,
		'I' => 0b_0001_0010_0000_0000,
		'J' => 0b_0000_0000_0001_1110,
		'K' => 0b_0010_0100_0111_0000,
		'L' => 0b_0000_0000_0011_1000,
		'M' => 0b_0000_0101_0011_0110,
		'N' => 0b_0010_0001_0011_0110,
		'O' => 0b_0000_0000_0011_1111,
		'P' => 0b_0000_0000_1111_0011,
		'Q' => 0b_0010_0000_0011_1111,
		'R' => 0b_0010_0000_1111_0011,
		'S' => 0b_0000_0000_1110_1101,
		'T' => 0b_0001_0010_0000_0001,
		'U' => 0b_0000_0000_0011_1110,
		'V' => 0b_0000_1100_0011_0000,
		'W' => 0b_0010_1000_0011_0110,
		'X' => 0b_0010_1101_0000_0000,
		'Y' => 0b_0001_0000_1110_0010,
		'Z' => 0b_0000_1100_0000_1001,
		'[' => 0b_0000_0000_0011_1001,
		'\\' => 0b_0010_0001_0000_0000,
		']' => 0b_0000_0000_0000_1111,
		'^' => 0b_0000_1100_0000_0011,
		'_' => 0b_0000_0000_0000_1000,
		'`' => 0b_0000_0001_0000_0000,
		'a' => 0b_0001_0000_0101_1000,
		'b' => 0b_0010_0000_0111_1000,
		'c' => 0b_0000_0000_1101_1000,
		'd' => 0b_0000_1000_1000_1110,
		'e' => 0b_0000_1000_0101_1000,
		'f' => 0b_0000_0000_0111_0001,
		'g' => 0b_0000_0100_1000_1110,
		'h' => 0b_0001_0000_0111_0000,
		'i' => 0b_0001_0000_0000_0000,
		'j' => 0b_0000_0000_0000_1110,
		'k' => 0b_0011_0110_0000_0000,
		'l' => 0b_0000_0000_0011_0000,
		'm' => 0b_0001_0000_1101_0100,
		'n' => 0b_0001_0000_0101_0000,
		'o' => 0b_0000_0000_1101_1100,
		'p' => 0b_0000_0001_0111_0000,
		'q' => 0b_0000_0100_1000_0110,
		'r' => 0b_0000_0000_0101_0000,
		's' => 0b_0010_0000_1000_1000,
		't' => 0b_0000_0000_0111_1000,
		'u' => 0b_0000_0000_0001_1100,
		'v' => 0b_0010_0000_0000_0100,
		'w' => 0b_0010_1000_0001_0100,
		'x' => 0b_0010_1000_1100_0000,
		'y' => 0b_0010_0000_0000_1100,
		'z' => 0b_0000_1000_0100_1000,
		'{' => 0b_0000_1001_0100_1001,
		'|' => 0b_0001_0010_0000_0000,
		'}' => 0b_0010_0100_1000_1001,
		'~' => 0b_0000_0101_0010_0000,
		_ => 0b_0011_1111_1111_1111,
	}
}
