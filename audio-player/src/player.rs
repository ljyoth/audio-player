use std::{
    error::Error,
    io::Seek,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Sender},
        Arc, Condvar, LockResult, Mutex, MutexGuard,
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    decoder::{self, DecodedTrack},
    output::{AudioOutputWriter, AudioOutputter},
    resampler::SymphoniaResampler,
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

    pub fn controller(&mut self) -> &mut AudioPlayerController {
        &mut self.controller
    }

    pub fn open<F: AsRef<Path>>(&mut self, file: F) -> Result<(), Box<dyn Error>> {
        self.executor.queue(file)?;
        Ok(())
    }

    pub fn wait_until_end(self) -> Result<(), Box<dyn Error>> {
        self.executor.wait_until_end()?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct AudioPlayerController {
    state: Arc<Mutex<AudioPlayerControllerState>>,
    cond_var: Arc<Condvar>,
}

impl AudioPlayerController {
    fn new() -> Self {
        let state = Arc::new(Mutex::new(AudioPlayerControllerState::new()));
        let cond_var = Arc::new(Condvar::new());
        Self { state, cond_var }
    }

    pub fn play(&mut self) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().unwrap();
        (*state).playing = true;
        self.cond_var.notify_all();
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().unwrap();
        (*state).playing = false;
        self.cond_var.notify_all();
        Ok(())
    }

    pub fn position(&self) -> Result<Duration, Box<dyn Error>> {
        let state = self.state.lock().unwrap();
        Ok(state.position.ok_or("unavailable")?)
    }

    pub fn seek(&mut self, progress: Duration) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().unwrap();
        (*state).seek_position = Some(progress);
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
        let position = Some(Duration::from_secs(0));
        let seek_position = None;
        Self {
            playing,
            position,
            seek_position,
        }
    }
}

struct AudioPlayerExecutor {
    tx: Sender<PathBuf>,
    handle: JoinHandle<()>,
}

impl AudioPlayerExecutor {
    fn new(controller: AudioPlayerController) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let run = move || -> Result<(), Box<dyn Error>> {
                let mut output = AudioOutputter::new()?;
                while let Ok(file) = rx.recv() {
                    let mut track = decoder::decode(&file)?;
                    let mut resampler = None;
                    loop {
                        {
                            let mut state = controller.state.lock().unwrap();
                            if let Some(seek_position) = state.seek_position {
                                // TODO: skip packets
                                track.seek(seek_position)?;
                                (*state).seek_position = None;
                            }
                            (*state).position = Some(track.progress());
                            while !state.playing {
                                state = controller.cond_var.wait(state).unwrap();
                            }
                        }

                        if let Ok(buffer) = track.next() {
                            if resampler.is_none() && buffer.spec().rate != *output.sample_rate() {
                                resampler =
                                    Some(SymphoniaResampler::new(*output.sample_rate(), &buffer));
                            }
                            let buffer = match resampler {
                                Some(ref mut resampler) => resampler.resample(buffer),
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

    fn queue<F: AsRef<Path>>(&mut self, file: F) -> Result<(), Box<dyn Error>> {
        self.tx.send(file.as_ref().to_path_buf())?;
        Ok(())
    }

    fn wait_until_end(self) -> Result<(), Box<dyn Error>> {
        self.handle.join().unwrap();
        Ok(())
    }
}
