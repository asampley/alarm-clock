use std::path::PathBuf;

use crate::note::MidiNote;

#[derive(Debug)]
pub enum BuzzerMessage {
	Clear,
	Note { on: bool, note: MidiNote },
}

#[derive(Debug)]
pub enum ButtonMessage {
	Press(usize),
	Release(usize),
}

#[derive(Debug)]
pub enum PlayerMessage {
	Play(PathBuf),
	Stop,
}
