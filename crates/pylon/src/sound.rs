//! CLAUDE (claude-opus-5-5): 2026-11-03 sounds that start at a set time, to the sample: [`PlayAt`].
//!
//! rodio starts a new sound with the next block of audio it hands the device (by default ~50ms of it), so a
//! sound spawned on a frame starts up to a block late, and a rhythm of them (the chase's chirps) stumbles.
//! A [`PlayAt`] sound names its moment instead, and pads itself with silence up to it, measured when the
//! audio thread first reads it (as it fills a block): it starts at that moment plus the output's fixed
//! latency, wherever the blocks fall.

use std::time::Duration;

use bevy::{
    audio::{AddAudioSource, AudioSink, ChannelCount, Decodable, Sample, SampleRate, Source, SpatialAudioSink},
    platform::time::Instant,
    prelude::*,
};

pub fn plugin(app: &mut App) {
    app.register_type::<PlayAt>()
        .add_audio_source::<TimedSound>()
        .add_observer(|remove: On<Remove<PlayAt>>, mut commands: Commands| {
            if let Ok(mut e) = commands.get_entity(remove.entity) {
                e.try_remove::<(AudioPlayer<TimedSound>, AudioSink, SpatialAudioSink, Waiting)>();
            }
        })
        .add_systems(PostUpdate, schedule);
}

/// play `sound` at `at`, with the entity's [`PlaybackSettings`]: seconds on the frame clock
/// (`Time::elapsed_secs_wrapped`, scripts' `now()`), at least an audio block ahead (~50ms, give it more)
/// or it starts late. Inserting it again replaces what it was playing or waiting to; removing it stops it
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(PlaybackSettings)]
pub struct PlayAt {
    pub sound: Handle<AudioSource>,
    pub at: f32,
}

/// a [`PlayAt`] whose sound hasn't loaded yet
#[derive(Component)]
struct Waiting;

/// a sound and the moment it starts
#[derive(Asset, TypePath)]
pub struct TimedSound {
    sound: AudioSource,
    start: Instant,
}

impl Decodable for TimedSound {
    type Decoder = Delayed;

    fn decoder(&self) -> Delayed {
        Delayed { inner: self.sound.decoder(), start: self.start, silence: None }
    }
}

/// the sound, after silence up to its start: counted on its first sample, on the audio thread
pub struct Delayed {
    inner: <AudioSource as Decodable>::Decoder,
    start: Instant,
    silence: Option<usize>,
}

impl Iterator for Delayed {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        let (rate, channels) = (self.inner.sample_rate().get(), self.inner.channels().get());
        let start = self.start;
        // whole frames, so the channels stay in step
        let silence = self.silence.get_or_insert_with(|| {
            let wait = start.saturating_duration_since(Instant::now());
            (wait.as_secs_f64() * rate as f64) as usize * channels as usize
        });
        if *silence > 0 {
            *silence -= 1;
            return Some(0.0);
        }
        self.inner.next()
    }
}

impl Source for Delayed {
    // (before the first sample, the silence isn't counted yet: right for sounds of one span, as wav is)
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len().map(|n| n + self.silence.unwrap_or(0))
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// a new or changed [`PlayAt`] (or one whose sound has since loaded) replaces what its entity was playing
/// with its sound at its moment. The frame clock's now is this frame's real-clock update
fn schedule(
    plays: Query<(Entity, Ref<PlayAt>, Has<Waiting>)>,
    sources: Res<Assets<AudioSource>>,
    mut sounds: ResMut<Assets<TimedSound>>,
    time: Res<Time>,
    real: Res<Time<Real>>,
    mut commands: Commands,
) {
    let now = real.last_update().unwrap_or_else(Instant::now);
    for (e, play, waiting) in &plays {
        if !play.is_changed() && !waiting {
            continue;
        }
        let mut entity = commands.entity(e);
        entity.remove::<(AudioPlayer<TimedSound>, AudioSink, SpatialAudioSink)>();
        let Some(sound) = sources.get(&play.sound) else {
            entity.insert(Waiting);
            continue;
        };
        let ahead = Duration::from_secs_f32((play.at - time.elapsed_secs_wrapped()).max(0.0));
        entity.remove::<Waiting>().insert(AudioPlayer(sounds.add(TimedSound { sound: sound.clone(), start: now + ahead })));
    }
}
