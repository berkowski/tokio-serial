/// filter terms from the serial port and make a sound when found.
/// 
/// dave horner 10/24
/// 
/// Default settings for Nordic Thingy53, nrf5340dk, and other nordic devices (baud/com).
use bytes::BytesMut;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures::stream::StreamExt;
use std::sync::Mutex;
use std::sync::Arc;
use std::{env, io, str};
use tokio::time::Duration;
use tokio_serial::SerialPortBuilderExt;
use tokio_util::codec::{Decoder, Encoder};
extern crate anyhow;

#[cfg(unix)]
const DEFAULT_TTY: &str = "/dev/ttyACM1";
#[cfg(windows)]
const DEFAULT_TTY: &str = "COM8";

// Create the table of findable strings and their sound parameters
fn create_find_text_map() -> HashMap<&'static str, SoundParams> {
    let mut map = HashMap::new();
    map.insert("Using Zephyr OS", SoundParams {
        waveform: Waveform::Sine,
        frequency: 500.0,
        duration: 150,
    });
    map.insert("Error", SoundParams {
        waveform: Waveform::Square,
        frequency: 800.0,
        duration: 150,
    });
    map.insert("Warning", SoundParams {
        waveform: Waveform::Triangle,
        frequency: 300.0,
        duration: 150,
    });
    map.insert("DK handling", SoundParams {
        waveform: Waveform::Triangle,
        frequency: 600.0,
        duration: 150,
    });
    map
}

#[tokio::main]
async fn main() -> tokio_serial::Result<()> {
    let mut args = env::args();
    let tty_path = args.nth(1).unwrap_or_else(|| DEFAULT_TTY.into());


    #[cfg(unix)]
    let mut port = tokio_serial::new(tty_path, 115200).open_native_async()?; // Mutable on Unix
    #[cfg(windows)]
    let port = tokio_serial::new(tty_path, 115200).open_native_async()?;      // Immutable on Windows
    #[cfg(unix)]
    port.set_exclusive(false)
        .expect("Unable to set serial port exclusive to false");
    let mut reader = LineCodec.framed(port);

    let find_text_map = create_find_text_map();
    while let Some(line_result) = reader.next().await {
        let line = line_result.expect("Failed to read line");
        print!("{}", line);

        for (phrase, params) in &find_text_map {
            if line.contains(phrase) {
                let params_clone = params.clone();
                tokio::spawn(async move {
                    let _ = play_sound(params_clone).await;
                });
                break;
            }
        }
    }
    Ok(())
}


///////////////////////////////////
///  Codec
/// ///////////////////////////////

struct LineCodec;

impl Decoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let newline = src.as_ref().iter().position(|b| *b == b'\n');
        if let Some(n) = newline {
            let line = src.split_to(n + 1);
            return match str::from_utf8(line.as_ref()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "Invalid String")),
            };
        }
        Ok(None)
    }
}

impl Encoder<String> for LineCodec {
    type Error = io::Error;

    fn encode(&mut self, _item: String, _dst: &mut BytesMut) -> Result<(), Self::Error> {
        Ok(())
    }
}


///////////////////////////////////
///  All this code to make noise.
/// ///////////////////////////////
use std::error::Error;
use std::f32::consts::PI;
use std::thread;
use std::collections::HashMap;

#[derive(Clone)]
struct SoundParams {
    waveform: Waveform,
    frequency: f32,
    duration: u64,
}

async fn play_sound(params: SoundParams) -> Result<(), Box<dyn Error + Send + Sync>> {
    let oscillator = Arc::new(Mutex::new(Oscillator::new(44100.0, params.frequency, params.waveform)));
    let oscillator_clone = Arc::clone(&oscillator);

    let play_handle = thread::spawn(move || {
        let stream = start_audio_stream_arc(oscillator_clone).expect("Failed to start audio stream");
        stream.play().expect("Failed to play audio stream");
        std::thread::sleep(Duration::from_millis(params.duration));
    });

    play_handle.join().expect("Play thread panicked");
    Ok(())
}

#[derive(Clone, Copy)]
pub enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
}

pub struct Oscillator {
    pub sample_rate: f32,
    pub waveform: Waveform,
    pub current_sample_index: f32,
    pub frequency_hz: f32,
}

