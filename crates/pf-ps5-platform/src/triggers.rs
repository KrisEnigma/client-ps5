//! The host's adaptive-trigger blocks into scePad's trigger effects.
//!
//! The host forwards what the game wrote to its virtual `DualSense`: the raw parameter block of
//! the pad's output report (mode byte first). scePad takes Sony's high-level modes instead, so
//! each raw mode is decoded to its position/strength form. A mode with no scePad equivalent
//! (bow, galloping, machine) leaves the trigger as it was.

use pf_ps5_sys::pad::*;

/// The command for one trigger, or `None` to leave it unchanged.
pub fn command(effect: &[u8]) -> Option<ScePadTriggerEffectCommand> {
    let b = |i: usize| effect.get(i).copied().unwrap_or(0);
    let u16_at = |i: usize| u16::from_le_bytes([b(i), b(i + 1)]);
    let u32_at = |i: usize| u32::from_le_bytes([b(i), b(i + 1), b(i + 2), b(i + 3)]);
    let pos = |raw: u8| (u32::from(raw) * 9 / 255) as u8;
    let level = |raw: u8| ((u32::from(raw) * 8 + 127) / 255) as u8;
    let mut cmd = ScePadTriggerEffectCommand::default();
    match b(0) {
        0x00 | 0x05 => {}
        // Simple feedback: start, force.
        0x01 => {
            cmd.mode = TRIGGER_MODE_FEEDBACK;
            cmd.data[0] = pos(b(1));
            cmd.data[1] = level(b(2));
        }
        // Simple weapon: start, end, force.
        0x02 => {
            let start = pos(b(1)).clamp(2, 7);
            cmd.mode = TRIGGER_MODE_WEAPON;
            cmd.data[0] = start;
            cmd.data[1] = pos(b(2)).clamp(start + 1, 8);
            cmd.data[2] = level(b(3));
        }
        // Simple vibration: frequency, amplitude, start.
        0x06 => {
            cmd.mode = TRIGGER_MODE_VIBRATION;
            cmd.data[0] = pos(b(3));
            cmd.data[1] = level(b(2));
            cmd.data[2] = b(1);
        }
        // Feedback: active zones (bit per position), 3-bit strength-1 per zone.
        0x21 => {
            let (zones, forces) = (u16_at(1), u32_at(3));
            cmd.mode = TRIGGER_MODE_MULTI_FEEDBACK;
            for i in 0..10 {
                if zones & (1 << i) != 0 {
                    cmd.data[i] = ((forces >> (3 * i)) & 7) as u8 + 1;
                }
            }
        }
        // Weapon: the start and end zones, strength-1.
        0x25 => {
            let zones = u16_at(1);
            if zones == 0 {
                return Some(cmd);
            }
            cmd.mode = TRIGGER_MODE_WEAPON;
            cmd.data[0] = zones.trailing_zeros() as u8;
            cmd.data[1] = 15 - zones.leading_zeros() as u8;
            cmd.data[2] = (b(3) & 7) + 1;
        }
        // Vibration: active zones, 3-bit amplitude-1 per zone, frequency at byte 9.
        0x26 => {
            let (zones, amps) = (u16_at(1), u32_at(3));
            cmd.mode = TRIGGER_MODE_MULTI_VIBRATION;
            cmd.data[0] = b(9);
            for i in 0..10 {
                if zones & (1 << i) != 0 {
                    cmd.data[1 + i] = ((amps >> (3 * i)) & 7) as u8 + 1;
                }
            }
        }
        _ => return None,
    }
    Some(cmd)
}

/// The full parameter for trigger `which` (0 = L2, 1 = R2).
pub fn param(which: u8, effect: &[u8]) -> Option<ScePadTriggerEffectParam> {
    let cmd = command(effect)?;
    let mut p = ScePadTriggerEffectParam::default();
    let index = usize::from(which.min(1));
    p.trigger_mask = if index == 0 { TRIGGER_L2 } else { TRIGGER_R2 };
    p.command[index] = cmd;
    Some(p)
}

/// Both triggers off: what a session leaves behind.
pub fn reset() -> ScePadTriggerEffectParam {
    ScePadTriggerEffectParam {
        trigger_mask: TRIGGER_L2 | TRIGGER_R2,
        ..ScePadTriggerEffectParam::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feedback from position 3 at strength 5: zones 3..9 active, each holding 4.
    #[test]
    fn zoned_feedback_decodes_per_position() {
        let zones: u16 = 0b11_1111_1000;
        let forces: u32 = (3..10).fold(0, |acc, i| acc | (4 << (3 * i)));
        let mut raw = vec![0x21];
        raw.extend_from_slice(&zones.to_le_bytes());
        raw.extend_from_slice(&forces.to_le_bytes());
        let cmd = command(&raw).unwrap();
        assert_eq!(cmd.mode, TRIGGER_MODE_MULTI_FEEDBACK);
        assert_eq!(&cmd.data[..10], &[0, 0, 0, 5, 5, 5, 5, 5, 5, 5]);
    }

    #[test]
    fn weapon_takes_its_end_zones() {
        let zones: u16 = (1 << 2) | (1 << 6);
        let mut raw = vec![0x25];
        raw.extend_from_slice(&zones.to_le_bytes());
        raw.push(7);
        let cmd = command(&raw).unwrap();
        assert_eq!(
            (cmd.mode, cmd.data[0], cmd.data[1], cmd.data[2]),
            (TRIGGER_MODE_WEAPON, 2, 6, 8)
        );
    }

    #[test]
    fn off_and_unknown_modes() {
        assert_eq!(command(&[0x05]).unwrap().mode, TRIGGER_MODE_OFF);
        assert!(command(&[0x22, 1, 2]).is_none(), "bow has no scePad form");
        let p = param(1, &[0x01, 255, 255]).unwrap();
        assert_eq!(p.trigger_mask, TRIGGER_R2);
        assert_eq!(
            (p.command[1].mode, p.command[1].data[0], p.command[1].data[1]),
            (TRIGGER_MODE_FEEDBACK, 9, 8)
        );
    }
}
