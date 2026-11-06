//! CODEX (GPT-6): conservative native stream negotiation; ALSA virtual devices advertise very broad ranges.
use anyhow::Context;
use cpal::{
    BufferSize,
    Device,
    SampleFormat,
    Stream,
    StreamConfig,
    SupportedBufferSize,
    SupportedStreamConfig,
    SupportedStreamConfigRange,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

// CODEX: output needs at least stereo; capture can downmix any supported channel count.
#[derive(Clone, Copy)]
pub(crate) enum Direction {
    Input,
    Output,
}

// CODEX: route only this client; explicit ALSA/PipeWire targets must never mutate system default devices.
pub(crate) fn select(
    host: &cpal::Host,
    requested: Option<&str>,
    direction: Direction,
) -> anyhow::Result<Device> {
    let Some(requested) = requested else {
        return match direction {
            Direction::Input => host.default_input_device(),
            Direction::Output => host.default_output_device(),
        }
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No default {} device",
                match direction {
                    Direction::Input => "microphone",
                    Direction::Output => "speaker",
                }
            )
        });
    };
    anyhow::ensure!(
        !requested.is_empty() && !requested.contains('\0'),
        "Audio device ID must be nonempty and contain no NUL characters"
    );
    #[cfg(target_os = "linux")]
    if host.id() == cpal::HostId::Alsa {
        // CODEX: CPAL 0.17.1 exposes this public conversion through its ALSA host's associated device type.
        // This allows PCM arguments such as pipewire:NODE=... that are absent from enumerated ALSA hints.
        type AlsaDevice = <cpal::platform::AlsaHost as HostTrait>::Device;
        return Ok(AlsaDevice::try_from(alsa::device_name::Hint {
            name: Some(requested.strip_prefix("alsa:").unwrap_or(requested).into()),
            desc: Some(requested.into()),
            direction: Some(match direction {
                Direction::Input => alsa::Direction::Capture,
                Direction::Output => alsa::Direction::Playback,
            }),
        })?
        .into());
    }
    host.devices()?
        .find(|device| {
            device.id().is_ok_and(|id| id.to_string() == requested)
                || device.description().is_ok_and(|d| d.name() == requested)
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Audio device {requested:?} was not found; use voice-probe --list-devices"
            )
        })
}

// CODEX: rank only formats the audio callbacks can convert; never select an unsupported format by accident.
fn format_rank(format: SampleFormat) -> Option<u8> {
    match format {
        SampleFormat::F32 => Some(0),
        SampleFormat::I16 => Some(1),
        SampleFormat::I32 => Some(2),
        SampleFormat::U16 => Some(3),
        SampleFormat::F64 => Some(4),
        _ => None,
    }
}

// CODEX: prefer the smallest useful layout, not the highest advertised sample rate or channel count.
fn candidates(
    default: Option<SupportedStreamConfig>,
    ranges: Vec<SupportedStreamConfigRange>,
    direction: Direction,
) -> Vec<SupportedStreamConfig> {
    let minimum = match direction {
        Direction::Input => 1,
        Direction::Output => 2,
    };
    let mut ranges = ranges
        .into_iter()
        .filter(|c| c.channels() >= minimum && format_rank(c.sample_format()).is_some())
        .collect::<Vec<_>>();
    let channels = ranges
        .iter()
        .map(|c| c.channels())
        .chain(
            default
                .iter()
                .filter(|c| c.channels() >= minimum && format_rank(c.sample_format()).is_some())
                .map(|c| c.channels()),
        )
        .min();
    let mut configs = Vec::new();
    // CODEX: OS defaults are usually already negotiated with the sound server. Retain them when usable.
    if let Some(default) = default {
        if format_rank(default.sample_format()).is_some()
            && default.channels() >= minimum
            && (matches!(direction, Direction::Input) || Some(default.channels()) == channels)
        {
            configs.push(default);
        }
    }
    ranges.retain(|c| Some(c.channels()) == channels);
    ranges.sort_by_key(|c| format_rank(c.sample_format()).unwrap());
    for range in ranges {
        for rate in [48_000, 44_100, range.min_sample_rate().max(8_000)] {
            if let Some(config) = range.try_with_sample_rate(rate) {
                if !configs.iter().any(|c| {
                    c.channels() == config.channels()
                        && c.sample_rate() == rate
                        && c.sample_format() == config.sample_format()
                }) {
                    configs.push(config);
                }
            }
        }
    }
    configs
}

// CODEX: CPAL 0.17's ALSA default-buffer path reapplies hw_params; retry with an explicit 10 ms period.
fn buffer_options(config: &SupportedStreamConfig) -> [StreamConfig; 2] {
    let default = config.config();
    let mut fixed = default.clone();
    let frames = (fixed.sample_rate / 100).max(1);
    let frames = match *config.buffer_size() {
        SupportedBufferSize::Range { min, max } => frames.clamp(min.max(1), max.max(min).max(1)),
        SupportedBufferSize::Unknown => frames,
    };
    fixed.buffer_size = BufferSize::Fixed(frames);
    [default, fixed]
}

