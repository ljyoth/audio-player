use std::time::Duration;

use symphonia::core::{
    meta::{MetadataRevision, StandardTagKey, StandardVisualKey, Value, Visual},
    probe::ProbeResult,
};

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
    cover: Option<Visual>,
    title: Option<String>,
    duration: Option<Duration>,
}

impl TrackDetails {
    pub(super) fn new(probe_result: &mut ProbeResult) -> Self {
        fn read_metadata(metadata: &MetadataRevision) -> (Option<&Visual>, Option<&String>) {
            let cover = metadata
                .visuals()
                .iter()
                .find(|&visual| match visual.usage {
                    Some(StandardVisualKey::FrontCover) => true,
                    _ => {
                        println!("{:?} {}", visual.usage, visual.media_type);
                        false
                    }
                });
            let title = metadata.tags().iter().find_map(|tag| match tag.std_key {
                Some(StandardTagKey::TrackTitle) => match &tag.value {
                    Value::String(v) => Some(v),
                    _ => None,
                },
                _ => None,
            });
            metadata.tags().iter().for_each(|tag| match tag {
                _ => println!("{} {:?} {}", tag.key, tag.std_key, tag.value),
            });
            (cover, title)
        }

        // Give priority to metadata in container
        let metadata = probe_result.format.metadata();
        let (mut cover, mut title) = match metadata.current() {
            Some(metadata) => {
                let (cover, title) = read_metadata(metadata);
                (cover.map(ToOwned::to_owned), title.map(ToOwned::to_owned))
            }
            None => (None, None),
        };
        if let Some(metadata) = probe_result.metadata.get() {
            if let Some(metadata) = metadata.current() {
                let (cover_new, title_new) = read_metadata(metadata);
                if cover.is_none() {
                    cover = cover_new.map(ToOwned::to_owned);
                }
                if title.is_none() {
                    title = title_new.map(ToOwned::to_owned);
                }
            }
        }
        let duration = probe_result
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

        Self {
            cover,
            title,
            duration,
        }
    }

    pub fn cover(&self) -> Option<&Visual> {
        self.cover.as_ref()
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn duration(&self) -> Option<&Duration> {
        self.duration.as_ref()
    }
}
