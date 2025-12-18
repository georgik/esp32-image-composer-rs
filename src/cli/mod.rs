use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "esp32-image-composer-rs")]
#[command(about = "ESP32 Image Composer - Create flashable ESP32 images from firmware binaries")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Firmware directory containing *.bin files with numerical prefixes
    #[arg(short, long, default_value = "firmwares")]
    pub firmware_dir: PathBuf,

    /// Output file path for the generated flash image
    #[arg(short, long, default_value = "combined-image.bin")]
    pub output: PathBuf,

    /// Flash size
    #[arg(long, default_value = "16MB", value_parser = ["8MB", "16MB", "32MB"])]
    pub flash_size: String,

    /// Maximum number of OTA partitions to create
    #[arg(long, default_value = "16")]
    pub max_ota_partitions: usize,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Dry run - show what would be done without creating files
    #[arg(long)]
    pub dry_run: bool,

    /// Pad image to full flash size with 0xFF (default: minimal size)
    #[arg(long)]
    pub pad_flash: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate only the partition table
    PartitionTable {
        /// Output file for partition table only
        #[arg(short, long, default_value = "partition-table.bin")]
        output: PathBuf,

        /// Export as CSV format instead of binary
        #[arg(long)]
        csv: bool,
    },
    /// Validate firmware files and show partition layout
    Validate {
        /// Show detailed partition information
        #[arg(long)]
        detailed: bool,
    },
    /// Show firmware information
    Info {
        /// Show binary sizes and layout information
        #[arg(long)]
        show_sizes: bool,
    },

    /// Inspect and analyze generated flash images
    Inspect {
        /// Flash image file to analyze
        image_file: PathBuf,

        /// Show detailed component analysis
        #[arg(long)]
        detailed: bool,

        /// Verify checksums of all components
        #[arg(long)]
        verify_checksums: bool,
    },
}

impl Args {
    pub fn get_flash_size_enum(&self) -> crate::config::FlashSize {
        match self.flash_size.as_str() {
            "8MB" => crate::config::FlashSize::Size8MB,
            "16MB" => crate::config::FlashSize::Size16MB,
            "32MB" => crate::config::FlashSize::Size32MB,
            _ => crate::config::FlashSize::Size16MB,
        }
    }
}
