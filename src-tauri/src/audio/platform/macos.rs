//! macOS system audio capture via ScreenCaptureKit
//!
//! Implementation planned for step 10-cross-platform.md

use anyhow::Result;

use crate::audio::buffer::AudioBuffer;
use crate::audio::AudioChannel;

pub struct SystemAudioCapture {
    buffer: AudioBuffer,
}

impl SystemAudioCapture {
    pub fn new() -> Result<Self> {
        Ok(Self {
            buffer: AudioBuffer::new(AudioChannel::System),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        anyhow::bail!("macOS system audio capture not yet implemented. See 10-cross-platform.md")
    }

    pub fn stop(&mut self) {}

    pub fn is_running(&self) -> bool {
        false
    }

    pub fn buffer(&self) -> &AudioBuffer {
        &self.buffer
    }
}
