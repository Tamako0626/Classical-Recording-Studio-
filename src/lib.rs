//! 古典录音工作台: GPUI Kit 界面, 裁剪版 ffmpeg 处理音频.

pub mod catalog;
pub mod cue;
pub mod effects;
pub mod error;
pub mod export;
pub mod ffmpeg;
pub mod gui;
pub mod llm;
pub mod mb;
pub mod model;
pub mod names;
pub mod playback;
pub mod store;
pub mod video;

pub use error::{StudioError, StudioResult};
