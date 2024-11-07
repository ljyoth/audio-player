use std::{
    error::Error,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SendError, Sender},
        Arc, Condvar, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    decoder::{self, DecodedTrack, DecoderError},
    output::AudioOutputter,
    resampler::SymphoniaResampler,
    track::Track,
};

#[derive(Debug, thiserror::Error)]
pub enum AudioPlayerError {
    #[error("DecoderError {0}")]
    Decoder(#[from] DecoderError),
    #[error("ExecutorError {0}")]
    Executor(#[from] AudioPlayerExecutorError),
}

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

    pub fn open<F: AsRef<Path>>(&mut self, file: F) -> Result<Track, AudioPlayerError> {
        let track = decoder::decode(&file)?;
        Ok(track)
    }

    // Place track on queue
    pub fn queue(&self, track: Track) -> Result<(), AudioPlayerError> {
        self.executor.queue(track.decoded)?;
        Ok(())
    }

    // drain all tracks in the queue
    pub fn drain(&mut self) {
        self.executor = AudioPlayerExecutor::new(self.controller.clone());
    }

    pub fn wait_until_end(self) {
        self.executor.wait_until_end();
    }
}

#[derive(Debug)]
pub enum AudioPlayerControllerError {
    NotPlaying,
}

#[derive(Clone)]
pub struct AudioPlayerController {
    state: Arc<Mutex<AudioPlayerControllerState>>,
    executor_condvar: Arc<Condvar>,
    controller_condvar: Arc<Condvar>,
}

impl AudioPlayerController {
    fn new() -> Self {
        let state = Arc::new(Mutex::new(AudioPlayerControllerState::new()));
        let executor_condvar = Arc::new(Condvar::new());
        let controller_condvar = Arc::new(Condvar::new());
        Self {
            state,
            executor_condvar,
            controller_condvar,
        }
    }

    pub fn play(&self) {
        let mut state = self.state.lock().unwrap();
        (*state).playing = true;
        self.executor_condvar.notify_all();
    }

    pub fn pause(&self) {
        let mut state = self.state.lock().unwrap();
        (*state).playing = false;
        self.executor_condvar.notify_all();
    }

    pub fn playing(&self) -> bool {
        let state = self.state.lock().unwrap();
        (*state).playing
    }

    pub fn position(&self) -> Result<Duration, AudioPlayerControllerError> {
        let state = self.state.lock().unwrap();
        state.position.ok_or(AudioPlayerControllerError::NotPlaying)
    }

    pub fn seek(&self, progress: Duration) {
        let mut state = self.state.lock().unwrap();
        if !state.running {
            return;
        }
        (*state).seek_position = Some(progress);
        while state.playing && state.seek_position.is_some() {
            state = self.controller_condvar.wait(state).unwrap();
        }
    }
}

struct AudioPlayerControllerState {
    running: bool,
    playing: bool,
    position: Option<Duration>,
    seek_position: Option<Duration>,
}

impl AudioPlayerControllerState {
    fn new() -> Self {
        let running = false;
        let playing = false;
        let position = None;
        let seek_position = None;
        Self {
            running,
            playing,
            position,
            seek_position,
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum AudioPlayerExecutorError {
    #[error("SendError {0}")]
    Send(#[from] SendError<DecodedTrack>),
}

struct AudioPlayerExecutor {
    /// This is an option to drop in [AudioPlayerExecutor::wait_until_end]
    tx: Option<Sender<DecodedTrack>>,
    dropped: Arc<AtomicBool>,
    /// This is an option to `join` in [AudioPlayerExecutor::wait_until_end]
    handle: Option<JoinHandle<()>>,
}

impl AudioPlayerExecutor {
    fn new(controller: AudioPlayerController) -> Self {
        let (tx, rx) = mpsc::channel::<DecodedTrack>();
        let dropped = Arc::new(AtomicBool::new(false));
        let dropped_clone = dropped.clone();
        let handle = std::thread::spawn(move || {
            let run = move || -> Result<(), Box<dyn Error>> {
                let mut output = AudioOutputter::new()?;
                output.play()?;
                while let Ok(mut track) = rx.recv() {
                    let mut resampler = None;
                    {
                        let mut state = controller.state.lock().unwrap();
                        state.running = true;
                    }
                    while !dropped.load(std::sync::atomic::Ordering::Acquire) {
                        {
                            let mut state = controller.state.lock().unwrap();
                            if let Some(seek_position) = state.seek_position {
                                // TODO: skip packets
                                track.seek(seek_position)?;
                                (*state).seek_position = None;
                                controller.controller_condvar.notify_all();
                            }
                            (*state).position = Some(track.progress()?);
                            let paused = !state.playing;
                            while !state.playing {
                                output.pause()?;
                                state = controller.executor_condvar.wait(state).unwrap();
                            }
                            if paused {
                                output.play()?;
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
                    {
                        let mut state = controller.state.lock().unwrap();
                        state.running = false;
                        controller.controller_condvar.notify_all();
                    }
                }
                Ok(())
            };
            run().unwrap();
        });

        Self {
            tx: Some(tx),
            dropped: dropped_clone,
            handle: Some(handle),
        }
    }

    fn queue(&self, track: DecodedTrack) -> Result<(), AudioPlayerExecutorError> {
        self.tx.as_ref().unwrap().send(track)?;
        Ok(())
    }

    fn wait_until_end(mut self) {
        self.tx = None;
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

impl Drop for AudioPlayerExecutor {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}
