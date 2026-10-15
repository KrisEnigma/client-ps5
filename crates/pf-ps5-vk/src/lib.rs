//! The PS5 client's Vulkan plane: one device for the title's life, shared by the presenter, the
//! UI layer (Skia) and the decode lanes.
//!
//! `unsafe` is allowed here and in `pf-ps5-sys` only; every block carries its proof. The crate
//! builds where `pf-client-video` does (Linux, and the PS5 with the upstream target change).
#![cfg(any(target_os = "linux", target_vendor = "sie"))]

pub mod device;
pub mod output;
pub mod picture;
pub mod pipelines;
pub mod presenter;
pub mod pyro;
pub mod ui;
pub mod util;

pub use device::{GpuDevice, Target};
pub use picture::{Picture, Planes, VideoSlot};
pub use presenter::Presenter;
pub use pyro::PyroLane;
