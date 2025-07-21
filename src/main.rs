use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};

#[derive(Serialize)]
struct NoteEvent {
    channel: u8,
    tick: u32,
    note: u8,
    active: bool,
}

#[derive(Serialize)]
struct MidiData {
    pulses_per_quarter_note: u16,
    tempo: u32,
    track_tick_length: u32,
    channels: u8,
    note_events: Vec<NoteEvent>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} <input.mid> <output.json>", args[0]);
        return Ok(());
    }

    let input_path = &args[1];
    let output_path = &args[2];

    let midi_file = File::open(input_path)?;
    let midi_reader = BufReader::new(midi_file);
    let midi_bytes = midi_reader.bytes().collect::<Result<Vec<_>, _>>()?;
    let smf = Smf::parse(&midi_bytes)?;

    let ppqn = match smf.header.timing {
        Timing::Metrical(m) => m.as_int(),
        _ => panic!("SMPTE time not supported"),
    };

    let mut tempo = 500_000; // Default tempo in microseconds per quarter note

    'outer: for track in &smf.tracks {
        for event in track {
            if let TrackEventKind::Meta(MetaMessage::Tempo(t)) = event.kind {
                tempo = u32::from(t);
                break 'outer;
            }
        }
    }

    let mut note_events = Vec::new();
    let mut max_tick = 0;
    let mut channels = HashSet::new();

    for track in smf.tracks {
        let mut tick = 0;

        for event in track {
            tick += event.delta.as_int();
            max_tick = max_tick.max(tick);
            if let TrackEventKind::Midi { channel, message } = event.kind {
                channels.insert(channel.as_int());
                match message {
                    MidiMessage::NoteOn { key, vel } => {
                        if vel > 0 {
                            note_events.push(NoteEvent {
                                channel: channel.as_int(),
                                tick,
                                note: key.as_int(),
                                active: true,
                            });
                        } else {
                            note_events.push(NoteEvent {
                                channel: channel.as_int(),
                                tick,
                                note: key.as_int(),
                                active: false,
                            });
                        }
                    }
                    MidiMessage::NoteOff { key, vel: _ } => {
                        note_events.push(NoteEvent {
                            channel: channel.as_int(),
                            tick,
                            note: key.as_int(),
                            active: false,
                        });
                    }
                    _ => {}
                }
            }
        } //-events
    } //-tracks
        note_events.sort_by_key(|e| (e.tick, e.channel, e.note, !e.active));
        let midi_data = MidiData {
            pulses_per_quarter_note: ppqn,
            tempo,
            track_tick_length: max_tick,
            channels: channels.len() as u8,
            note_events,
        };

        let mut output_file = File::create(output_path)?;
        serde_json::to_writer_pretty(&mut output_file, &midi_data)?;
    Ok(())
}