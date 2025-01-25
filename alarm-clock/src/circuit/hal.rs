#[cfg(target_arch = "xtensa")]
pub use xtensa::*;

#[cfg(target_arch = "xtensa")]
mod xtensa {
	use embassy_time::Duration;

	use embedded_hal::i2c::ErrorType;

	use esp_hal::gpio::{AnyPin, Input, Level, Output, Pull};
	use esp_hal::i2c::master::{AnyI2c, I2c};
	use esp_hal::Async;

	pub type Alphanum = crate::circuit::alphanum::Alphanum<I2c<'static, Async>>;
	pub type Button = crate::circuit::button::Button<Input<'static>>;
	pub type Buzzer = crate::circuit::buzzer::Buzzer<Output<'static>>;

	impl From<AnyPin> for Buzzer {
		fn from(pin: AnyPin) -> Self {
			Self::new(Output::new(pin, Level::Low))
		}
	}

	impl<'a> From<(AnyPin, Duration)> for Button {
		fn from((pin, bounce_time): (AnyPin, Duration)) -> Self {
			Self::new(Input::new(pin, Pull::Up), bounce_time)
		}
	}

	impl<'a> Alphanum {
		pub fn new_esp_hal(
			pin: AnyI2c,
			sda: AnyPin,
			scl: AnyPin,
		) -> Result<Self, <esp_hal::i2c::master::I2c<'a, esp_hal::Async> as ErrorType>::Error> {
			Self::new(
				esp_hal::i2c::master::I2c::new(pin, Default::default())
					.unwrap()
					.with_sda(sda)
					.with_scl(scl)
					.into_async(),
			)
		}
	}
}
