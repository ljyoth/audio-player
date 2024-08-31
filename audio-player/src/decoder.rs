use std::{error::Error, fs::File, path::Path, time::Duration};

use symphonia::core::{
    audio::AudioBufferRef,
    codecs::{Decoder, DecoderOptions},
    formats::{FormatOptions, FormatReader, Packet, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::TimeStamp,
};
use tracing::info;

pub(super) fn decode<P: AsRef<Path>>(path: &P) -> Result<DecodedTrack, Box<dyn Error>> {
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

    if let Some(metadata) = probed.format.metadata().current() {
        metadata.tags().iter().for_each(|tag| match tag {
            _ => info!("{} {:?} {}", tag.key, tag.std_key, tag.value),
        });
    }
    if let Some(metadata) = probed.metadata.get() {
        if let Some(metadata) = metadata.current() {
            metadata.tags().iter().for_each(|tag| match tag {
                _ => info!("{} {:?} {}", tag.key, tag.std_key, tag.value),
            });
        }
    }
    let duration = probed
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
        .flatten()
        .unwrap();

    let track = probed.format.default_track().unwrap();
    let decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let progress = decoder.codec_params().start_ts;
    Ok(DecodedTrack {
        reader: probed.format,
        decoder,
        duration,
        progress,
        next_packet: None,
    })
}

pub(super) struct DecodedTrack {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    duration: Duration,
    progress: TimeStamp,
    // buffer next_packet call to accurately determine progress after seek call
    next_packet: Option<Packet>,
}

impl DecodedTrack {
    pub(super) fn next(&mut self) -> Result<AudioBufferRef, Box<dyn Error>> {
        let packet = match self.next_packet.take() {
            Some(packet) => {
                self.next_packet = None;
                packet
            }
            None => self.next_packet()?,
        };
        self.progress = packet.ts();

        while !self.reader.metadata().is_latest() {
            self.reader.metadata().pop();
            if let Some(metadata) = self.reader.metadata().current() {
                metadata.tags().iter().for_each(|tag| match tag {
                    _ => info!("{} {:?} {}", tag.key, tag.std_key, tag.value),
                });
            }
        }

        let decoded = self.decoder.decode(&packet)?;
        Ok(decoded)
    }

    fn next_packet(&mut self) -> Result<Packet, Box<dyn Error>> {
        let packet = loop {
            let packet = self.reader.next_packet()?;
            if packet.track_id() == self.reader.default_track().unwrap().id {
                break packet;
            }
        };
        self.progress = packet.ts();
        Ok(packet)
    }

    pub(super) fn seek(&mut self, progress: Duration) -> Result<(), Box<dyn Error>> {
        self.reader.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: progress.into(),
                track_id: None,
            },
        )?;
        self.decoder.reset();
        self.next_packet = Some(self.next_packet()?);
        Ok(())
    }

    pub(super) fn duration(&self) -> Duration {
        self.duration
    }

    pub(super) fn progress(&self) -> Duration {
        self.decoder
            .codec_params()
            .time_base
            .unwrap()
            .calc_time(self.progress)
            .into()
    }

    pub(super) fn reader(&self) -> &Box<dyn FormatReader> {
        &self.reader
    }
}
