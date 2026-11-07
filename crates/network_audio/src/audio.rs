//! CODEX (GPT-6): bounded live PCM queues, device-rate conversion, and shared native stereo mixer.
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use cpal::traits::DeviceTrait;
use crate::device::{self, Direction};
use crate::{Pose, VoiceConfig, VoiceStatus, spatial};

pub(crate) const RATE: usize = 48_000;
pub(crate) const FRAME: usize = 960;
const MAX_QUEUE: usize = RATE / 5;

// CODEX: each speaker owns a bounded mono playout queue; there is never local microphone monitoring.
#[derive(Default)]
pub(crate) struct Speaker {
    pub position: Option<[f32; 3]>,
    pub pcm: VecDeque<f32>,
    pub fraction: f64,
    pub playing: bool,
    pub gain: [f32; 2],
    // CODEX (GPT-6): optional caller diagnostics measure decoded remote speech, never local/radio audio.
    pub rms: f32,
    pub received_at: Option<std::time::Instant>,
}

// CODEX: one short-lived buffer lock bridges audio callbacks and asynchronous transport tasks.
pub(crate) struct State {
    pub config: VoiceConfig,
    pub pose: Pose,
    pub status: VoiceStatus,
    pub capture: VecDeque<f32>,
    pub speakers: HashMap<String, Speaker>,
    /// CLAUDE: peers whose WebRTC connection is up (status.connected is its size)
    pub connected: std::collections::HashSet<String>,
    /// CLAUDE: the caller wants capture; the worker opens/releases the input device to match
    pub microphone: bool,
}

// CODEX: all queue limits are in samples at the internal 48 kHz rate.
impl State {
    // CODEX: Create isolated service or mixer state without opening any network listener.
    pub fn new(config: VoiceConfig) -> Self {
        Self {
            pose: Pose::default(),
            status: VoiceStatus {
                state: "starting".into(),
                ..Default::default()
            },
            capture: VecDeque::new(),
            speakers: HashMap::new(),
            connected: Default::default(),
            microphone: config.microphone,
            config,
        }
    }
    // CODEX: Preserve an actionable asynchronous failure for the caller.
    pub fn error(&mut self, error: impl ToString) {
        self.status.state = "error".into();
        self.status.error = Some(error.to_string());
    }
    // CODEX: Bound decoded speech latency instead of accumulating stale samples.
    pub fn push_pcm(&mut self, id: &str, pcm: &[f32]) {
        let speaker = self.speakers.entry(id.into()).or_default();
        // CODEX: freshness prevents a disconnected speaker's last loud packet staying active forever.
        speaker.rms = (pcm.iter().map(|x| x * x).sum::<f32>() / pcm.len().max(1) as f32).sqrt();
        speaker.received_at = Some(std::time::Instant::now());
        speaker.pcm.extend(pcm.iter().copied());
        // CODEX: prefer current speech to an ever-growing backlog during CPU stalls.
        if speaker.pcm.len() > MAX_QUEUE {
            speaker.pcm.drain(..speaker.pcm.len() - MAX_QUEUE);
        }
    }
    // CODEX: Apply listener-relative panning and distance gains before measuring stereo output.
    pub fn mix(&mut self, stereo: &mut [f32], output_rate: u32) {
        stereo.fill(0.);
        let step = RATE as f64 / output_rate as f64;
        let smoothing = 1. - (-1. / (0.015 * output_rate as f32)).exp();
        for speaker in self.speakers.values_mut() {
            let target = speaker
                .position
                .map(|p| spatial::gains(self.pose, p, self.config.spatial))
                .unwrap_or([0., 0.]);
            // CODEX: a small initial prebuffer absorbs jitter; rebuffer after a full underrun.
            if !speaker.playing && speaker.pcm.len() >= FRAME * 2 {
                speaker.playing = true;
            }
            if !speaker.playing {
                continue;
            }
            for frame in stereo.as_chunks_mut::<2>().0 {
                if speaker.pcm.len() < 2 {
                    speaker.playing = false;
                    speaker.fraction = 0.;
                    break;
                }
                let mono =
                    speaker.pcm[0] + (speaker.pcm[1] - speaker.pcm[0]) * speaker.fraction as f32;
                for channel in 0..2 {
                    speaker.gain[channel] += (target[channel] - speaker.gain[channel]) * smoothing;
                    frame[channel] += mono * speaker.gain[channel];
                }
                speaker.fraction += step;
                let advance = speaker.fraction.floor() as usize;
                speaker.fraction -= advance as f64;
                let remove = advance.min(speaker.pcm.len());
                speaker.pcm.drain(..remove);
            }
        }
        let mut squares = [0.; 2];
        for frame in stereo.as_chunks_mut::<2>().0 {
            for channel in 0..2 {
                frame[channel] = frame[channel].clamp(-1., 1.);
                squares[channel] += frame[channel] * frame[channel];
            }
        }
        let n = (stereo.len() / 2).max(1) as f32;
        self.status.output_left = (squares[0] / n).sqrt();
        self.status.output_right = (squares[1] / n).sqrt();
    }
}

