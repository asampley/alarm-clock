#[cfg(target_arch = "xtensa")]
pub use xtensa::*;

#[cfg(target_arch = "xtensa")]
mod xtensa {
	use embassy_time::Duration;

	use embedded_hal::i2c::ErrorType;

	use esp_hal::gpio::{AnyPin, DriveMode, Flex, Input, InputConfig, Level, Output, OutputConfig, Pull};
	use esp_hal::i2c::master::{AnyI2c, I2c};
	use esp_hal::Async;

	pub type Alphanum = crate::circuit::alphanum::Alphanum<I2c<'static, Async>>;
	pub type Button = crate::circuit::button::Button<Input<'static>>;
	pub type Buzzer = crate::circuit::buzzer::Buzzer<Output<'static>>;
	pub type Dht11 = crate::circuit::dht::Dht11<Flex<'static>>;

	impl From<AnyPin<'static>> for Dht11 {
		fn from(pin: AnyPin<'static>) -> Self {
			let mut flex = Flex::new(pin);
			flex.apply_output_config(
				&OutputConfig::default()
					.with_drive_mode(DriveMode::OpenDrain)
					.with_pull(Pull::Up)
			);

			Self::new(flex)
		}
	}

	impl From<AnyPin<'static>> for Buzzer {
		fn from(pin: AnyPin<'static>) -> Self {
			Self::new(Output::new(pin, Level::Low, OutputConfig::default()))
		}
	}

	impl From<(AnyPin<'static>, Duration)> for Button {
		fn from((pin, bounce_time): (AnyPin<'static>, Duration)) -> Self {
			Self::new(Input::new(pin, InputConfig::default().with_pull(Pull::Up)), bounce_time)
		}
	}

	impl Alphanum {
		pub fn new_esp_hal(
			pin: AnyI2c<'static>,
			sda: AnyPin<'static>,
			scl: AnyPin<'static>,
		) -> Result<Self, <esp_hal::i2c::master::I2c<'static, esp_hal::Async> as ErrorType>::Error>
		{
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