// CODEX: keep retry policy independently testable; drop a failed stream before attempting the next config.
fn try_configs<T>(
    configs: &[SupportedStreamConfig],
    mut build: impl FnMut(&SupportedStreamConfig, &StreamConfig) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let mut errors = Vec::new();
    for supported in configs {
        for config in buffer_options(supported) {
            match build(supported, &config) {
                Ok(stream) => return Ok(stream),
                Err(error) => errors.push(format!(
                    "{} channels, {} Hz, {:?}, {:?}: {error:#}",
                    config.channels,
                    config.sample_rate,
                    supported.sample_format(),
                    config.buffer_size
                )),
            }
        }
    }
    if errors.is_empty() {
        anyhow::bail!("No supported channel layout/sample format (stereo output is required)");
    }
    anyhow::bail!(
        "All supported stream configurations failed:\n{}",
        errors.join("\n")
    )
}

// CODEX: device identity and stream direction survive the async worker's error flattening.
pub(crate) fn open_stream(
    device: &Device,
    direction: Direction,
    mut build: impl FnMut(SampleFormat, &StreamConfig) -> anyhow::Result<Stream>,
) -> anyhow::Result<Stream> {
    let name = device
        .description()
        .map(|d| d.to_string())
        .unwrap_or_else(|_| "unnamed device".into());
    let label = match direction {
        Direction::Input => "microphone",
        Direction::Output => "speaker",
    };
    let (default, ranges) = match direction {
        Direction::Input => (
            device.default_input_config(),
            device
                .supported_input_configs()
                .map(|r| r.collect::<Vec<_>>()),
        ),
        Direction::Output => (
            device.default_output_config(),
            device
                .supported_output_configs()
                .map(|r| r.collect::<Vec<_>>()),
        ),
    };
    let mut discovery = Vec::new();
    if let Err(error) = &default {
        discovery.push(format!("default config: {error}"));
    }
    if let Err(error) = &ranges {
        discovery.push(format!("supported configs: {error}"));
    }
    let configs = candidates(default.ok(), ranges.unwrap_or_default(), direction);
    try_configs(&configs, |supported, config| {
        let stream = build(supported.sample_format(), config).context("creating stream")?;
        stream.play().context("starting stream")?;
        Ok(stream)
    })
    .with_context(|| {
        format!(
            "Could not initialize {label} device {name:?}{}",
            if discovery.is_empty() {
                String::new()
            } else {
                format!(" ({})", discovery.join("; "))
            }
        )
    })
}

#[cfg(test)]
// CODEX: regressions cover ALSA's 32-channel advertisement and failure after a successful capability query.
mod tests {
    use super::*;
    // CODEX: model the broad range advertised by PipeWire's ALSA plugin without needing hardware.
    fn range(channels: u16) -> SupportedStreamConfigRange {
        SupportedStreamConfigRange::new(
            channels,
            8_000,
            192_000,
            SupportedBufferSize::Unknown,
            SampleFormat::F32,
        )
    }
    #[cfg(target_os = "linux")]
    #[test]
    // CODEX: PCM arguments must survive selection intact; constructing the handle opens no audio stream.
    fn accepts_explicit_pipewire_target_without_changing_defaults() {
        let host = cpal::default_host();
        let device = select(
            &host,
            Some("alsa:pipewire:NODE=voice-test-sink"),
            Direction::Output,
        )
        .unwrap();
        assert_eq!(
            device.id().unwrap().to_string(),
            "alsa:pipewire:NODE=voice-test-sink"
        );
        assert!(select(&host, Some("bad\0device"), Direction::Output).is_err());
    }
    #[test]
    // CODEX: the original max_by_key selected the last equally ranked range, often 32 channels.
    fn stereo_wins_over_32_channel_virtual_device() {
        let configs = candidates(None, vec![range(2), range(32)], Direction::Output);
        assert!(configs.iter().all(|c| c.channels() == 2));
        assert_eq!(configs[0].sample_rate(), 48_000);
        assert!(candidates(None, vec![range(1)], Direction::Output).is_empty());
        assert_eq!(
            candidates(None, vec![range(6), range(8)], Direction::Output)[0].channels(),
            6
        );
    }
    #[test]
    // CODEX: preserve a usable OS-selected rate; output resampling already handles 44.1 kHz.
    fn preserves_default_stereo_config() {
        let default =
            SupportedStreamConfig::new(2, 44_100, SupportedBufferSize::Unknown, SampleFormat::I16);
        let configs = candidates(Some(default), vec![range(2), range(32)], Direction::Output);
        assert_eq!(configs[0].sample_rate(), 44_100);
        assert_eq!(configs[0].sample_format(), SampleFormat::I16);
    }
    #[test]
    // CODEX: a queried/default config is not proof it can be built; retry with explicit buffer timing.
    fn retries_fixed_buffer_and_reports_attempts() {
        let config = range(2).with_sample_rate(48_000);
        let mut attempts = 0;
        let result = try_configs(std::slice::from_ref(&config), |_, c| {
            attempts += 1;
            if c.buffer_size == BufferSize::Default {
                anyhow::bail!("snd_pcm_hw_params failed");
            }
            Ok(c.buffer_size)
        })
        .unwrap();
        assert_eq!(attempts, 2);
        assert_eq!(result, BufferSize::Fixed(480));
        let failure = try_configs::<()>(&[config], |_, _| anyhow::bail!("device offline"))
            .unwrap_err()
            .to_string();
        assert!(failure.contains("2 channels, 48000 Hz"));
        assert!(failure.contains("Fixed(480)"));
        assert!(failure.contains("device offline"));
    }
}
