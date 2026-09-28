use async_channel::{Receiver, Sender};
use embedded_hal::{digital::PinState, i2c::I2c as _};
use futures_lite::future::block_on;
use gpiocdev_embedded_hal::{OutputPin, async_io::InputPin};
use linux_embedded_hal::I2cdev;
use log::error;
use thiserror::Error;

pub enum FlexPin {
	Input(InputPin),
	Output(OutputPin),
	Poison,
}

pub struct I2c {
	send_transaction: Sender<(u8, Box<[OwnedOperation]>)>,
	receive_transaction_response:
		Receiver<Result<Box<[OwnedOperation]>, linux_embedded_hal::I2CError>>,
}

pub enum OwnedOperation {
	Read(Box<[u8]>),
	Write(Box<[u8]>),
}

#[derive(Debug, Error)]
pub enum Error {
	#[error("poisoned flex pin")]
	Poisoned,
	#[error("gpio error")]
	Gpio(#[from] gpiocdev_embedded_hal::Error),
}

#[derive(Debug, Error)]
pub enum I2cError {
	#[error("internal channel error")]
	SendRequest(#[source] async_channel::SendError<(u8, Box<[OwnedOperation]>)>),
	#[error("internal channel error")]
	RecvResponse(#[source] async_channel::RecvError),
	// Errors currently can't be observed because they're in a thread that's not joined
	//#[error("internal channel error")]
	//SendResponse(#[source] async_channel::SendError<Result<(), linux_embedded_hal::I2CError>>),
	//#[error("internal channel error")]
	//RecvRequest(#[source] async_channel::RecvError),
	#[error("i2c error")]
	I2c(#[from] linux_embedded_hal::I2CError),
}

impl embedded_hal::digital::Error for Error {
	fn kind(&self) -> embedded_hal::digital::ErrorKind {
		embedded_hal::digital::ErrorKind::Other
	}
}

impl FlexPin {
	fn into_input_pin(self) -> Result<Self, Error> {
		Ok(match self {
			Self::Output(o) => Self::Input(o.into_input_pin()?.into()),
			x => x,
		})
	}

	fn mut_input_pin(&mut self) -> Result<&mut InputPin, Error> {
		if !matches!(self, Self::Input(_)) {
			let mut temp = FlexPin::Poison;
			core::mem::swap(self, &mut temp);
			core::mem::swap(self, &mut temp.into_input_pin()?);
		}

		if let Self::Input(i) = self {
			Ok(i)
		} else {
			Err(Error::Poisoned)
		}
	}

	fn into_output_pin(self) -> Result<Self, Error> {
		Ok(match self {
			Self::Input(i) => Self::Output(i.into_output_pin(PinState::Low)?),
			x => x,
		})
	}

	fn mut_output_pin(&mut self) -> Result<&mut OutputPin, Error> {
		if !matches!(self, Self::Input(_)) {
			let mut temp = FlexPin::Poison;
			core::mem::swap(self, &mut temp);
			core::mem::swap(self, &mut temp.into_output_pin()?);
		}

		if let Self::Output(o) = self {
			Ok(o)
		} else {
			Err(Error::Poisoned)
		}
	}
}

impl embedded_hal::digital::ErrorType for FlexPin {
	type Error = Error;
}

impl embedded_hal::digital::InputPin for FlexPin {
	fn is_high(&mut self) -> Result<bool, Self::Error> {
		Ok(self.mut_input_pin()?.is_high()?)
	}

	fn is_low(&mut self) -> Result<bool, Self::Error> {
		Ok(self.mut_input_pin()?.is_low()?)
	}
}

impl embedded_hal::digital::OutputPin for FlexPin {
	fn set_low(&mut self) -> Result<(), Self::Error> {
		Ok(self.mut_output_pin()?.set_low()?)
	}

	fn set_high(&mut self) -> Result<(), Self::Error> {
		Ok(self.mut_output_pin()?.set_high()?)
	}
}

impl embedded_hal_async::digital::Wait for FlexPin {
	async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
		Ok(self.mut_input_pin()?.wait_for_high().await?)
	}

	async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
		Ok(self.mut_input_pin()?.wait_for_low().await?)
	}

	async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
		Ok(self.mut_input_pin()?.wait_for_rising_edge().await?)
	}

