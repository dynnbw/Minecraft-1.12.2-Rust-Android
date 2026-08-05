#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

//! Minecraft Java Edition 1.12.2 client port.
//! Source paths mirror the MCP 1.12.2 package/class layout. Native Vulkan and
//! OpenGL implementation details remain under `src/vulkan` and `src/opengl`;
//! launcher, backend selection and Java compatibility support stay outside the
//! MCP package tree.

pub mod com;
pub mod compat;
pub mod launcher;
pub mod net;
pub mod opengl;
pub mod renderer;
pub mod vulkan;

pub const GAME_VERSION: &str = "1.12.2";
pub const PROTOCOL_VERSION: i32 = 340;
