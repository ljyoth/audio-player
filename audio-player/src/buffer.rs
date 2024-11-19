/// TODO: custom sample?
pub(super) use cpal::{FromSample, Sample};

pub(super) trait ToSample<S> {
    fn to_sample(self) -> S;
}

impl<S, T: cpal::FromSample<S>> ToSample<T> for S {
    #[inline]
    fn to_sample(self) -> T {
        T::from_sample_(self)
    }
}

/// planar format
pub(super) struct SampleBuffer<T: Sample> {
    // TODO: support other types
    buffer: Vec<Vec<T>>,
}

impl<T: Sample> SampleBuffer<T> {
    pub(super) fn new() -> Self {
        Self { buffer: vec![] }
    }

    pub(super) fn with_capacity(channels: usize, samples_per_channel: usize) -> Self {
        Self {
            buffer: (0..channels)
                .map(|_| Vec::with_capacity(samples_per_channel))
                .collect(),
        }
    }

    pub(super) fn with_buffer(buffer: Vec<Vec<T>>) -> Self {
        Self { buffer }
    }

    pub(super) fn resize(&mut self, channels: usize, samples_per_channel: usize) {
        self.buffer.truncate(channels);
        self.buffer.iter_mut().for_each(|b| {
            b.resize(samples_per_channel, T::EQUILIBRIUM);
        });
    }

    pub(super) fn channels(&self) -> usize {
        self.buffer.len()
    }

    pub(super) fn frames(&self) -> usize {
        match self.buffer.get(0) {
            Some(b) => b.len(),
            None => 0,
        }
    }

    pub(super) fn channel_samples(&self) -> impl Iterator<Item = &[T]> + '_ {
        self.buffer.iter().map(|b| b.as_slice())
    }

    pub(super) fn channel_samples_mut(&mut self) -> impl Iterator<Item = &mut [T]> + '_ {
        self.buffer.iter_mut().map(|b| b.as_mut())
    }

    pub(super) fn samples(&self, channel: usize) -> Option<&[T]> {
        self.buffer.get(channel).map(|b| b.as_slice())
    }

    pub(super) fn samples_mut(&mut self, channel: usize) -> Option<&mut [T]> {
        self.buffer.get_mut(channel).map(|b| b.as_mut())
    }

    pub(super) fn interleaved(&self) -> impl Iterator<Item = T> + '_ {
        (0..self.frames()).flat_map(|f| self.buffer.iter().map(move |b| b[f]))
    }
}

impl<T: Sample> AsRef<[Vec<T>]> for SampleBuffer<T> {
    fn as_ref(&self) -> &[Vec<T>] {
        &self.buffer
    }
}

impl<T: Sample> AsMut<[Vec<T>]> for SampleBuffer<T> {
    fn as_mut(&mut self) -> &mut [Vec<T>] {
        &mut self.buffer
    }
}
