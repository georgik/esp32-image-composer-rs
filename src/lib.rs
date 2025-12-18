pub mod cli;
pub mod config;
pub mod esp32;
pub mod firmware;
pub mod image;
pub mod partition;

pub use config::Config;
pub use esp32::{Esp32P4Processor, EspChecksum};
pub use firmware::{FirmwareBinary, FirmwareLoader};
pub use image::ImageBuilder;
pub use partition::PartitionGenerator;

pub type Result<T> = anyhow::Result<T>;
