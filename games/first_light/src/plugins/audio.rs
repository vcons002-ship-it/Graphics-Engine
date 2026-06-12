//! Procedural audio: every sound is synthesized into an in-memory WAV at
//! startup — no asset files, in keeping with the "generated, not
//! hand-made" pipeline. Heavy stone impacts are a 38 Hz sub thump plus
//! low-passed rubble noise and crackle; the trebuchet creaks while
//! cranking, whooshes on release, and thunks at the arm stop; footsteps
//! tap softly on grass; a low wind loop fills the valley.
//!
//! Other plugins request playback by writing [`SoundEvent`]s; spatial
//! sounds attenuate from their world position relative to the camera's
//! [`SpatialListener`].

use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, SpatialListener, Volume};
use bevy::prelude::*;

use engine::player::Grounded;
use engine::prelude::*;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SoundEvent>()
            .add_systems(Startup, setup_sound_bank)
            .add_systems(Update, (attach_listener, play_sounds, footsteps));
    }
}

#[derive(Clone, Copy)]
pub enum SoundKind {
    /// Multi-tonne stone on stone. Intensity scales volume and pitch pick.
    StoneImpact,
    /// A block shattering.
    RockCrack,
    /// Trebuchet release.
    Whoosh,
    /// Trebuchet arm hitting the padded stop.
    FrameThunk,
    /// Winch creak while cranking.
    Creak,
}

#[derive(Message)]
pub struct SoundEvent {
    pub kind: SoundKind,
    pub position: Vec3,
    /// 0..1, scales volume.
    pub intensity: f32,
}

#[derive(Resource)]
struct SoundBank {
    impacts: Vec<Handle<AudioSource>>,
    cracks: Vec<Handle<AudioSource>>,
    whoosh: Handle<AudioSource>,
    thunk: Handle<AudioSource>,
    creak: Handle<AudioSource>,
    steps: Vec<Handle<AudioSource>>,
}

const SAMPLE_RATE: u32 = 22_050;

/// Tiny deterministic generator for noise and variation.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Packs mono f32 samples into an in-memory 16-bit WAV.
fn wav(samples: &[f32]) -> AudioSource {
    let n = samples.len() as u32;
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + n * 2).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(n * 2).to_le_bytes());
    for s in samples {
        bytes.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    AudioSource {
        bytes: bytes.into(),
    }
}

/// One-pole low-pass coefficient for a cutoff in Hz.
fn lp_a(cutoff: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * cutoff / SAMPLE_RATE as f32).exp()
}

/// Heavy stone-on-stone: 38/55 Hz sub thump + low-passed rubble noise +
/// crackle transients, soft-clipped for weight.
fn synth_impact(seed: u64) -> AudioSource {
    let mut rng = Lcg(seed);
    let len = (SAMPLE_RATE as f32 * 1.1) as usize;
    let mut out = vec![0.0f32; len];
    let mut lp = 0.0f32;
    let a = lp_a(280.0);
    // Crackle onsets in the first 150 ms.
    let crackles: Vec<(usize, f32)> = (0..6)
        .map(|_| {
            (
                (rng.next().abs() * 0.15 * SAMPLE_RATE as f32) as usize,
                rng.next() * 0.5,
            )
        })
        .collect();
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        let sub = (std::f32::consts::TAU * 38.0 * t).sin() * (-4.5 * t).exp() * 0.95
            + (std::f32::consts::TAU * 55.0 * t).sin() * (-7.0 * t).exp() * 0.5;
        lp += a * (rng.next() - lp);
        let rumble = lp * (-9.0 * t).exp() * 2.2;
        let mut crackle = 0.0;
        for &(start, amp) in &crackles {
            if i >= start {
                let ct = (i - start) as f32 / SAMPLE_RATE as f32;
                if ct < 0.03 {
                    crackle += rng.next() * amp * (-160.0 * ct).exp();
                }
            }
        }
        *sample = ((sub + rumble + crackle) * 1.15).tanh();
    }
    wav(&out)
}

