use std::{error::Error, fs::File, io::BufReader, path::Path, time::Duration};

use rodio::Source;
use symphonia::core::{
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{MetadataOptions, MetadataRevision, StandardTagKey, StandardVisualKey, Value, Visual},
    probe::{Hint, ProbeResult},
};

pub(super) struct AudioPlayer {
    track: Option<TrackDetails>,

    // TODO: remove rodio
    stream: rodio::OutputStream,
    sink: rodio::Sink,
}

impl AudioPlayer {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        // TODO: remove rodio
        let (stream, handle) = rodio::OutputStream::try_default()?;
        let sink = rodio::Sink::try_new(&handle)?;
        sink.pause();

        Ok(Self {
            track: None,
            stream,
            sink,
        })
    }

    // TODO: proper errors
    pub(super) fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Box<dyn Error>> {
        let file = File::open(path.as_ref())?;
        let decoder = rodio::Decoder::new(BufReader::new(file.try_clone()?))?;
        println!("rodio {:?}", decoder.total_duration());
        // TODO: remove rodio
        self.sink
            .append(rodio::Decoder::new(BufReader::new(file.try_clone()?))?);

        let mss = MediaSourceStream::new(Box::new(File::open(path.as_ref())?), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.as_ref().extension() {
            hint.with_extension(&ext.to_string_lossy());
        }
        let mut probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        self.track = Some(TrackDetails::parse(&mut probed));

        Ok(())
    }

    /// Get the current playing track
    pub(super) fn current(&self) -> Option<&TrackDetails> {
        self.track.as_ref()
    }

    pub(super) fn play(&self) {
        self.sink.play();
    }

    pub(super) fn playing(&self) -> bool {
        !self.sink.is_paused()
    }

    pub(super) fn pause(&self) {
        self.sink.pause();
    }

    pub(super) fn stop(&mut self) {
        self.sink.stop();
        self.track = None;
    }

    pub(super) fn position(&self) -> Duration {
        self.sink.get_pos()
    }

    pub(super) fn seek(&self, position: Duration) -> Result<(), Box<dyn Error>> {
        self.sink.try_seek(position)?;
        Ok(())
    }
}

pub(super) struct TrackDetails {
    cover: Option<Visual>,
    title: Option<String>,
    duration: Option<Duration>,
}

impl TrackDetails {
    fn parse(probe_result: &mut ProbeResult) -> Self {
        let mut cover = None;
        let mut title = None;
        // Give priority to metadata in container
        if let Some(metadata) = probe_result.format.metadata().current() {
            // TODO: avoid clone
            cover = get_cover(metadata).map(ToOwned::to_owned);
            title = get_title(metadata).map(ToOwned::to_owned);
            metadata.tags().iter().for_each(|tag| match tag {
                _ => println!("{} {:?} {}", tag.key, tag.std_key, tag.value),
            });
        }
        if let Some(metadata) = probe_result.metadata.get() {
            if let Some(metadata) = metadata.current() {
                if cover.is_none() {
                    // TODO: avoid clone
                    cover = get_cover(&metadata).map(ToOwned::to_owned);
                }
                if title.is_none() {
                    title = get_title(metadata).map(ToOwned::to_owned);
                }

                metadata.tags().iter().for_each(|tag| match tag {
                    _ => println!("{} {:?} {}", tag.key, tag.std_key, tag.value),
                });
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
        println!("{:?}", duration);

        Self {
            cover,
            title,
            duration,
        }
    }

    pub(super) fn cover(&self) -> Option<&Visual> {
        self.cover.as_ref()
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(super) fn duration(&self) -> Option<&Duration> {
        self.duration.as_ref()
    }
}

fn get_cover(metadata: &MetadataRevision) -> Option<&Visual> {
    metadata
        .visuals()
        .iter()
        .find(|&visual| match visual.usage {
            Some(StandardVisualKey::FrontCover) => true,
            _ => {
                println!("{:?} {}", visual.usage, visual.media_type);
                false
            }
        })
}

fn get_title(metadata: &MetadataRevision) -> Option<&String> {
    metadata.tags().iter().find_map(|tag| match tag.std_key {
        Some(StandardTagKey::TrackTitle) => match &tag.value {
            Value::String(v) => Some(v),
            _ => None,
        },
        _ => None,
    })
}