// CODEX: streaming linear interpolation retains phase across variable-size CPAL callbacks.
struct CaptureResampler {
    previous: f32,
    phase: f64,
    initialized: bool,
    step: f64,
}
// CODEX: convert hardware mono (downmixed from any channel count) to the fixed Opus clock.
impl CaptureResampler {
    // CODEX: Retain resampling phase across hardware callback boundaries.
    fn push(&mut self, input: impl Iterator<Item = f32>, queue: &mut VecDeque<f32>) {
        for sample in input {
            if !self.initialized {
                self.previous = sample;
                self.initialized = true;
                continue;
            }
            while self.phase < 1. {
                queue.push_back(self.previous + (sample - self.previous) * self.phase as f32);
                self.phase += self.step;
            }
            self.phase -= 1.;
            self.previous = sample;
        }
        if queue.len() > MAX_QUEUE {
            queue.drain(..queue.len() - MAX_QUEUE);
        }
    }
}

// CODEX: own streams on the voice worker thread; dropping them releases the physical devices.
pub(crate) struct Devices {
    _input: Option<cpal::Stream>,
    _output: Option<cpal::Stream>,
    /// CLAUDE: capture is live (a real input stream, or the test tone that replaces it)
    pub capturing: bool,
}
// CODEX: native probes can bypass either device independently without altering the media path.
impl Devices {
    // CODEX: Open physical devices only when the selected test configuration needs them.
    pub fn open(state: Arc<Mutex<State>>) -> anyhow::Result<Self> {
        let (microphone, headless, output_device) = {
            let s = state.lock().unwrap();
            (s.microphone, s.config.headless, s.config.output_device.clone())
        };
        let host = cpal::default_host();
        let output = if headless {
            None
        } else {
            let device = device::select(&host, output_device.as_deref(), Direction::Output)?;
            Some(device::open_stream(
                &device,
                Direction::Output,
                |format, config| match format {
                    cpal::SampleFormat::F32 => output_stream::<f32>(&device, config, state.clone()),
                    cpal::SampleFormat::I16 => output_stream::<i16>(&device, config, state.clone()),
                    cpal::SampleFormat::U16 => output_stream::<u16>(&device, config, state.clone()),
                    cpal::SampleFormat::I32 => output_stream::<i32>(&device, config, state.clone()),
                    cpal::SampleFormat::F64 => output_stream::<f64>(&device, config, state.clone()),
                    format => anyhow::bail!("Unsupported speaker sample format {format:?}"),
                },
            )?)
        };
        let mut devices = Self {
            _input: None,
            _output: output,
            capturing: false,
        };
        devices.set_input(&state, microphone)?;
        Ok(devices)
    }