/// A single block shattering: brighter noise burst + clicks.
fn synth_crack(seed: u64) -> AudioSource {
    let mut rng = Lcg(seed);
    let len = (SAMPLE_RATE as f32 * 0.4) as usize;
    let mut out = vec![0.0f32; len];
    let mut lp = 0.0f32;
    let a = lp_a(900.0);
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        lp += a * (rng.next() - lp);
        let body = lp * (-16.0 * t).exp() * 1.8;
        let thump = (std::f32::consts::TAU * 70.0 * t).sin() * (-20.0 * t).exp() * 0.4;
        *sample = (body + thump).tanh() * 0.9;
    }
    wav(&out)
}

/// Trebuchet release: noise through a swept low-pass, swelling and dying.
fn synth_whoosh() -> AudioSource {
    let mut rng = Lcg(99);
    let len = (SAMPLE_RATE as f32 * 0.8) as usize;
    let mut out = vec![0.0f32; len];
    let mut lp = 0.0f32;
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / len as f32;
        let cutoff = 150.0 + 1400.0 * (std::f32::consts::PI * t).sin();
        let a = lp_a(cutoff);
        lp += a * (rng.next() - lp);
        *sample = lp * (std::f32::consts::PI * t).sin() * 1.6;
    }
    wav(&out)
}

/// The beam slamming the padded stop.
fn synth_thunk() -> AudioSource {
    let mut rng = Lcg(7);
    let len = (SAMPLE_RATE as f32 * 0.3) as usize;
    let mut out = vec![0.0f32; len];
    let mut lp = 0.0f32;
    let a = lp_a(180.0);
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        lp += a * (rng.next() - lp);
        let body = (std::f32::consts::TAU * 65.0 * t).sin() * (-15.0 * t).exp() * 0.8;
        *sample = (body + lp * (-18.0 * t).exp() * 1.4).tanh();
    }
    wav(&out)
}

/// Wood-on-rope winch creak: stick-slip square wave with wandering pitch.
fn synth_creak() -> AudioSource {
    let mut rng = Lcg(13);
    let len = (SAMPLE_RATE as f32 * 0.16) as usize;
    let mut out = vec![0.0f32; len];
    let mut phase = 0.0f32;
    let mut freq = 95.0f32;
    let mut lp = 0.0f32;
    let a = lp_a(700.0);
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        freq += rng.next() * 3.0;
        phase += freq / SAMPLE_RATE as f32;
        let square = if phase.fract() < 0.5 { 1.0 } else { -1.0 };
        lp += a * (square - lp);
        *sample = lp * (-12.0 * t).exp() * 0.35;
    }
    wav(&out)
}

/// A soft grass footstep.
fn synth_step(seed: u64) -> AudioSource {
    let mut rng = Lcg(seed);
    let len = (SAMPLE_RATE as f32 * 0.11) as usize;
    let mut out = vec![0.0f32; len];
    let mut lp = 0.0f32;
    let a = lp_a(420.0);
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        lp += a * (rng.next() - lp);
        let thump = (std::f32::consts::TAU * 72.0 * t).sin() * (-55.0 * t).exp() * 0.25;
        *sample = lp * (-40.0 * t).exp() * 1.0 + thump;
    }
    wav(&out)
}

/// An 8-second seamless wind loop (overlap-faded ends).
fn synth_wind() -> AudioSource {
    let mut rng = Lcg(4242);
    let len = SAMPLE_RATE as usize * 8;
    let mut out = vec![0.0f32; len];
    let mut lp = 0.0f32;
    let a = lp_a(240.0);
    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        lp += a * (rng.next() - lp);
        let gust = 0.55 + 0.45 * (std::f32::consts::TAU * 0.11 * t).sin().powi(2);
        *sample = lp * gust * 0.8;
    }
    // Crossfade the loop seam.
    let fade = SAMPLE_RATE as usize / 2;
    for k in 0..fade {
        let w = k as f32 / fade as f32;
        out[k] = out[k] * w + out[len - fade + k] * (1.0 - w);
    }
    out.truncate(len - fade);
    wav(&out)
}

