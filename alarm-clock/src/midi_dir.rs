use dir_static::File;

pub type Midi = File;

#[dir_static::dir_array("../midi/")]
pub static MIDI_DIR: [File];
