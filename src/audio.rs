//! Sound, all synthesised here; Saalis ships no audio files (Kaiku's synth).
//!
//! Each cue is written as a short row of steps (a pitch, a waveform and a
//! volume) played at a fixed speed, in the manner of Ceres's effects. The
//! bank is rendered once at start. Without an audio device, without the
//! `sound` feature, or muted (`--mute`, F8), every call does nothing.

use crate::sim::cue::Cue;

#[cfg(feature = "sound")]
mod synth {
    use super::Cue;

    pub const SR: u32 = 44_100;
    const SRF: f32 = SR as f32;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Wave {
        Tri,
        Square,
        Pulse,
        Saw,
        Noise,
    }
    use Wave::*;

    /// One step: MIDI pitch (fractions slide), waveform, volume 0-1.
    type Step = (f32, Wave, f32);

    /// Steps from pitch `a` to `b` in `n` even moves, volume `v0` to `v1`.
    fn slide(a: f32, b: f32, n: usize, w: Wave, v0: f32, v1: f32) -> Vec<Step> {
        (0..n)
            .map(|i| {
                let t = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
                (a + (b - a) * t, w, v0 + (v1 - v0) * t)
            })
            .collect()
    }

    fn notes(ps: &[f32], w: Wave, v: f32, each: usize) -> Vec<Step> {
        ps.iter().flat_map(|&p| std::iter::repeat_n((p, w, v), each)).collect()
    }

    fn fade(mut s: Vec<Step>, from: f32) -> Vec<Step> {
        let n = s.len() as f32;
        for (i, st) in s.iter_mut().enumerate() {
            let t = i as f32 / n;
            if t > from {
                st.2 *= 1.0 - (t - from) / (1.0 - from);
            }
        }
        s
    }

    /// A cue's sound: step length in seconds, and layers played together.
    fn score(cue: Cue) -> (f32, Vec<Vec<Step>>) {
        match cue {
            // the laser pair: a soft zap, it plays a lot
            Cue::Gun => (0.008, vec![slide(86.0, 70.0, 4, Pulse, 0.2, 0.0)]),
            Cue::EnemyGun => (0.009, vec![slide(74.0, 58.0, 4, Square, 0.12, 0.0)]),
            Cue::ShieldHit => (0.012, vec![slide(64.0, 52.0, 6, Tri, 0.45, 0.05), slide(80.0, 60.0, 4, Noise, 0.25, 0.0)]),
            Cue::ArmourHit => (0.014, vec![slide(46.0, 36.0, 7, Square, 0.5, 0.05), slide(66.0, 44.0, 7, Noise, 0.55, 0.0)]),
            Cue::HitConfirm => (0.006, vec![vec![(96.0, Tri, 0.12), (93.0, Tri, 0.05)]]),
            Cue::Kill => (0.012, vec![slide(62.0, 34.0, 10, Noise, 0.6, 0.0), slide(68.0, 46.0, 7, Square, 0.25, 0.0)]),
            Cue::BigKill => (0.016, vec![slide(55.0, 24.0, 22, Noise, 0.85, 0.0), slide(50.0, 28.0, 16, Saw, 0.4, 0.0)]),
            Cue::PlayerLost => (0.02, vec![slide(60.0, 22.0, 40, Noise, 0.9, 0.0), slide(58.0, 26.0, 34, Saw, 0.5, 0.0)]),
            Cue::Burner => (0.02, vec![slide(36.0, 52.0, 14, Noise, 0.25, 0.4)]),
            Cue::Target => (0.03, vec![notes(&[84.0, 88.0], Tri, 0.25, 1)]),
            Cue::Comms => (0.03, vec![fade(notes(&[79.0, 79.0, 86.0], Pulse, 0.12, 1), 0.6)]),
            Cue::Contact => (0.08, vec![fade(notes(&[67.0, 61.0, 67.0, 61.0], Square, 0.25, 1), 0.7)]),
            Cue::Empty => (0.01, vec![vec![(40.0, Pulse, 0.15), (40.0, Pulse, 0.0)]]),
            Cue::Missile => (0.02, vec![slide(40.0, 70.0, 16, Noise, 0.5, 0.1), slide(50.0, 62.0, 12, Saw, 0.2, 0.0)]),
            Cue::Warning => (0.06, vec![notes(&[88.0, 0.0, 88.0, 0.0], Square, 0.2, 1)]),
        }
    }

    pub const ALL: [Cue; 15] = [
        Cue::Missile,
        Cue::Warning,
        Cue::Gun,
        Cue::EnemyGun,
        Cue::ShieldHit,
        Cue::ArmourHit,
        Cue::HitConfirm,
        Cue::Kill,
        Cue::BigKill,
        Cue::PlayerLost,
        Cue::Burner,
        Cue::Target,
        Cue::Comms,
        Cue::Contact,
        Cue::Empty,
    ];

