use std::convert::TryInto;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError::{Disconnected, Empty};
use std::time;

use crate::message::{BuzzerMessage, PlayerMessage, EventMessage, SongEvent};

use crate::note::MidiNote;

use midly::Smf;
use midly::MidiMessage;
use midly::MetaMessage;
use midly::EventKind;
use midly::Timing;

pub fn midi_player(
	player_receiver: mpsc::Receiver<PlayerMessage>,
	note_sender: mpsc::Sender<BuzzerMessage>,
	event_sender: mpsc::Sender<EventMessage>,
) {
	let mut playing_name = None;

	loop {
		// stopped loop
		while playing_name == None {
			match player_receiver.recv() {
				Ok(message) => match message {
					PlayerMessage::Play(name) => {
						playing_name = Some(name);
					}
					PlayerMessage::Stop => (),
				}
				Err(_) => return,
			}
		}

		// playing
		if let Some(ref some_name) = playing_name {
			event_sender.send(SongEvent::Start(song_name(some_name)).into()).unwrap();

			// load file and set initial variables
			let midi_file = fs::read(some_name).expect("Unable to read midi file");
			let smf = Smf::parse(&midi_file).expect("Unable to parse midi file");
			let tracks = &smf.tracks;

			let mut tempo = 500_000; // microseconds per beat
			let ticks_per_beat =
				if let Timing::Metrical(tpb) = smf.header.timing {
					tpb.as_int()
				} else {
					panic!("Currently only supports metrical time")
				};
			let mut events = Vec::with_capacity(tracks.len());
			let mut next_times = Vec::with_capacity(tracks.len());
			let now = time::Instant::now();

			for track in tracks {
				let mut events_i = track.iter().peekable();
				let next_time_ms = match events_i.peek() {
					Some(ev) => delta_to_micros(
						ticks_per_beat, tempo, ev.delta.as_int()
					),
					None => 0,
				};

				events.push(events_i);

				next_times.push(now + time::Duration::from_micros(next_time_ms));
			}

			loop {
				// break out of loop when there are no more notes
				if !events.iter_mut().any(|ev| ev.peek().is_some()) {
					event_sender.send(SongEvent::End(song_name(some_name)).into()).unwrap();
					playing_name = None;
					break;
				}

				// break if any new messages are received
				match player_receiver.try_recv() {
					Ok(message) => match message {
						PlayerMessage::Play(name) => {
							event_sender.send(SongEvent::End(song_name(some_name)).into()).unwrap();
							playing_name = Some(name);
							break;
						}
						PlayerMessage::Stop => {
							event_sender.send(SongEvent::End(song_name(some_name)).into()).unwrap();
							playing_name = None;
							break;
						}
					}
					Err(e) => match e {
						Empty => (),
						Disconnected => (),
					}
				}

				for ti in 0..events.len() {
					if events[ti].peek().is_none() {
						continue;
					}

					while next_times[ti] <= time::Instant::now() {
						let event = match events[ti].next() {
							Some(event) => event,
							None => break,
						};

						match event.kind {
							EventKind::Midi { channel: _, message } => {
								if let Some((on, note)) = midi_to_buzzer(message) {
									note_sender.send(BuzzerMessage::Note{on, note}).unwrap();
								}
							}
							EventKind::Meta(MetaMessage::Tempo(new_tempo)) => {
								tempo = new_tempo.as_int()
							}
							_ => (),
						};

						if let Some(next_event) = events[ti].peek() {
							next_times[ti] += time::Duration::from_micros(
								delta_to_micros(
									ticks_per_beat, tempo, next_event.delta.as_int()
								)
							);
						};
					}
				}
			}
		}

		note_sender.send(BuzzerMessage::Clear).unwrap();
	}
}

fn delta_to_micros(ticks_per_beat: u16, tempo: u32, delta: u32) -> u64 {
	tempo as u64 * delta as u64 / ticks_per_beat as u64
}

fn midi_to_buzzer(msg: MidiMessage) -> Option<(bool, MidiNote)> {
	match msg {
		MidiMessage::NoteOff { key, .. } => Some((
			false,
			MidiNote(key.as_int().try_into().unwrap()),
		)),
		MidiMessage::NoteOn  { key, vel } => Some((
			vel.as_int() > 0,
			MidiNote(key.as_int().try_into().unwrap()),
		)),
		MidiMessage::Aftertouch { key, vel } => Some((
			vel.as_int() > 0,
			MidiNote(key.as_int().try_into().unwrap()),
		)),
		_ => None,
	}
}

fn song_name(path: &Path) -> String {
	path.file_stem().map_or("".to_owned(), |s| s.to_string_lossy().into_owned())
}
