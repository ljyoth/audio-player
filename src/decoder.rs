use std::{
    error::Error,
    fs::File,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use symphonia::core::{
    audio::AudioBufferRef,
    codecs::{Decoder, DecoderOptions},
    formats::{FormatOptions, FormatReader},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

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
            _ => println!("{} {:?} {}", tag.key, tag.std_key, tag.value),
        });
    }
    if let Some(metadata) = probed.metadata.get() {
        if let Some(metadata) = metadata.current() {
            metadata.tags().iter().for_each(|tag| match tag {
                _ => println!("{} {:?} {}", tag.key, tag.std_key, tag.value),
            });
        }
    }
    // let duration = probed
    //     .format
    //     .default_track()
    //     .map(|track| {
    //         if let Some(time_base) = track.codec_params.time_base {
    //             if let Some(n_frames) = track.codec_params.n_frames {
    //                 return Some(time_base.calc_time(n_frames).into());
    //             }
    //         }
    //         None
    //     })
    //     .flatten();

    let track = probed.format.default_track().unwrap();
    let decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    Ok(DecodedTrack {
        reader: probed.format,
        decoder,
    })
}

pub(super) struct DecodedTrack {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
}

impl DecodedTrack {
    pub(super) fn next(&mut self) -> Result<AudioBufferRef, Box<dyn Error>> {
        let packet = loop {
            let packet = self.reader.next_packet()?;
            if packet.track_id() == self.reader.default_track().unwrap().id {
                break packet;
            }
        };
        while !self.reader.metadata().is_latest() {
            self.reader.metadata().pop();
            if let Some(metadata) = self.reader.metadata().current() {
                metadata.tags().iter().for_each(|tag| match tag {
                    _ => println!("{} {:?} {}", tag.key, tag.std_key, tag.value),
                });
            }
        }

        let decoded = self.decoder.decode(&packet)?;
        Ok(decoded)
    }

    pub(super) fn reader(&self) -> &Box<dyn FormatReader> {
        &self.reader
    }

    pub(super) fn decoder(&self) -> &Box<dyn Decoder> {
        &self.decoder
    }
}
