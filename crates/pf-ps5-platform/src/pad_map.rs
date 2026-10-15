//! scePad samples into the app's [`PadSample`]: wire button bits, sticks in the wire's range
//! with +y up, triggers 0..=255. Pure, so it tests anywhere.

use pf_ps5_app::platform::{PadKind, PadSample};
use pf_ps5_sys::pad::*;
use punktfunk_core::input::gamepad as wire;

const BUTTONS: [(u32, u32); 15] = [
    (BUTTON_CROSS, wire::BTN_A),
    (BUTTON_CIRCLE, wire::BTN_B),
    (BUTTON_SQUARE, wire::BTN_X),
    (BUTTON_TRIANGLE, wire::BTN_Y),
    (BUTTON_L1, wire::BTN_LB),
    (BUTTON_R1, wire::BTN_RB),
    (BUTTON_L3, wire::BTN_LS_CLICK),
    (BUTTON_R3, wire::BTN_RS_CLICK),
    (BUTTON_OPTIONS, wire::BTN_START),
    (BUTTON_CREATE, wire::BTN_BACK),
    (BUTTON_UP, wire::BTN_DPAD_UP),
    (BUTTON_DOWN, wire::BTN_DPAD_DOWN),
    (BUTTON_LEFT, wire::BTN_DPAD_LEFT),
    (BUTTON_RIGHT, wire::BTN_DPAD_RIGHT),
    (BUTTON_TOUCHPAD, wire::BTN_TOUCHPAD),
];

/// 0..=255 (128 centre) to the wire's i16 range.
fn stick(raw: u8) -> i16 {
    let centred = i32::from(raw) - 128;
    (centred * 32767 / 127).clamp(-32768, 32767) as i16
}

/// One pad's sample, or `None` when it is not connected. A sample the shell has intercepted
/// (home menu, a system dialog) reads as a pad with nothing held, so the host gets releases.
pub fn sample(slot: u8, data: &ScePadData) -> Option<PadSample> {
    if data.connected == 0 {
        return None;
    }
    let mut out = PadSample {
        slot,
        kind: PadKind::DualSense,
        ..PadSample::default()
    };
    if data.buttons & BUTTON_INTERCEPTED != 0 {
        return Some(out);
    }
    out.buttons = BUTTONS
        .iter()
        .filter(|(sce, _)| data.buttons & sce != 0)
        .fold(0, |acc, (_, w)| acc | w);
    out.lx = stick(data.left_x);
    out.ly = stick(data.left_y).saturating_neg();
    out.rx = stick(data.right_x);
    out.ry = stick(data.right_y).saturating_neg();
    out.lt = data.l2;
    out.rt = data.r2;
    Some(out)
}

/// 0..=65535 motor level to scePad's 0..=255.
pub fn motor(level: u16) -> u8 {
    (level >> 8) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(buttons: u32) -> ScePadData {
        ScePadData {
            buttons,
            connected: 1,
            left_x: 128,
            left_y: 128,
            right_x: 128,
            right_y: 128,
            ..ScePadData::default()
        }
    }

    #[test]
    fn buttons_map_to_wire_bits() {
        let s = sample(0, &data(BUTTON_CROSS | BUTTON_OPTIONS | BUTTON_TOUCHPAD)).unwrap();
        assert_eq!(s.buttons, wire::BTN_A | wire::BTN_START | wire::BTN_TOUCHPAD);
        assert_eq!((s.lx, s.ly), (0, 0));
    }

    #[test]
    fn up_on_the_stick_is_positive_on_the_wire() {
        let mut d = data(0);
        d.left_y = 0;
        d.right_x = 255;
        let s = sample(0, &d).unwrap();
        assert!(s.ly > 32_000, "{}", s.ly);
        assert_eq!(s.rx, 32767);
    }

    #[test]
    fn an_intercepted_sample_holds_nothing() {
        let s = sample(1, &data(BUTTON_CROSS | BUTTON_INTERCEPTED)).unwrap();
        assert_eq!(s.buttons, 0);
        assert!(sample(0, &ScePadData::default()).is_none());
    }
}
