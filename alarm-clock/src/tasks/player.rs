use crate::{error, MIDI_NOTE_CAPACITY};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Instant};
use heapless::Vec;

use crate::message::{BuzzerMessage, EventMessage, PlayerMessage, SongEvent};

use crate::note::MidiNote;

use embassy_sync::channel::{Receiver, Sender, TryReceiveError};
use embassy_time::Timer;

use midly::MetaMessage;
use midly::MidiMessage;
use midly::Timing;
use midly::TrackEventKind;

const TRACK_CAPCITY: usize = 64;

#[embassy_executor::task]
pub async fn midi_player(
	player_receiver: Receiver<'static, CriticalSectionRawMutex, PlayerMessage, 1>,
	note_sender: Sender<'static, CriticalSectionRawMutex, BuzzerMessage, MIDI_NOTE_CAPACITY>,
	event_sender: Sender<'static, CriticalSectionRawMutex, EventMessage, 1>,
) {
	let mut playing = None;
	let mut looping = false;

	loop {
		// stopped loop
		while playing.is_none() {
			match player_receiver.receive().await {
				PlayerMessage::Loop(midi) => {
					playing = Some(midi);
					looping = true;
				}
				PlayerMessage::Play(midi) => {
					playing = Some(midi);
					looping = false;
				}
				PlayerMessage::Stop => (),
			}
		}

		// playing
		if let Some(ref now_playing) = playing {
			event_sender
				.send(SongEvent::Start(now_playing.name).into())
				.await;

			// load file and set initial variables
			let (header, tracks) =
				midly::parse(&now_playing.data).expect("Unable to parse midi file");

			let mut tempo = 500_000; // microseconds per beat
			let ticks_per_beat = if let Timing::Metrical(tpb) = header.timing {
				tpb.as_int()
			} else {
				panic!("Currently only supports metrical time")
			};
			let mut events = Vec::<_, TRACK_CAPCITY>::new();
			let mut next_times = Vec::<_, TRACK_CAPCITY>::new();
			let now = Instant::now();

			for track in tracks {
				match track {
					Ok(event_iter) => {
						let mut events_i = event_iter.peekable();
						let next_time_ms = match events_i.peek() {
							Some(Ok(ev)) => {
								delta_to_micros(ticks_per_beat, tempo, ev.delta.as_int())
							}
							_ => 0,
						};

						if let Err(_) = events.push(events_i) {
							error!("Midi events buffer full, skipping");
							break;
						}

						if let Err(_) = next_times.push(now + Duration::from_micros(next_time_ms)) {
							error!("Midi times buffer full, skipping");
							break;
						}
					}
					Err(_) => (),
				}
			}

			loop {
				// break out of loop when there are no more notes
				if !events.iter_mut().any(|ev| ev.peek().is_some()) {
					event_sender
						.send(SongEvent::End(now_playing.name).into())
						.await;
					if !looping {
						playing = None;
					}
					break;
				}

				// break if any new messages are received
				match player_receiver.try_receive() {
					Ok(message) => match message {
						PlayerMessage::Loop(midi) => {
							event_sender
								.send(SongEvent::End(now_playing.name).into())
								.await;
							playing = Some(midi);
							looping = true;
							break;
						}
						PlayerMessage::Play(midi) => {
							event_sender
								.send(SongEvent::End(now_playing.name).into())
								.await;
							playing = Some(midi);
							looping = false;
							break;
						}
						PlayerMessage::Stop => {
							event_sender
								.send(SongEvent::End(now_playing.name).into())
								.await;
							playing = None;
							break;
						}
					},
					Err(e) => match e {
						TryReceiveError::Empty => (),
					},
				}

				for ti in 0..events.len() {
					if events[ti].peek().is_none() {
						continue;
					}

					while next_times[ti] <= Instant::now() {
						let event = match events[ti].next() {
							Some(Ok(event)) => event,
							_ => break,
						};

						match event.kind {
							TrackEventKind::Midi {
								channel: _,
								message,
							} => {
								if let Some((on, note)) = midi_to_buzzer(message) {
									note_sender.send(BuzzerMessage::Note { on, note }).await;
								}
							}
							TrackEventKind::Meta(MetaMessage::Tempo(new_tempo)) => {
								tempo = new_tempo.as_int()
							}
							_ => (),
						};

						if let Some(Ok(next_event)) = events[ti].peek() {
							next_times[ti] += Duration::from_micros(delta_to_micros(
								ticks_per_beat,
								tempo,
								next_event.delta.as_int(),
							));
						};
					}

					Timer::at(*next_times.iter().min().unwrap()).await
				}
			}
		}

		note_sender.send(BuzzerMessage::Clear).await;
	}
}

fn delta_to_micros(ticks_per_beat: u16, tempo: u32, delta: u32) -> u64 {
	tempo as u64 * delta as u64 / ticks_per_beat as u64
}

fn midi_to_buzzer(msg: MidiMessage) -> Option<(bool, MidiNote)> {
	match msg {
		MidiMessage::NoteOff { key, .. } => {
			Some((false, MidiNote(key.as_int().try_into().unwrap())))
		}
		MidiMessage::NoteOn { key, vel } => {
			Some((vel.as_int() > 0, MidiNote(key.as_int().try_into().unwrap())))
		}
		MidiMessage::Aftertouch { key, vel } => {
			Some((vel.as_int() > 0, MidiNote(key.as_int().try_into().unwrap())))
		}
		_ => None,
	}
}