impl Oscillator {
    pub fn new(sample_rate: f32, frequency_hz: f32, waveform: Waveform) -> Self {
        Self {
            sample_rate,
            waveform,
            current_sample_index: 0.0,
            frequency_hz,
        }
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    pub fn tick(&mut self) -> f32 {
        match self.waveform {
            Waveform::Sine => self.sine_wave(),
            Waveform::Square => self.square_wave(),
            Waveform::Saw => self.saw_wave(),
            Waveform::Triangle => self.triangle_wave(),
        }
    }

    fn advance_sample(&mut self) {
        self.current_sample_index = (self.current_sample_index + 1.0) % self.sample_rate;
    }

    fn calculate_sine_output(&self) -> f32 {
        (self.current_sample_index * self.frequency_hz * 2.0 * PI / self.sample_rate).sin()
    }

    fn sine_wave(&mut self) -> f32 {
        self.advance_sample();
        self.calculate_sine_output()
    }

    fn square_wave(&mut self) -> f32 {
        self.generative_waveform(2, 1.0)
    }

    fn saw_wave(&mut self) -> f32 {
        self.generative_waveform(1, 1.0)
    }

    fn triangle_wave(&mut self) -> f32 {
        self.generative_waveform(2, 2.0)
    }

    fn generative_waveform(&mut self, harmonic_step: i32, gain_factor: f32) -> f32 {
        self.advance_sample();
        let mut output = 0.0;
        let mut harmonic = 1;
        while self.frequency_hz * harmonic as f32 <= self.sample_rate / 2.0 {
            let gain = 1.0 / (harmonic as f32).powf(gain_factor);
            output += gain * self.calculate_sine_output();
            harmonic += harmonic_step;
        }
        output
    }
}

use cpal::{Sample, SampleFormat, SizedSample};

pub fn start_audio_stream(waveform: Waveform, frequency: f32) -> anyhow::Result<cpal::Stream> {
    let (_host, device, config) = host_device_setup()?;
    match config.sample_format() {
        SampleFormat::F32 => create_stream::<f32>(&device, &config.into(), waveform, frequency),
        _ => Err(anyhow::Error::msg("Unsupported sample format")),
    }
}

pub fn start_audio_stream_arc(oscillator: Arc<Mutex<Oscillator>>) -> anyhow::Result<cpal::Stream> {
    let (_host, device, config) = host_device_setup()?;
    match config.sample_format() {
        SampleFormat::F32 => create_stream_arc::<f32>(&device, &config.into(), oscillator),
        _ => Err(anyhow::Error::msg("Unsupported sample format")),
    }
}

fn host_device_setup(
) -> Result<(cpal::Host, cpal::Device, cpal::SupportedStreamConfig), anyhow::Error> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::Error::msg("No output device available"))?;
    let config = device.default_output_config()?;
    Ok((host, device, config))
}

pub fn create_stream_arc<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    oscillator: Arc<Mutex<Oscillator>>,
) -> anyhow::Result<cpal::Stream>
where
    T: Sample + SizedSample + cpal::FromSample<f32>,
{
    let num_channels = config.channels as usize;

    let stream = device.build_output_stream(
        config,
        move |output: &mut [T], _| {
            let mut osc = oscillator.lock().unwrap();
            for frame in output.chunks_mut(num_channels) {
                let sample_value: T = T::from_sample(osc.tick());
                for sample in frame.iter_mut() {
                    *sample = sample_value;
                }
            }
        },
        |err| eprintln!("Error: {}", err),
        None,
    )?;

    Ok(stream)
}

fn create_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    waveform: Waveform,
    frequency: f32,
) -> anyhow::Result<cpal::Stream>
where
    T: Sample + SizedSample + cpal::FromSample<f32>,
{
    let mut oscillator = Oscillator::new(config.sample_rate.0 as f32, frequency, waveform);
    let num_channels = config.channels as usize;

    let stream = device.build_output_stream(
        config,
        move |output: &mut [T], _| {
            for frame in output.chunks_mut(num_channels) {
                let sample_value: T = T::from_sample(oscillator.tick());
                for sample in frame.iter_mut() {
                    *sample = sample_value;
                }
            }
        },
        |err| eprintln!("Error: {}", err),
        None,
    )?;

    Ok(stream)
}
