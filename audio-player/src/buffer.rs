/// planar format
pub(super) struct SampleBuffer {
    // TODO: support other types
    buffer: Vec<Vec<f64>>,
}

impl SampleBuffer {
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

    pub(super) fn with_buffer(buffer: Vec<Vec<f64>>) -> Self {
        Self { buffer }
    }

    pub(super) fn resize(&mut self, channels: usize, samples_per_channel: usize) {
        self.buffer.truncate(channels);
        self.buffer.iter_mut().for_each(|b| {
            b.resize(samples_per_channel, 0.0);
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

    pub(super) fn channel_samples(&self) -> impl Iterator<Item = &[f64]> + '_ {
        self.buffer.iter().map(|b| b.as_slice())
    }

    pub(super) fn channel_samples_mut(&mut self) -> impl Iterator<Item = &mut [f64]> + '_ {
        self.buffer.iter_mut().map(|b| b.as_mut())
    }

    pub(super) fn samples(&self, channel: usize) -> Option<&[f64]> {
        self.buffer.get(channel).map(|b| b.as_slice())
    }

    pub(super) fn samples_mut(&mut self, channel: usize) -> Option<&mut [f64]> {
        self.buffer.get_mut(channel).map(|b| b.as_mut())
    }
}

impl AsRef<[Vec<f64>]> for SampleBuffer {
    fn as_ref(&self) -> &[Vec<f64>] {
        &self.buffer
    }
}

impl AsMut<[Vec<f64>]> for SampleBuffer {
    fn as_mut(&mut self) -> &mut [Vec<f64>] {
        &mut self.buffer
    }
}
