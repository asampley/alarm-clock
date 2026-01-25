#![no_std]

use defmt::Format;
pub use dir_static_macro::*;

#[derive(Copy, Clone, Debug, Format)]
pub struct File {
	pub name: &'static str,
	pub data: &'static [u8],
}
