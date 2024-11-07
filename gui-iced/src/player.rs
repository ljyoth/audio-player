use std::{error::Error, fs::File, io::BufReader, path::Path, time::Duration};

use audio_player::TrackDetails;

pub(super) struct AudioPlayer {
    track: Option<TrackDetails>,
    player: audio_player::AudioPlayer,
}

impl AudioPlayer {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let player = audio_player::AudioPlayer::new();

        Ok(Self {
            track: None,
            player,
        })
    }

    // TODO: proper errors
    pub(super) fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Box<dyn Error>> {
        let track = self.player.open(path.as_ref().to_path_buf())?;
        self.track = Some(track.details().clone());
        self.player.queue(track)?;

        Ok(())
    }

    /// Get the current playing track
    pub(super) fn current(&self) -> Option<&TrackDetails> {
        self.track.as_ref()
    }

    pub(super) fn play(&self) {
        self.player.controller().play().unwrap();
    }

    pub(super) fn playing(&self) -> bool {
        self.player.controller().playing().unwrap()
    }

    pub(super) fn pause(&self) {
        self.player.controller().pause().unwrap();
    }

    pub(super) fn stop(&self) {
        self.player.drain().unwrap()
    }

    pub(super) fn position(&self) -> Duration {
        self.player.controller().position().unwrap()
    }

    pub(super) fn seek(&self, position: Duration) -> Result<(), Box<dyn Error>> {
        self.player.controller().seek(position)?;
        Ok(())
    }
}
