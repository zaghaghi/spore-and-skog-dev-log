//! The one audio stream (CPAL), fed by the desk: the game's
//! `src/audio/device.rs`, less its iPad and browser lines.

use std::sync::mpsc::Receiver;

use cpal::{
    Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::desk::{Command, Desk};

/// Open the default output and start it playing what the desk makes.
pub fn open(commands: Receiver<Command>) -> Result<Stream, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no audio output")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let desk = Desk::new(config.sample_rate, commands);
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, desk),
        SampleFormat::I16 => build::<i16>(&device, config, desk),
        SampleFormat::U16 => build::<u16>(&device, config, desk),
        SampleFormat::I32 => build::<i32>(&device, config, desk),
        other => return Err(format!("unsupported sample format {other}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &Device,
    config: StreamConfig,
    mut desk: Desk,
) -> Result<Stream, String> {
    let channels = config.channels as usize;
    let mut scratch = Vec::<f32>::new();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                scratch.resize(data.len(), 0.0);
                desk.render(&mut scratch, channels);
                for (d, s) in data.iter_mut().zip(&scratch) {
                    *d = T::from_sample(*s);
                }
            },
            |e| eprintln!("audio: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}
