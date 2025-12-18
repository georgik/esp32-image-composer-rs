use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub flash_size: FlashSize,
    pub firmware_dir: PathBuf,
    pub output_file: PathBuf,
    pub max_ota_partitions: usize,
    pub verbose: bool,
    pub pad_flash: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlashSize {
    #[serde(rename = "8MB")]
    Size8MB,
    #[serde(rename = "16MB")]
    Size16MB,
    #[serde(rename = "32MB")]
    Size32MB,
}

impl FlashSize {
    pub fn size_bytes(&self) -> u32 {
        match self {
            FlashSize::Size8MB => 8 * 1024 * 1024,
            FlashSize::Size16MB => 16 * 1024 * 1024,
            FlashSize::Size32MB => 32 * 1024 * 1024,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            flash_size: FlashSize::Size16MB,
            firmware_dir: PathBuf::from("firmwares"),
            output_file: PathBuf::from("combined-image.bin"),
            max_ota_partitions: 16,
            verbose: false,
            pad_flash: false,
        }
    }
}

pub mod defaults {

    pub const BOOTLOADER_OFFSET: u32 = 0x2000; // ESP32-P4 bootloader at 0x2000 (from ESP-IDF flash_args)
    pub const BOOTLOADER_SIZE: u32 = 24 * 1024; // 24KB (to fit between 0x2000 and 0x8000)
    pub const PARTITION_TABLE_OFFSET: u32 = 0x10000; // ESP32-P4 partition table at 0x10000 (from ESP-IDF flash_args)
    pub const PARTITION_TABLE_SIZE: u32 = 4 * 1024; // 4KB
    pub const NVS_OFFSET: u32 = 0x9000; // NVS between bootloader and partition table
    pub const NVS_SIZE: u32 = 4 * 1024; // 4KB (to avoid overlap with otadata)
    pub const OTADATA_OFFSET: u32 = 0xA000; // Move earlier
    pub const OTADATA_SIZE: u32 = 8 * 1024; // 8KB
    pub const FACTORY_OFFSET: u32 = 0x20000; // ESP32-P4 factory app at 0x20000 (from ESP-IDF flash_args)
    pub const FACTORY_SIZE: u32 = 1 * 1024 * 1024; // 1MB

    pub const OTA_ALIGNMENT: u32 = 64 * 1024; // 64KB
    pub const MIN_OTA_SIZE: u32 = 256 * 1024; // 256KB
    pub const DEFAULT_OTA_SIZE: u32 = 4 * 1024 * 1024; // 4MB
}
