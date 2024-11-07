use std::{error::Error, fs::File, io::BufReader, path::Path, time::Duration};

use audio_player::{AudioPlayerError, TrackDetails};

pub(super) struct AudioPlayer {
    track: Option<TrackDetails>,
    player: audio_player::AudioPlayer,
}

impl AudioPlayer {
    pub(super) fn new() -> Self {
        let player = audio_player::AudioPlayer::new();

        Self {
            track: None,
            player,
        }
    }

    pub(super) fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<(), AudioPlayerError> {
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
        self.player.controller().play();
    }

    pub(super) fn playing(&self) -> bool {
        self.player.controller().playing()
    }

    pub(super) fn pause(&self) {
        self.player.controller().pause();
    }

    pub(super) fn stop(&mut self) {
        self.player.drain()
    }

    pub(super) fn position(&self) -> Duration {
        self.player
            .controller()
            .position()
            .unwrap_or(Duration::from_secs(0))
    }

    pub(super) fn seek(&self, position: Duration) {
        self.player.controller().seek(position);
    }
}
