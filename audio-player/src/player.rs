use std::{
    error::Error,
    io::Seek,
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
    controller: AudioPlayerController,
    current: Option<DecodedTrack>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let output = AudioOutputter::new()?;
        let state = Arc::new(AudioPlayerControllerState::new());
        let controller = AudioPlayerController::new(state);
        Ok(Self {
            output,
            controller,
            current: None,
        })
    }

    pub fn controller(&mut self) -> &mut AudioPlayerController {
        &mut self.controller
    }

    pub fn open<F: AsRef<Path>>(&mut self, file: F) -> Result<(), Box<dyn Error>> {
        self.current = Some(decoder::decode(&file)?);
        let mut resampler = None;
        loop {
            // TODO: use one mutex
            if let Some(seek_position) = self.controller.state.seek_position()? {
                self.seek(seek_position)?;
                self.controller.state.reset_seek_position()?;
            }
            self.controller.state.wait_for_playing();

            let track = self.current.as_mut().ok_or("TODO")?;
            if let Ok(buffer) = track.next() {
                if resampler.is_none() && buffer.spec().rate != *self.output.sample_rate() {
                    resampler = Some(SymphoniaResampler::new(*self.output.sample_rate(), &buffer));
                }
                let buffer = match resampler {
                    Some(ref mut resampler) => resampler.resample(buffer),
                    None => buffer,
                };

                self.output.write(buffer);
            } else {
                break;
            }
        }
        Ok(())
    }

    fn seek(&mut self, progress: Duration) -> Result<(), Box<dyn Error>> {
        // TODO: skip packets
        match self.current {
            Some(ref mut track) => track.seek(progress)?,
            None => todo!(),
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct AudioPlayerController {
    state: Arc<AudioPlayerControllerState>,
}

impl AudioPlayerController {
    fn new(state: Arc<AudioPlayerControllerState>) -> Self {
        Self { state }
    }

    pub fn play(&mut self) -> Result<(), Box<dyn Error>> {
        let mut playing = self.state.playing.0.lock().unwrap();
        *playing = true;
        self.state.playing.1.notify_all();
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        let mut playing = self.state.playing.0.lock().unwrap();
        *playing = false;
        self.state.playing.1.notify_all();
        Ok(())
    }

    pub fn seek(&mut self, progress: Duration) -> Result<(), Box<dyn Error>> {
        let mut seeking = self.state.seek_position.lock().unwrap();
        *seeking = Some(progress);
        Ok(())
    }
}

struct AudioPlayerControllerState {
    playing: (Mutex<bool>, Condvar),
    seek_position: Mutex<Option<Duration>>,
}

impl AudioPlayerControllerState {
    fn new() -> Self {
        let playing = (Mutex::new(false), Condvar::new());
        let seek_position = Mutex::new(None);
        Self {
            playing,
            seek_position,
        }
    }

    fn wait_for_playing(&self) {
        let mut playing = self.playing.0.lock().unwrap();
        while !*playing {
            playing = self.playing.1.wait(playing).unwrap();
        }
    }

    /// Returns None if not seeking
    fn seek_position(&self) -> Result<Option<Duration>, Box<dyn Error>> {
        let seek = self.seek_position.lock().unwrap();
        Ok(seek.clone())
    }

    fn reset_seek_position(&self) -> Result<(), Box<dyn Error>> {
        let mut seek = self.seek_position.lock().unwrap();
        *seek = None;
        Ok(())
    }
}
