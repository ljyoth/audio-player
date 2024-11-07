use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Sender},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    decoder::{self, DecodedTrack},
    output::AudioOutputter,
    resampler::SymphoniaResampler,
    track::{self, Track},
};

pub struct AudioPlayer {
    controller: AudioPlayerController,
    executor: AudioPlayerExecutor,
}

impl AudioPlayer {
    pub fn new() -> Self {
        let controller = AudioPlayerController::new();
        let executor = AudioPlayerExecutor::new(controller.clone());
        Self {
            controller,
            executor,
        }
    }

    pub fn controller(&self) -> &AudioPlayerController {
        &self.controller
    }

    pub fn open<F: AsRef<Path>>(&mut self, file: F) -> Result<Track, Box<dyn Error>> {
        let track = decoder::decode(&file)?;
        Ok(track)
    }

    // Place track on queue
    pub fn queue(&self, track: Track) -> Result<(), Box<dyn Error>> {
        self.executor.queue(track.decoded)?;
        Ok(())
    }

    // drain all tracks in the queue
    pub fn drain(&self) -> Result<(), Box<dyn Error>> {
        todo!();
    }

    pub fn wait_until_end(self) -> Result<(), Box<dyn Error>> {
        self.executor.wait_until_end()?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct AudioPlayerController {
    state: Arc<Mutex<AudioPlayerControllerState>>,
    playing_condvar: Arc<Condvar>,
    seeking_condvar: Arc<Condvar>,
}

impl AudioPlayerController {
    fn new() -> Self {
        let state = Arc::new(Mutex::new(AudioPlayerControllerState::new()));
        let executor_condvar = Arc::new(Condvar::new());
        let controller_condvar = Arc::new(Condvar::new());
        Self {
            state,
            playing_condvar: executor_condvar,
            seeking_condvar: controller_condvar,
        }
    }

    pub fn play(&self) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().unwrap();
        (*state).playing = true;
        self.playing_condvar.notify_all();
        Ok(())
    }

    pub fn pause(&self) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().unwrap();
        (*state).playing = false;
        self.playing_condvar.notify_all();
        Ok(())
    }

    pub fn playing(&self) -> Result<bool, Box<dyn Error>> {
        let state = self.state.lock().unwrap();
        Ok((*state).playing)
    }

    pub fn position(&self) -> Result<Duration, Box<dyn Error>> {
        let state = self.state.lock().unwrap();
        Ok(state.position.ok_or("unavailable")?)
    }

    pub fn seek(&self, progress: Duration) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().unwrap();
        (*state).seek_position = Some(progress);
        while (*state).seek_position.is_some() {
            state = self.seeking_condvar.wait(state).unwrap();
        }
        Ok(())
    }
}

struct AudioPlayerControllerState {
    playing: bool,
    position: Option<Duration>,
    seek_position: Option<Duration>,
}

impl AudioPlayerControllerState {
    fn new() -> Self {
        let playing = false;
        let position = None;
        let seek_position = None;
        Self {
            playing,
            position,
            seek_position,
        }
    }
}

struct AudioPlayerExecutor {
    tx: Sender<DecodedTrack>,
    handle: JoinHandle<()>,
}

impl AudioPlayerExecutor {
    fn new(controller: AudioPlayerController) -> Self {
        let (tx, rx) = mpsc::channel::<DecodedTrack>();
        let handle = std::thread::spawn(move || {
            let run = move || -> Result<(), Box<dyn Error>> {
                let mut output = AudioOutputter::new()?;
                while let Ok(mut track) = rx.recv() {
                    let mut resampler = None;
                    loop {
                        {
                            let mut state = controller.state.lock().unwrap();
                            if let Some(seek_position) = state.seek_position {
                                // TODO: skip packets
                                track.seek(seek_position)?;
                                (*state).seek_position = None;
                                controller.seeking_condvar.notify_all();
                            }
                            (*state).position = Some(track.progress());
                            while !state.playing {
                                state = controller.playing_condvar.wait(state).unwrap();
                            }
                        }

                        if let Ok(buffer) = track.next() {
                            if resampler.is_none() && buffer.spec().rate != *output.sample_rate() {
                                resampler =
                                    Some(SymphoniaResampler::new(*output.sample_rate(), &buffer)?);
                            }
                            let buffer = match resampler {
                                Some(ref mut resampler) => resampler.resample(buffer)?,
                                None => buffer,
                            };

                            output.write(buffer);
                        } else {
                            break;
                        }
                    }
                }
                Ok(())
            };
            run().unwrap();
        });

        Self { tx, handle }
    }

    fn queue(&self, track: DecodedTrack) -> Result<(), Box<dyn Error>> {
        self.tx.send(track)?;
        Ok(())
    }

    fn wait_until_end(self) -> Result<(), Box<dyn Error>> {
        drop(self.tx);
        self.handle.join().unwrap();
        Ok(())
    }
}
