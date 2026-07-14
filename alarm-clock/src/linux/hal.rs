use linux_embedded_hal::{CdevPin, CdevPinError, I2CError, I2cdev};

pub struct Pin(CdevPin);
pub struct I2c(I2cdev);

impl embedded_hal::digital::ErrorType for Pin {
	type Error = CdevPinError;
}

impl embedded_hal::digital::InputPin for Pin {
	fn is_high(&mut self) -> Result<bool, Self::Error> {
		Ok(self.0.is_high()?)
	}

	fn is_low(&mut self) -> Result<bool, Self::Error> {
		Ok(self.0.is_low()?)
	}
}

impl embedded_hal::digital::OutputPin for Pin {
	fn set_low(&mut self) -> Result<(), Self::Error> {
		Ok(self.0.set_low()?)
	}

	fn set_high(&mut self) -> Result<(), Self::Error> {
		Ok(self.0.set_high()?)
	}
}

impl embedded_hal_async::digital::Wait for Pin {
	async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
		todo!()
	}

	async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
		todo!()
	}

	async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
		todo!()
	}

	async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
		todo!()
	}

	async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
		todo!()
	}
}

impl From<CdevPin> for Pin {
	fn from(value: CdevPin) -> Self {
		Self(value)
	}
}

//impl embedded_hal::digital::Error for PinError {
//	fn kind(&self) -> embedded_hal::digital::ErrorKind {
//		embedded_hal::digital::ErrorKind::Other
//	}
//}

//impl From<CdevPinError> for PinError {
//	fn from(value: CdevPinError) -> Self {
//		Self(value)
//	}
//}

impl embedded_hal::i2c::ErrorType for I2c {
	type Error = I2CError;
}

impl embedded_hal::i2c::I2c for I2c {
	fn transaction(
		&mut self,
		address: u8,
		operations: &mut [embedded_hal::i2c::Operation<'_>],
	) -> Result<(), Self::Error> {
		Ok(self.0.transaction(address, operations)?)
	}
}

impl embedded_hal_async::i2c::I2c for I2c {
	async fn transaction(
		&mut self,
		address: u8,
		operations: &mut [embedded_hal::i2c::Operation<'_>],
	) -> Result<(), Self::Error> {
		todo!()
	}
}

impl From<I2cdev> for I2c {
	fn from(value: I2cdev) -> Self {
		Self(value)
	}
}

//impl embedded_hal::i2c::Error for I2cError {
//	fn kind(&self) -> embedded_hal::i2c::ErrorKind {
//		embedded_hal::i2c::ErrorKind::Other
//	}
//}

//impl From<I2CError> for I2cError {
//	fn from(value: I2CError) -> Self {
//		Self(value)
//	}
//}
