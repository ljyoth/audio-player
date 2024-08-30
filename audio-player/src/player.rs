use std::{
    error::Error,
    path::Path,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use crate::{
    decoder::{self, DecodedTrack},
    output::{AudioOutputWriter, AudioOutputter},
    resampler::SymphoniaResampler,
};

pub struct AudioPlayer {
    output: Box<dyn AudioOutputWriter>,
    playing: Arc<(Mutex<bool>, Condvar)>,
    current: Option<DecodedTrack>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let output = AudioOutputter::new()?;

        let playing = Arc::new((Mutex::new(false), Condvar::new()));

        Ok(Self {
            output,
            playing,
            current: None,
        })
    }

    pub fn open<F: AsRef<Path>>(&mut self, file: F) -> Result<(), Box<dyn Error>> {
        self.current = Some(decoder::decode(&file)?);
        let track = self.current.as_mut().ok_or("TODO")?;
        let mut resampler = None;
        loop {
            let mut playing = self.playing.0.lock().unwrap();
            while !*playing {
                println!("{playing}");
                playing = self.playing.1.wait(playing).unwrap();
            }
            if let Ok(buffer) = track.next() {
                if resampler.is_none() && buffer.spec().rate != *self.output.sample_rate() {
                    resampler = Some(SymphoniaResampler::new(*self.output.sample_rate(), &buffer));
                }
                if let Some(ref mut resampler) = resampler {
                    let buffer = resampler.resample(buffer);
                    self.output.write(buffer);
                } else {
                    println!("writing...");
                    self.output.write(buffer);
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    pub fn play(&mut self) -> Result<(), Box<dyn Error>> {
        let mut playing = self.playing.0.lock().unwrap();
        *playing = true;
        self.playing.1.notify_all();
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        let mut playing = self.playing.0.lock().unwrap();
        *playing = false;
        self.playing.1.notify_all();
        Ok(())
    }

    pub fn seek(&mut self, progress: Duration) -> Result<(), Box<dyn Error>> {
        match self.current {
            Some(ref mut track) => track.seek(progress)?,
            None => todo!(),
        }
        Ok(())
    }
}
