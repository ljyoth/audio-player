use std::{
    error::Error, fs::File, hash::Hash, io::BufReader, path::{Path, PathBuf}, time::Duration
};

use audio_player::{AudioPlayerError, TrackDetails};

pub(super) struct AudioPlayer {
    track: Option<Track>,
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
        self.track = Some(Track {
            file_path: path.as_ref().to_path_buf(),
            details: track.details().clone(),
        });
        self.player.queue(track)?;

        Ok(())
    }

    /// Get the current playing track
    pub(super) fn current(&self) -> Option<&Track> {
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

pub(super) struct Track {
    file_path: PathBuf,
    details: TrackDetails,
}

impl Track {
    pub(super) fn file_path(&self) -> &PathBuf {
        &self.file_path
    }

    pub(super) fn details(&self) -> &TrackDetails {
        &self.details
    }
}

impl Hash for Track {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.file_path.hash(state);
    }
}