use std::time::Duration;

use symphonia::core::{
    meta::{MetadataRevision, StandardTagKey, StandardVisualKey, Value, Visual},
    probe::ProbeResult,
};
use tracing::debug;

use crate::decoder::DecodedTrack;

pub struct Track {
    pub(super) decoded: DecodedTrack,
    pub(super) details: TrackDetails,
}

impl Track {
    pub fn details(&self) -> &TrackDetails {
        &self.details
    }
}

#[derive(Debug, Clone)]
pub struct TrackDetails {
    duration: Option<Duration>,
    title: Option<String>,
    artist: Option<String>,

    // TODO: custom Image type
    cover: Option<Visual>,
}

impl TrackDetails {
    pub(super) fn new(probe_result: &mut ProbeResult) -> Self {
        // Give priority to metadata in container
        let metadata = probe_result.format.metadata();
        let mut new = match metadata.current() {
            Some(metadata) => Self::read_metadata(metadata),
            None => Self {
                duration: None,
                title: None,
                artist: None,
                cover: None,
            },
        };
        if let Some(metadata) = probe_result.metadata.get() {
            if let Some(metadata) = metadata.current() {
                let new_2 = Self::read_metadata(metadata);
                if new.cover.is_none() {
                    new.cover = new_2.cover;
                }
                if new.title.is_none() {
                    new.title = new_2.title;
                }
            }
        }
        new.duration = probe_result
            .format
            .default_track()
            .map(|track| {
                if let Some(time_base) = track.codec_params.time_base {
                    if let Some(n_frames) = track.codec_params.n_frames {
                        return Some(time_base.calc_time(n_frames).into());
                    }
                }
                None
            })
            .flatten();

        new
    }

    fn read_metadata(metadata: &MetadataRevision) -> Self {
        let mut new = Self {
            duration: None,
            title: None,
            artist: None,
            cover: None,
        };
        new.cover = metadata
            .visuals()
            .iter()
            .find(|&visual| match visual.usage {
                Some(StandardVisualKey::FrontCover) => true,
                _ => {
                    debug!(
                        "visual: {{ usage: {:?} media_type: {} }}",
                        visual.usage, visual.media_type
                    );
                    false
                }
            })
            .cloned();
        metadata.tags().iter().for_each(|tag| match tag.std_key {
            Some(StandardTagKey::TrackTitle) => {
                new.title = match &tag.value {
                    Value::String(v) => Some(v.clone()),
                    _ => None,
                }
            }
            Some(StandardTagKey::Artist) => {
                new.artist = match &tag.value {
                    Value::String(v) => Some(v.clone()),
                    _ => None,
                };
            }
            _ => debug!("{} {:?} {}", tag.key, tag.std_key, tag.value),
        });
        new
    }

    pub fn duration(&self) -> Option<&Duration> {
        self.duration.as_ref()
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }

    pub fn cover(&self) -> Option<&Visual> {
        self.cover.as_ref()
    }

}