    // CLAUDE: open or release the microphone while playback (and the connection) continues
    pub fn set_input(&mut self, state: &Arc<Mutex<State>>, on: bool) -> anyhow::Result<()> {
        if on == self.capturing {
            return Ok(());
        }
        let (tone, input_device) = {
            let s = state.lock().unwrap();
            (s.config.test_tone_hz.is_some(), s.config.input_device.clone())
        };
        self._input = None;
        state.lock().unwrap().capture.clear();
        if on && !tone {
            let host = cpal::default_host();
            let device = device::select(&host, input_device.as_deref(), Direction::Input)?;
            self._input = Some(device::open_stream(
                &device,
                Direction::Input,
                |format, config| match format {
                    cpal::SampleFormat::F32 => input_stream::<f32>(&device, config, state.clone()),
                    cpal::SampleFormat::I16 => input_stream::<i16>(&device, config, state.clone()),
                    cpal::SampleFormat::U16 => input_stream::<u16>(&device, config, state.clone()),
                    cpal::SampleFormat::I32 => input_stream::<i32>(&device, config, state.clone()),
                    cpal::SampleFormat::F64 => input_stream::<f64>(&device, config, state.clone()),
                    format => anyhow::bail!("Unsupported microphone sample format {format:?}"),
                },
            )?);
        }
        self.capturing = on;
        state.lock().unwrap().status.microphone = on;
        Ok(())
    }
}

// CODEX: capture callbacks only downmix/resample/enqueue; Opus and networking run outside callbacks.
fn input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<State>>,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let mut resampler = CaptureResampler {
        previous: 0.,
        phase: 0.,
        initialized: false,
        step: config.sample_rate as f64 / RATE as f64,
    };
    let errors = state.clone();
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            if let Ok(mut state) = state.lock() {
                resampler.push(
                    data.chunks_exact(channels).map(|frame| {
                        frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32
                    }),
                    &mut state.capture,
                );
            }
        },
        // CLAUDE: a lost microphone stops capture only; the worker releases the stream, receiving continues
        move |e| {
            if let Ok(mut s) = errors.lock() {
                s.microphone = false;
                s.status.microphone_error = Some(e.to_string());
            }
        },
        None,
    )?)
}

// CODEX: render a buffer under one lock; only L/R channels carry positional voice on surround devices.
fn output_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    state: Arc<Mutex<State>>,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let rate = config.sample_rate;
    let errors = state.clone();
    let mut stereo = Vec::new();
    Ok(device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            stereo.resize(data.len() / channels * 2, 0.);
            if let Ok(mut state) = state.lock() {
                state.mix(&mut stereo, rate);
            } else {
                stereo.fill(0.);
            }
            for (out, pair) in data
                .chunks_exact_mut(channels)
                .zip(stereo.as_chunks::<2>().0)
            {
                out.fill(T::from_sample(0.));
                out[0] = T::from_sample(pair[0]);
                out[1] = T::from_sample(pair[1]);
            }
        },
        move |e| {
            if let Ok(mut s) = errors.lock() {
                s.error(e);
            }
        },
        None,
    )?)
}

#[cfg(test)]
// CODEX: verify both common hardware rates produce a stable 48 kHz stream without callback-boundary drift.
mod tests {
    use super::*;
    #[test]
    #[ignore = "opens the real default microphone and stereo output; run explicitly on an audio host"]
    // CODEX: exercise device initialization without signaling, recording files, or playing any audio.
    fn opens_default_devices() {
        let state = Arc::new(Mutex::new(State::new(VoiceConfig {
            input_device: std::env::var("VOICE_TEST_INPUT_DEVICE").ok(),
            output_device: std::env::var("VOICE_TEST_OUTPUT_DEVICE").ok(),
            ..Default::default()
        })));
        let _devices =
            Devices::open(state.clone()).expect("default capture and stereo output must start");
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(state.lock().unwrap().status.error.is_none());
    }
    #[test]
    // CODEX: Check hardware-rate conversion over many callback boundaries.
    fn resampling_preserves_duration() {
        for rate in [44_100, 48_000, 96_000] {
            let mut r = CaptureResampler {
                previous: 0.,
                phase: 0.,
                initialized: false,
                step: rate as f64 / RATE as f64,
            };
            let mut q = VecDeque::new();
            let mut total = 0;
            for _ in 0..100 {
                r.push(std::iter::repeat_n(0.5, rate / 100), &mut q);
                total += q.len();
                q.clear();
            }
            assert!(
                (total as isize - RATE as isize).abs() <= 3,
                "{rate}: {total}"
            );
        }
    }
}
