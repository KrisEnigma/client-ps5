//! Every struct the client hands the console, pinned at the size and offsets the console sources
//! agree on (`docs/NOTES.md` § "ABI sources"). Run on any 64-bit host: the ABI is `SysV` LP64.

use std::mem::{offset_of, size_of};

use pf_ps5_sys::audio::GRAIN_FRAMES;
use pf_ps5_sys::net::SceNetEpollEvent;
use pf_ps5_sys::pad::*;
use pf_ps5_sys::user::SceUserServiceLoginUserIdList;
use pf_ps5_sys::videodec2::*;

#[test]
fn pad_data_is_120_bytes() {
    assert_eq!(size_of::<ScePadData>(), 120);
    assert_eq!(offset_of!(ScePadData, l2), 0x08);
    assert_eq!(offset_of!(ScePadData, orientation), 0x0c);
    assert_eq!(offset_of!(ScePadData, acceleration), 0x1c);
    assert_eq!(offset_of!(ScePadData, angular_velocity), 0x28);
    assert_eq!(offset_of!(ScePadData, touch_count), 0x34);
    assert_eq!(offset_of!(ScePadData, touch), 0x3c);
    assert_eq!(offset_of!(ScePadData, connected), 0x4c);
    assert_eq!(offset_of!(ScePadData, timestamp_us), 0x50);
    assert_eq!(offset_of!(ScePadData, ext), 0x58);
    assert_eq!(offset_of!(ScePadData, connected_count), 0x68);
    assert_eq!(offset_of!(ScePadData, device_unique_len), 0x6b);
    assert_eq!(offset_of!(ScePadData, device_unique), 0x6c);
}

#[test]
fn pad_feedback_structs() {
    assert_eq!(size_of::<ScePadVibrationParam>(), 2);
    assert_eq!(size_of::<ScePadLightBarParam>(), 4);
    assert_eq!(size_of::<ScePadTriggerEffectCommand>(), 56);
    assert_eq!(offset_of!(ScePadTriggerEffectCommand, data), 8);
    assert_eq!(size_of::<ScePadTriggerEffectParam>(), 120);
    assert_eq!(offset_of!(ScePadTriggerEffectParam, command), 8);
    assert_eq!(
        offset_of!(ScePadTriggerEffectParam, command) + size_of::<ScePadTriggerEffectCommand>(),
        64
    );
}

#[test]
fn videodec2_structs() {
    assert_eq!(size_of::<Videodec2DecoderConfig>(), 72);
    assert_eq!(offset_of!(Videodec2DecoderConfig, codec_type), 0x0c);
    assert_eq!(offset_of!(Videodec2DecoderConfig, max_width), 0x18);
    assert_eq!(offset_of!(Videodec2DecoderConfig, compute_queue), 0x28);
    assert_eq!(offset_of!(Videodec2DecoderConfig, cpu_affinity), 0x30);
    assert_eq!(offset_of!(Videodec2DecoderConfig, cpu_priority), 0x38);
    assert_eq!(offset_of!(Videodec2DecoderConfig, optimize_progressive), 0x3c);
    assert_eq!(size_of::<Videodec2DecoderMemory>(), 72);
    assert_eq!(offset_of!(Videodec2DecoderMemory, cpu_gpu), 0x30);
    assert_eq!(offset_of!(Videodec2DecoderMemory, max_frame_size), 0x38);
    assert_eq!(offset_of!(Videodec2DecoderMemory, frame_alignment), 0x40);
    assert_eq!(size_of::<Videodec2ComputeConfig>(), 16);
    assert_eq!(size_of::<Videodec2ComputeMemory>(), 24);
    assert_eq!(size_of::<Videodec2Input>(), 48);
    assert_eq!(offset_of!(Videodec2Input, dts), 0x20);
    assert_eq!(size_of::<Videodec2Frame>(), 32);
    assert_eq!(offset_of!(Videodec2Frame, accepted), 0x18);
    assert_eq!(size_of::<Videodec2Output>(), 56);
    assert_eq!(offset_of!(Videodec2Output, pitch), 0x14);
    assert_eq!(offset_of!(Videodec2Output, height), 0x18);
    assert_eq!(offset_of!(Videodec2Output, buffer), 0x20);
    assert_eq!(offset_of!(Videodec2Output, frame_format), 0x30);
    assert_eq!(offset_of!(Videodec2Output, pitch_bytes), 0x34);
}

#[test]
fn every_videodec2_struct_starts_sized() {
    assert_eq!(Videodec2Output::new().size, 56);
    assert_eq!(Videodec2DecoderConfig::new().size, 72);
}

#[test]
fn user_and_net_structs() {
    assert_eq!(size_of::<SceUserServiceLoginUserIdList>(), 16);
    assert_eq!(SceUserServiceLoginUserIdList::default().user_id, [-1; 4]);
    assert_eq!(size_of::<SceNetEpollEvent>(), 24);
    assert_eq!(GRAIN_FRAMES % 256, 0);
}