    /// A cue's samples, mono, at `SR`.
    pub fn render(cue: Cue) -> Vec<f32> {
        let (len, layers) = score(cue);
        let per = (len * SRF) as usize;
        let mut out = vec![0.0f32; per * layers.iter().map(Vec::len).max().unwrap_or(0)];
        for steps in &layers {
            let (mut ph, mut noise, mut lp, mut hold, mut prev_vol) = (0.0f32, 0x1234_5678u32, 0.0f32, 0.0f32, 0.0f32);
            for (k, &(pitch, w, vol)) in steps.iter().enumerate() {
                let next = steps.get(k + 1).map_or(pitch, |s| s.0);
                for s in 0..per {
                    let t = s as f32 / per as f32;
                    let p = pitch + (next - pitch) * t * 0.5;
                    let f = 440.0 * 2f32.powf((p - 69.0) / 12.0);
                    ph = (ph + f / SRF).fract();
                    // ramp volume changes over 64 samples against clicks
                    let v = prev_vol + (vol - prev_vol) * (s as f32 / 64.0).min(1.0);
                    let x = match w {
                        Tri => 1.0 - 4.0 * (ph - 0.5).abs(),
                        Square => 0.6 * if ph < 0.5 { 1.0 } else { -1.0 },
                        Pulse => 0.6 * if ph < 0.25 { 1.0 } else { -1.0 },
                        Saw => (ph * 2.0 - 1.0) * 0.6,
                        Noise => {
                            // sample-and-hold noise at the pitch rate, softened
                            if ph < f / SRF {
                                noise ^= noise << 13;
                                noise ^= noise >> 17;
                                noise ^= noise << 5;
                                hold = (noise as f32 / u32::MAX as f32) * 2.0 - 1.0;
                            }
                            lp += (hold - lp) * 0.5;
                            lp
                        }
                    };
                    out[k * per + s] += x * v;
                }
                prev_vol = vol;
            }
        }
        // short fade out against clicks
        let n = out.len();
        for (i, s) in out.iter_mut().enumerate().skip(n.saturating_sub(200)) {
            *s *= (n - i) as f32 / 200.0;
        }
        out
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn every_cue_renders_to_a_short_clean_sound() {
            for cue in ALL {
                let s = render(cue);
                assert!(!s.is_empty(), "{cue:?}");
                assert!(s.len() < SR as usize * 3, "{cue:?} is under three seconds");
                assert!(s.iter().all(|x| x.is_finite() && x.abs() < 2.0), "{cue:?} stays in range");
            }
        }
    }
}

pub struct Audio {
    #[cfg(feature = "sound")]
    sink: Option<rodio::MixerDeviceSink>,
    #[cfg(feature = "sound")]
    bank: std::collections::HashMap<Cue, rodio::buffer::SamplesBuffer>,
    /// when each rapid cue last sounded: guns fire faster than they should be heard
    #[cfg(feature = "sound")]
    last: std::collections::HashMap<Cue, std::time::Instant>,
    pub muted: bool,
}

impl Audio {
    pub fn new(muted: bool) -> Audio {
        #[cfg(feature = "sound")]
        {
            use std::num::NonZero;
            let sink = match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut s) => {
                    s.log_on_drop(false);
                    Some(s)
                }
                Err(e) => {
                    log::warn!("no audio: {e}");
                    None
                }
            };
            let bank = if sink.is_some() {
                synth::ALL
                    .iter()
                    .map(|&c| (c, rodio::buffer::SamplesBuffer::new(NonZero::new(1).unwrap(), NonZero::new(synth::SR).unwrap(), synth::render(c))))
                    .collect()
            } else {
                Default::default()
            };
            Audio { sink, bank, last: Default::default(), muted }
        }
        #[cfg(not(feature = "sound"))]
        Audio { muted }
    }

    #[cfg_attr(not(feature = "sound"), allow(unused_variables))]
    pub fn play(&mut self, cue: Cue) {
        #[cfg(feature = "sound")]
        {
            use rodio::Source;
            if self.muted {
                return;
            }
            let gap = match cue {
                Cue::Gun | Cue::EnemyGun | Cue::HitConfirm => 70,
                Cue::ShieldHit | Cue::ArmourHit => 60,
                Cue::Empty => 250,
                _ => 0,
            };
            if gap > 0 {
                let now = std::time::Instant::now();
                if self.last.get(&cue).is_some_and(|t| now.duration_since(*t).as_millis() < gap) {
                    return;
                }
                self.last.insert(cue, now);
            }
            let (Some(sink), Some(b)) = (&self.sink, self.bank.get(&cue)) else { return };
            sink.mixer().add(b.clone().amplify(0.35));
        }
    }
}
