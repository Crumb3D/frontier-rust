//! The soundtrack in `assets/music`. One track loops for each place:
//! Intro on the title, Hall of the Mountain King in dock, the Blue Danube
//! in flight, Bare Mountain on the charts.
//!
//! The device is opened on the first frame, after the window exists. Opening
//! it earlier leaves WASAPI silent for the whole session. The track is decoded
//! up front so the audio thread is only copying samples.

use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;

pub struct Music {
    sink: Option<Sink>,
    stream: Option<(OutputStream, OutputStreamHandle)>,
    current: Option<&'static str>,
    tries: u8,
}

impl Music {
    pub fn new() -> Self {
        Self { sink: None, stream: None, current: None, tries: 0 }
    }

    fn ensure(&mut self) -> bool {
        if self.stream.is_some() {
            return true;
        }
        if self.tries >= 8 {
            return false;
        }
        self.tries += 1;
        match OutputStream::try_default() {
            Ok(pair) => {
                self.stream = Some(pair);
                true
            }
            Err(e) => {
                if self.tries == 1 {
                    eprintln!("music: no audio device ({e})");
                }
                false
            }
        }
    }

    /// Start `key` if it is not already playing.
    pub fn set(&mut self, key: &'static str) {
        if !self.ensure() || self.current == Some(key) {
            return;
        }
        let Some(name) = track(key) else { return };
        let path = frontier_sim::paths::assets_dir().join("music").join(name);
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("music: {} ({e})", path.display());
                self.current = Some(key);
                return;
            }
        };
        let decoded = match Decoder::new(BufReader::new(file)) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("music: could not decode {} ({e})", path.display());
                self.current = Some(key);
                return;
            }
        };
        let channels = decoded.channels();
        let rate = decoded.sample_rate();
        let samples: Vec<f32> = decoded.convert_samples().collect();
        if samples.is_empty() {
            eprintln!("music: {} has no samples", path.display());
            self.current = Some(key);
            return;
        }
        let Some((_, handle)) = &self.stream else { return };
        let Ok(sink) = Sink::try_new(handle) else {
            eprintln!("music: could not open a mixer");
            return;
        };
        sink.set_volume(0.9);
        sink.append(SamplesBuffer::new(channels, rate, samples).repeat_infinite());
        sink.play();
        if let Some(old) = self.sink.take() {
            old.stop();
        }
        self.sink = Some(sink);
        self.current = Some(key);
    }

    pub fn gain(&mut self, volume: f32) {
        if let Some(sink) = &self.sink {
            sink.set_volume(volume);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soundtrack_mp3s_decode() {
        for name in ["01. Intro.mp3", "03. Hall King.mp3", "05. Bare M.mp3", "07. Blue Dan.mp3"] {
            let path = frontier_sim::paths::assets_dir().join("music").join(name);
            let file = File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let dec = Decoder::new(BufReader::new(file)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(dec.take(4096).count() > 1000, "{name} produced no samples");
        }
    }
}

fn track(key: &str) -> Option<&'static str> {
    Some(match key {
        "intro" => "01. Intro.mp3",
        "docked" => "03. Hall King.mp3",
        "bare" => "05. Bare M.mp3",
        "space" => "07. Blue Dan.mp3",
        _ => return None,
    })
}
