//! The gamepad (Kaiku's `pad.rs`, remapped for flight). Gamepads are
//! first-class in Asciiverse: gilrs, only SDL-mapped devices
//! (`SAALIS_ANY_PAD=1` accepts any), hot-plugging, rumble. Buttons go by
//! position (gilrs' South is Xbox A but Switch B).
//!
//! Left stick or d-pad: x turns, y is the throttle. South fires (and
//! confirms), East is the afterburner, the right bumper cycles targets, the
//! left bumper picks the nearest, Start pauses, Select is help.

#[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
const DEADZONE: f32 = 0.3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    pub turn: f32,
    /// up is faster
    pub throttle: f32,
    pub fire: bool,
    pub burner: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadPresses {
    pub target: bool,
    pub nearest: bool,
    pub confirm: bool,
    pub pause: bool,
    pub help: bool,
}

#[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
pub fn shape(v: f32) -> f32 {
    let a = v.abs();
    if a < DEADZONE { 0.0 } else { v.signum() * ((a - DEADZONE) / (1.0 - DEADZONE)).min(1.0) }
}

pub struct Pad {
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    #[cfg(feature = "gamepad")]
    id: Option<gilrs::GamepadId>,
    #[cfg(feature = "gamepad")]
    rumbles: Vec<(gilrs::ff::Effect, std::time::Instant)>,
    pub name: Option<String>,
    pub state: PadState,
    presses: PadPresses,
}

#[cfg(feature = "gamepad")]
fn usable(p: &gilrs::Gamepad) -> bool {
    p.mapping_source() == gilrs::MappingSource::SdlMappings || std::env::var("SAALIS_ANY_PAD").is_ok_and(|v| v == "1")
}

impl Pad {
    pub fn open() -> Pad {
        #[cfg_attr(not(feature = "gamepad"), allow(unused_mut))]
        let mut pad = Pad {
            #[cfg(feature = "gamepad")]
            gilrs: None,
            #[cfg(feature = "gamepad")]
            id: None,
            #[cfg(feature = "gamepad")]
            rumbles: Vec::new(),
            name: None,
            state: PadState::default(),
            presses: PadPresses::default(),
        };
        #[cfg(feature = "gamepad")]
        match gilrs::Gilrs::new() {
            Ok(g) => {
                let all: Vec<_> = g.gamepads().map(|(id, p)| (id, p.name().to_string(), usable(&p), p.is_ff_supported())).collect();
                log::info!("gamepads (id, name, used, rumble): {all:?}");
                if let Some((id, name, _, _)) = all.into_iter().find(|p| p.2) {
                    pad.id = Some(id);
                    pad.name = Some(name);
                }
                pad.gilrs = Some(g);
            }
            Err(e) => log::warn!("gamepad support unavailable: {e}"),
        }
        pad
    }

    /// Drains pad events; call once per frame, before reading the state.
    pub fn poll(&mut self) {
        #[cfg(feature = "gamepad")]
        {
            use gilrs::{Axis, Button, EventType};
            let Some(g) = &mut self.gilrs else { return };
            while let Some(ev) = g.next_event() {
                let pad = g.gamepad(ev.id);
                if !usable(&pad) {
                    continue;
                }
                match ev.event {
                    EventType::Connected => {
                        self.id = Some(ev.id);
                        self.name = Some(pad.name().to_string());
                        log::info!("gamepad connected: {:?}", self.name);
                    }
                    EventType::Disconnected => {
                        log::info!("gamepad disconnected");
                        self.id = None;
                        self.name = None;
                        self.state = PadState::default();
                        continue;
                    }
                    EventType::ButtonPressed(b, _) => {
                        log::debug!("pad press {b:?}");
                        match b {
                            Button::South => self.presses.confirm = true,
                            Button::RightTrigger => self.presses.target = true,
                            Button::LeftTrigger => self.presses.nearest = true,
                            Button::Start => self.presses.pause = true,
                            Button::Select => self.presses.help = true,
                            _ => {}
                        }
                    }
                    _ => {}
                }
                let on = |b: Button| pad.is_pressed(b);
                let axis = |a: Axis| pad.axis_data(a).map_or(0.0, |d| d.value());
                let dpad = |neg: Button, pos: Button| on(pos) as i8 as f32 - on(neg) as i8 as f32;
                let (sx, sy) = (shape(axis(Axis::LeftStickX)), shape(axis(Axis::LeftStickY)));
                let (dx, dy) = (dpad(Button::DPadLeft, Button::DPadRight), dpad(Button::DPadDown, Button::DPadUp));
                self.state = PadState {
                    turn: if dx != 0.0 { dx } else { sx },
                    throttle: if dy != 0.0 { dy } else { sy },
                    fire: on(Button::South),
                    burner: on(Button::East),
                };
            }
        }
    }

    #[cfg_attr(not(feature = "gamepad"), allow(unused_variables))]
    pub fn rumble(&mut self, strong: f32, weak: f32, ms: u32) {
        #[cfg(feature = "gamepad")]
        {
            use gilrs::ff::{BaseEffect, BaseEffectType, EffectBuilder, Replay, Ticks};
            let now = std::time::Instant::now();
            self.rumbles.retain(|(_, until)| *until > now);
            let (Some(id), Some(g)) = (self.id, &mut self.gilrs) else { return };
            if !g.gamepad(id).is_ff_supported() {
                return;
            }
            let dur = Ticks::from_ms(ms);
            let mag = |v: f32| (v.clamp(0.0, 1.0) * u16::MAX as f32) as u16;
            let built = EffectBuilder::new()
                .add_effect(BaseEffect { kind: BaseEffectType::Strong { magnitude: mag(strong) }, scheduling: Replay { play_for: dur, ..Default::default() }, envelope: Default::default() })
                .add_effect(BaseEffect { kind: BaseEffectType::Weak { magnitude: mag(weak) }, scheduling: Replay { play_for: dur, ..Default::default() }, envelope: Default::default() })
                .repeat(gilrs::ff::Repeat::For(dur))
                .gamepads(&[id])
                .finish(g);
            match built.and_then(|e| e.play().map(|_| e)) {
                Ok(e) => self.rumbles.push((e, now + std::time::Duration::from_millis(ms as u64 + 50))),
                Err(e) => log::debug!("rumble failed: {e}"),
            }
        }
    }

    pub fn feel(&mut self, cue: crate::sim::cue::Cue) {
        use crate::sim::cue::Cue;
        let (strong, weak, ms) = match cue {
            Cue::ShieldHit => (0.2, 0.3, 90),
            Cue::ArmourHit => (0.55, 0.3, 140),
            Cue::PlayerLost => (1.0, 1.0, 700),
            Cue::BigKill => (0.4, 0.3, 160),
            Cue::Kill => (0.15, 0.25, 90),
            Cue::Burner => (0.0, 0.35, 160),
            _ => return,
        };
        self.rumble(strong, weak, ms);
    }

    pub fn take_presses(&mut self) -> PadPresses {
        std::mem::take(&mut self.presses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stick_has_a_deadzone_then_scales_to_full() {
        assert_eq!(shape(0.2), 0.0);
        assert!((shape(0.65) - 0.5).abs() < 1e-6);
        assert_eq!(shape(-1.0), -1.0);
    }
}
