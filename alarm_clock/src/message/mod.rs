use std::path::PathBuf;

use crate::note::MidiNote;

#[derive(Debug)]
pub enum BuzzerMessage {
	Clear,
	Note { on: bool, note: MidiNote },
}

#[derive(Debug)]
pub enum ButtonMessage {
	Press(u8),
	Release(u8),
}

#[derive(Debug)]
pub enum PlayerMessage {
	Play(PathBuf),
	Stop,
}