fn setup_sound_bank(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let bank = SoundBank {
        impacts: (0..3).map(|k| sources.add(synth_impact(100 + k))).collect(),
        cracks: (0..3).map(|k| sources.add(synth_crack(200 + k))).collect(),
        whoosh: sources.add(synth_whoosh()),
        thunk: sources.add(synth_thunk()),
        creak: sources.add(synth_creak()),
        steps: (0..4).map(|k| sources.add(synth_step(300 + k))).collect(),
    };
    // Valley wind, non-spatial, quiet, forever.
    commands.spawn((
        AudioPlayer(sources.add(synth_wind())),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.16)),
    ));
    commands.insert_resource(bank);
}

/// The main camera is the ears.
fn attach_listener(
    mut commands: Commands,
    cameras: Query<Entity, (With<MainCamera>, Without<SpatialListener>)>,
) {
    for camera in &cameras {
        commands.entity(camera).insert(SpatialListener::default());
    }
}

/// Plays queued sound events as one-shot spatial sounds (bounded per frame).
fn play_sounds(
    mut commands: Commands,
    mut events: MessageReader<SoundEvent>,
    bank: Res<SoundBank>,
    mut salt: Local<u64>,
) {
    let mut budget = 8;
    for event in events.read() {
        if budget == 0 {
            break;
        }
        budget -= 1;
        *salt = salt.wrapping_add(1);
        let pick = |list: &Vec<Handle<AudioSource>>| list[(*salt as usize) % list.len()].clone();
        let (source, volume) = match event.kind {
            SoundKind::StoneImpact => (pick(&bank.impacts), 1.6 * event.intensity),
            SoundKind::RockCrack => (pick(&bank.cracks), 0.9 * event.intensity),
            SoundKind::Whoosh => (bank.whoosh.clone(), 1.0 * event.intensity),
            SoundKind::FrameThunk => (bank.thunk.clone(), 1.1 * event.intensity),
            SoundKind::Creak => (bank.creak.clone(), 0.7 * event.intensity),
        };
        commands.spawn((
            AudioPlayer(source),
            PlaybackSettings::DESPAWN
                .with_spatial(true)
                .with_volume(Volume::Linear(volume.clamp(0.05, 2.0))),
            Transform::from_translation(event.position),
        ));
    }
}

/// Footsteps from the player's own movement: grounded + moving.
fn footsteps(
    mut commands: Commands,
    time: Res<Time>,
    bank: Res<SoundBank>,
    players: Query<(&avian3d::prelude::LinearVelocity, Has<Grounded>), With<Player>>,
    mut accumulator: Local<f32>,
    mut salt: Local<u64>,
) {
    let Ok((velocity, grounded)) = players.single() else {
        return;
    };
    let speed = Vec2::new(velocity.x, velocity.z).length();
    if !grounded || speed < 1.5 {
        *accumulator = 0.3; // first step lands quickly after moving again
        return;
    }
    *accumulator += time.delta_secs();
    // Cadence tracks speed: ~0.5 s walking, ~0.32 s sprinting.
    let interval = (2.4 / speed).clamp(0.3, 0.55);
    if *accumulator >= interval {
        *accumulator = 0.0;
        *salt = salt.wrapping_add(1);
        commands.spawn((
            AudioPlayer(bank.steps[(*salt as usize) % bank.steps.len()].clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(0.5)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_consistent() {
        let source = wav(&[0.0, 0.5, -0.5, 1.0]);
        assert_eq!(source.bytes.len(), 44 + 4 * 2);
        assert_eq!(&source.bytes[0..4], b"RIFF");
        assert_eq!(&source.bytes[8..12], b"WAVE");
    }
}