	async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
		Ok(self.mut_input_pin()?.wait_for_falling_edge().await?)
	}

	async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
		Ok(self.mut_input_pin()?.wait_for_any_edge().await?)
	}
}

impl From<InputPin> for FlexPin {
	fn from(value: InputPin) -> Self {
		Self::Input(value)
	}
}

impl From<OutputPin> for FlexPin {
	fn from(value: OutputPin) -> Self {
		Self::Output(value)
	}
}

impl embedded_hal::i2c::Error for I2cError {
	fn kind(&self) -> embedded_hal::i2c::ErrorKind {
		match self {
			Self::I2c(e) => e.kind(),
			_ => embedded_hal::i2c::ErrorKind::Other,
		}
	}
}

impl embedded_hal::i2c::ErrorType for I2c {
	type Error = I2cError;
}

impl embedded_hal::i2c::I2c for I2c {
	fn transaction(
		&mut self,
		address: u8,
		operations: &mut [embedded_hal::i2c::Operation<'_>],
	) -> Result<(), Self::Error> {
		block_on(async move {
			embedded_hal_async::i2c::I2c::transaction(self, address, operations).await
		})
	}
}

impl embedded_hal_async::i2c::I2c for I2c {
	async fn transaction(
		&mut self,
		address: u8,
		operations: &mut [embedded_hal::i2c::Operation<'_>],
	) -> Result<(), Self::Error> {
		let owned_operations: Box<[_]> = operations
			.iter()
			.map(|op| match op {
				embedded_hal::i2c::Operation::Read(buf) => {
					OwnedOperation::Read(buf.to_vec().into_boxed_slice())
				}
				embedded_hal::i2c::Operation::Write(buf) => {
					OwnedOperation::Write(buf.to_vec().into_boxed_slice())
				}
			})
			.collect();

		self.send_transaction
			.send((address, owned_operations))
			.await
			.map_err(I2cError::SendRequest)?;
		let response = self
			.receive_transaction_response
			.recv()
			.await
			.map_err(I2cError::RecvResponse)??;
		for (i, op) in response.iter().enumerate() {
			match op {
				OwnedOperation::Read(buf) => match &mut operations[i] {
					embedded_hal::i2c::Operation::Read(real_buf) => real_buf.copy_from_slice(buf),
					_ => unreachable!(),
				},
				_ => (),
			}
		}
		Ok(())
	}
}

impl From<I2cdev> for I2c {
	fn from(mut value: I2cdev) -> Self {
		let (requests_sender, requests_receiver) = async_channel::unbounded();
		let (responses_sender, responses_receiver) = async_channel::unbounded();

		std::thread::spawn(move || {
			let send = responses_sender;
			let receive = requests_receiver;

			block_on(async {
				loop {
					let mut request: (u8, Box<[OwnedOperation]>) = receive.recv().await.unwrap();

					let mut operations: Vec<_> = request
						.1
						.iter_mut()
						.map(|op| match op {
							OwnedOperation::Read(buf) => {
								embedded_hal::i2c::Operation::Read(&mut *buf)
							}
							OwnedOperation::Write(buf) => {
								embedded_hal::i2c::Operation::Write(&mut *buf)
							}
						})
						.collect();

					let _ = send
						.send(
							value
								.transaction(request.0, &mut operations)
								.map(|()| request.1),
						)
						.await
						.inspect_err(|e| error!("{}", e));
				}
			});
		});

		Self {
			send_transaction: requests_sender,
			receive_transaction_response: responses_receiver,
		}
	}
}
