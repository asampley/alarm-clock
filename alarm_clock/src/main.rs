use std::fs;
use std::sync::mpsc;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

mod circuit;
use circuit::Buzzer;
use circuit::Button;
use circuit::Alphanum;

mod config;
use config::Config;

mod message;
use message::{AlphanumMessage, ButtonMessage, PlayerMessage};

mod note;
use note::MidiNote;

mod selector;
use selector::{LinearSelector, Selector};

mod threads;
use threads::input::poll_inputs;
use threads::buzzer::update_buzzer;
use threads::player::midi_player;
use threads::alphanum::alphanum_thread;

use pre_table;

#[cfg(test)] mod tests;

fn main() {
	let mut args = std::env::args();

	// parse arguments
	if 2 != args.len() {
		eprintln!("Usage: {} CONFIG", args.next().unwrap());
		return;
	}

	let args = args.collect::<Vec<_>>();

	let config: Config = toml::from_str(
			&fs::read_to_string(&args[1])
				.expect(&format!("Unable to read config file \"{}\"", &args[1]))
		).expect("Unable to parse config file");

	// create buzzer controller
	let buzzer = Buzzer::<MidiNote>::new(config.buzzer_pin).expect(
		&format!("Unable to access gpio pin {0}.", config.buzzer_pin)
	);

	// create button pollers
	let buttons = config.button_pins.iter()
		.map(|&p| Button::new(p).expect(&format!("Unable to access gpio pin {0}.", p)))
		.collect::<Vec<_>>();

	// create alphanum controller
	let mut alphanum = Alphanum::new().expect("Unable to create alphanum display struct");
	alphanum.set_brightness(config.brightness);

	// create channels for messages
	let (midi_note_sender, midi_note_receiver) = mpsc::channel();
	let (button_sender, button_receiver) = mpsc::channel();
	let (player_sender, player_receiver) = mpsc::channel();
	let (alphanum_sender, alphanum_receiver) = mpsc::channel();

	// start thread to update buzzer
	let _thread_buzzer = thread::spawn(move || update_buzzer(midi_note_receiver, buzzer));

	// start thread to read poll button
	let _thread_buttons = thread::spawn(move || {
		poll_inputs(button_sender, buttons)
	});

	// start playing midi file
	let _thread_midi_player = thread::spawn(move || midi_player(player_receiver, midi_note_sender));

	// start display thread
	let _thread_display = thread::spawn(move || alphanum_thread(alphanum, alphanum_receiver, Duration::from_millis(500)));

	let mut midi_files = list_files(&config.midi_dir)
		.expect(&format!("Unable to read the directory \"{:?}\"", config.midi_dir))
		.collect::<Vec<PathBuf>>();
	midi_files.sort();

	let mut selector = LinearSelector::new(midi_files);

	loop {
		match button_receiver.recv().unwrap() {
			ButtonMessage::Press(x) => {
				if x == 0 || x == 1 {
					let midi_file = if x == 0 {
						selector.incr()
					} else {
						selector.decr()
					};

					player_sender.send(PlayerMessage::Play(midi_file.clone()))
						.expect("Unable to send midi file name");

					alphanum_sender.send(
						AlphanumMessage::Text(
							midi_file.as_path()
								.file_stem()
								.map(|s| s.to_string_lossy().into_owned())
								.unwrap_or("".to_owned())
						)
					);
				}
			}
			ButtonMessage::Release(_) => (),
		}
	}

	//println!("{} updates per second", thread_buzzer.join().unwrap()?);
	//thread_midi_player.join().unwrap();
	//thread_buttons.join().unwrap()?;

	//Ok(())
}

fn list_files<P: AsRef<Path>>(dir: P) -> std::io::Result<impl Iterator<Item = PathBuf>> {
	Ok(fs::read_dir(dir)?
		.filter(|f| f.is_ok())
		.map(|f| f.unwrap().path())
		.peekable()
	)
}
