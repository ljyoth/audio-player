use std::{fs::File, path::Path, time::Duration};

use symphonia::core::{
    audio::AudioBufferRef,
    codecs::{CodecParameters, Decoder, DecoderOptions},
    formats::{FormatOptions, FormatReader, Packet, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::TimeStamp,
};
use tracing::info;

use crate::{buffer::SampleBuffer, Track, TrackDetails};

#[derive(Debug, thiserror::Error)]
pub(super) enum DecoderError {
    #[error("TrackUnavailable")]
    TrackUnavailable,
    #[error("IO Error {0}")]
    IO(#[from] std::io::Error),
    #[error("SymphoniaError {0}")]
    Symphonia(#[from] symphonia::core::errors::Error),
    #[error("Failed to Calculate Progress")]
    ProgressUnavailable,
}

pub(super) fn decode<P: AsRef<Path>>(path: &P) -> Result<Track, DecoderError> {
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

    let details = TrackDetails::new(&mut probed);

    let track = probed
        .format
        .default_track()
        .ok_or(DecoderError::TrackUnavailable)?;
    let decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let progress = decoder.codec_params().start_ts;
    Ok(Track {
        decoded: DecodedTrack {
            reader: probed.format,
            decoder,
            progress,
            next_packet: None,
        },
        details,
    })
}

pub(super) struct DecodedTrack {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    progress: TimeStamp,
    // buffer next_packet call to accurately determine progress after seek call
    next_packet: Option<Packet>,
}

impl DecodedTrack {
    pub(super) fn codec_params(&self) -> &CodecParameters {
        self.decoder.codec_params()
    }

    // TODO: return SampleBuffer
    pub(super) fn next(&mut self) -> Result<SampleBuffer, DecoderError> {
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

        let decoded = match self.decoder.decode(&packet)? {
            AudioBufferRef::U8(buffer) => buffer.into(),
            AudioBufferRef::U16(buffer) => buffer.into(),
            AudioBufferRef::U24(buffer) => buffer.into(),
            AudioBufferRef::U32(buffer) => buffer.into(),
            AudioBufferRef::S8(buffer) => buffer.into(),
            AudioBufferRef::S16(buffer) => buffer.into(),
            AudioBufferRef::S24(buffer) => buffer.into(),
            AudioBufferRef::S32(buffer) => buffer.into(),
            AudioBufferRef::F32(buffer) => buffer.into(),
            AudioBufferRef::F64(buffer) => buffer.into(),
        };
        Ok(decoded)
    }

    fn next_packet(&mut self) -> Result<Packet, DecoderError> {
        let packet = loop {
            let packet = self.reader.next_packet()?;
            if packet.track_id()
                == self
                    .reader
                    .default_track()
                    .ok_or(DecoderError::TrackUnavailable)?
                    .id
            {
                break packet;
            }
        };
        self.progress = packet.ts();
        Ok(packet)
    }

    pub(super) fn seek(&mut self, progress: Duration) -> Result<(), DecoderError> {
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

    pub(super) fn progress(&self) -> Result<Duration, DecoderError> {
        Ok(self
            .decoder
            .codec_params()
            .time_base
            .ok_or(DecoderError::ProgressUnavailable)?
            .calc_time(self.progress)
            .into())
    }
}
