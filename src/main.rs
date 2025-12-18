use clap::Parser;
use colored::*;
use esp32_image_composer_rs::{
    cli::Args, config::Config, firmware::FirmwareLoader, image::ImageBuilder,
};
use log::LevelFilter;
use std::fs;
use std::io::Write;
use std::process;

fn main() {
    let args = Args::parse();

    // Initialize logging
    let level = if args.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    env_logger::Builder::from_default_env()
        .filter_level(level)
        .init();

    if let Err(e) = run(args) {
        eprintln!("{}: {}", "Error".red().bold(), e);
        process::exit(1);
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config {
        flash_size: args.get_flash_size_enum(),
        firmware_dir: args.firmware_dir.clone(),
        output_file: args.output.clone(),
        max_ota_partitions: args.max_ota_partitions,
        verbose: args.verbose,
        pad_flash: args.pad_flash,
    };

    match args.command {
        Some(Commands::PartitionTable { output, csv }) => {
            generate_partition_table(&config, &output, csv, args.dry_run)?;
        }
        Some(Commands::Validate { detailed }) => {
            validate_firmwares(&config, detailed)?;
        }
        Some(Commands::Info { show_sizes }) => {
            show_firmware_info(&config, show_sizes)?;
        }
        Some(Commands::Inspect {
            image_file,
            detailed,
            verify_checksums,
        }) => {
            inspect_flash_image(&image_file, detailed, verify_checksums)?;
        }
        None => {
            generate_flash_image(&config, args.dry_run)?;
        }
    }

    Ok(())
}

use esp32_image_composer_rs::cli::Commands;

fn generate_flash_image(config: &Config, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "🚀 ESP32 Image Composer".green().bold());
    println!("Flash size: {}\n", config.flash_size.size_bytes());

    // Load firmware files
    println!("{} firmware directory...", "Loading".blue());
    let firmwares = FirmwareLoader::load_from_directory(&config.firmware_dir)?;
    println!("Found {} firmware files:", firmwares.len());

    for firmware in &firmwares {
        println!(
            "  {} {} ({} bytes)",
            "▸".yellow(),
            firmware.name.cyan(),
            format_size(firmware.size)
        );
    }
    println!();

    if dry_run {
        println!(
            "{}",
            "🔍 DRY RUN - Would create the following:".yellow().bold()
        );
    }

    // Build flash image
    println!("{} flash image...", "Building".blue());
    let flash_image = ImageBuilder::build_flash_image(&firmwares, config)?;

    if !dry_run {
        // Write to output file
        println!(
            "{} to {}...",
            "Writing".blue(),
            config.output_file.display()
        );
        let mut file = fs::File::create(&config.output_file)?;
        file.write_all(&flash_image)?;
        file.sync_all()?;

        println!(
            "✅ {} created successfully! ({})",
            config.output_file.display().to_string().green(),
            format_size(flash_image.len() as u32)
        );
    } else {
        println!(
            "📄 Would create flash image: {} ({})",
            config.output_file.display(),
            format_size(flash_image.len() as u32)
        );
    }

    Ok(())
}

fn generate_partition_table(
    config: &Config,
    output: &std::path::Path,
    csv: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "🗂️  Partition Table Generator".green().bold());

    if dry_run {
        println!(
            "{}",
            "🔍 DRY RUN - Would generate partition table"
                .yellow()
                .bold()
        );
        return Ok(());
    }

    let partition_table_data = if csv {
        // Generate CSV format
        println!("{} CSV partition table...", "Generating".blue());
        let dummy_bootloader = esp32_image_composer_rs::firmware::FirmwareBinary::new(
            "bootloader".to_string(),
            config.firmware_dir.join("dummy-bootloader.bin"),
            vec![0; 32 * 1024],
            1,
        );
        let dummy_factory = esp32_image_composer_rs::firmware::FirmwareBinary::new(
            "factory".to_string(),
            config.firmware_dir.join("dummy-factory.bin"),
            vec![0; 1 * 1024 * 1024],
            2,
        );
        let partition_table =
            esp32_image_composer_rs::partition::PartitionGenerator::generate_table(
                &[dummy_bootloader, dummy_factory],
                config,
            )?;
        partition_table.to_csv()?.into_bytes()
    } else {
        // Generate binary format
        println!("{} binary partition table...", "Generating".blue());
        ImageBuilder::build_partition_table_only(config)?
    };

    fs::write(output, partition_table_data)?;
    println!(
        "✅ {} created successfully!",
        output.display().to_string().green()
    );

    Ok(())
}

fn validate_firmwares(config: &Config, detailed: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "✅ Firmware Validator".green().bold());

    // Check firmware directory
    if !config.firmware_dir.exists() {
        return Err(format!(
            "Firmware directory does not exist: {}",
            config.firmware_dir.display()
        )
        .into());
    }

    // Load and validate firmwares
    let firmwares = FirmwareLoader::load_from_directory(&config.firmware_dir)?;

    println!("Found {} valid firmware files:", firmwares.len());
    for firmware in &firmwares {
        println!(
            "  {} {} ({} bytes)",
            "✓".green(),
            firmware.name.cyan(),
            format_size(firmware.size)
        );
    }

    if detailed {
        println!("\n{}", "📊 Detailed Partition Layout:".blue().bold());
        let partition_table =
            esp32_image_composer_rs::partition::PartitionGenerator::generate_table(
                &firmwares, config,
            )?;

        for partition in partition_table.partitions() {
            println!(
                "  {} {} @ 0x{:X} ({} bytes) [{}]",
                "▸".yellow(),
                partition.name().cyan(),
                partition.offset(),
                format_size(partition.size()),
                format!("{:?}", partition.subtype()).dimmed()
            );
        }

        let total_used: u32 = partition_table
            .partitions()
            .into_iter()
            .map(|p| p.size())
            .sum();
        let flash_size = config.flash_size.size_bytes();
        let usage_percent = (total_used as f64 / flash_size as f64) * 100.0;

        println!(
            "\n  {} Total used: {} / {} ({:.1}%)",
            "📈".blue(),
            format_size(total_used),
            format_size(flash_size),
            usage_percent
        );
    }

    println!("\n{}", "✅ All firmwares are valid!".green().bold());
    Ok(())
}

fn show_firmware_info(config: &Config, show_sizes: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "ℹ️  Firmware Information".green().bold());

    let firmwares = FirmwareLoader::load_from_directory(&config.firmware_dir)?;

    if firmwares.is_empty() {
        println!(
            "No firmware files found in {}",
            config.firmware_dir.display()
        );
        return Ok(());
    }

    println!("Firmware directory: {}", config.firmware_dir.display());
    println!("Total firmware files: {}\n", firmwares.len());

    for (i, firmware) in firmwares.iter().enumerate() {
        println!("{}. {}", i + 1, firmware.name.cyan());
        println!("   Prefix: {:02}", firmware.prefix);
        println!("   Path: {}", firmware.path.display());
        if show_sizes {
            println!("   Size: {} bytes", format_size(firmware.size));
            println!(
                "   Aligned size: {} bytes",
                format_size(align_size(firmware.size, 64 * 1024))
            );
        }
        println!();
    }

    let total_size: u32 = firmwares.iter().map(|f| f.size).sum();
    println!("Total firmware size: {}", format_size(total_size));

    Ok(())
}

fn format_size(bytes: u32) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

fn align_size(size: u32, alignment: u32) -> u32 {
    ((size + alignment - 1) / alignment) * alignment
}

fn inspect_flash_image(
    image_file: &std::path::Path,
    detailed: bool,
    verify_checksums: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "🔍 ESP32 Flash Image Inspector".green().bold());
    println!("Analyzing: {}\n", image_file.display());

    // Read the image file
    let image_data = std::fs::read(image_file)?;
    let image_size = image_data.len();

    println!(
        "📄 Image size: {} bytes ({})",
        format_size(image_size as u32),
        format_hex(image_size as u32)
    );

    // Analyze key components
    println!("\n{}", "🧩 Component Analysis:".blue().bold());

    // Check bootloader at 0x2000
    if image_size > 0x2000 {
        println!("\n  🚀 Bootloader (offset 0x2000):");
        if let Some(bootloader_data) = get_component_at_offset(&image_data, 0x2000, 0x8000) {
            println!(
                "    Size: {} bytes",
                format_size(bootloader_data.len() as u32)
            );

            if bootloader_data.len() > 0 {
                println!(
                    "    Magic: 0x{:02X} {}",
                    bootloader_data[0],
                    if bootloader_data[0] == 0xE9 {
                        "(valid ESP32)"
                    } else {
                        "(invalid)"
                    }
                );

                if verify_checksums {
                    if let Ok(verified) =
                        esp32_image_composer_rs::esp32::EspChecksum::verify_checksum(
                            &bootloader_data,
                        )
                    {
                        println!(
                            "    Checksum: {} {}",
                            if verified { "✅".green() } else { "❌".red() },
                            format!("(0x{:02X})", bootloader_data[bootloader_data.len() - 1])
                        );

                        if !verified {
                            if let Ok(calculated) =
                                esp32_image_composer_rs::esp32::EspChecksum::calculate_checksum(
                                    &bootloader_data[..bootloader_data.len() - 1],
                                )
                            {
                                println!("    Calculated: 0x{:02X}", calculated);
                            }
                        }
                    } else {
                        println!("    Checksum: ⚠️  Unable to verify");
                    }
                } else {
                    println!(
                        "    Checksum: 0x{:02X}",
                        bootloader_data[bootloader_data.len() - 1]
                    );
                }
            }
        } else {
            println!("    ❌ Not found or invalid");
        }
    }

    // Check partition table at 0x8000
    if image_size > 0x8000 {
        println!("\n  📋 Partition Table (offset 0x8000):");
        if let Some(pt_data) = get_component_at_offset(&image_data, 0x8000, 0x9000) {
            println!("    Size: {} bytes", format_size(pt_data.len() as u32));

            if pt_data.len() > 0 {
                println!(
                    "    Magic: 0x{:02X}{:02X} {}",
                    pt_data[0],
                    pt_data[1],
                    if pt_data[0] == 0xAA && pt_data[1] == 0x50 {
                        "(valid MD5)"
                    } else {
                        "(invalid)"
                    }
                );

                // Count partitions
                let mut partition_count = 0;
                for chunk in pt_data.chunks(32) {
                    if chunk.len() >= 2 && chunk[0] == 0xAA && chunk[1] == 0x50 {
                        partition_count += 1;
                        // Extract partition name if valid
                        if chunk.len() >= 16 {
                            let name_bytes = &chunk[8..24];
                            if let Ok(name) = std::str::from_utf8(name_bytes) {
                                let name_clean = name.trim_end_matches('\0');
                                if !name_clean.is_empty() {
                                    println!(
                                        "      📦 Partition {}: {}",
                                        partition_count,
                                        name_clean.cyan()
                                    );
                                }
                            }
                        }
                    } else if chunk.len() >= 2 && chunk[0] == 0xEB && chunk[1] == 0xEB {
                        // MD5 magic - end of partitions
                        break;
                    }
                }
                println!("    Total partitions: {}", partition_count);
            }
        } else {
            println!("    ❌ Not found or invalid");
        }
    }

    // Check factory app at 0x10000
    if image_size > 0x10000 {
        println!("\n  🏭 Factory App (offset 0x10000):");
        if let Some(factory_data) = get_component_at_offset(&image_data, 0x10000, 0x20000) {
            println!("    Size: {} bytes", format_size(factory_data.len() as u32));

            if factory_data.len() > 0 {
                println!(
                    "    Magic: 0x{:02X} {}",
                    factory_data[0],
                    if factory_data[0] == 0xE9 {
                        "(valid ESP32)"
                    } else {
                        "(invalid)"
                    }
                );

                if verify_checksums {
                    if let Ok(verified) =
                        esp32_image_composer_rs::esp32::EspChecksum::verify_checksum(&factory_data)
                    {
                        println!(
                            "    Checksum: {} {}",
                            if verified { "✅".green() } else { "❌".red() },
                            format!("(0x{:02X})", factory_data[factory_data.len() - 1])
                        );

                        if !verified {
                            if let Ok(calculated) =
                                esp32_image_composer_rs::esp32::EspChecksum::calculate_checksum(
                                    &factory_data[..factory_data.len() - 1],
                                )
                            {
                                println!("    Calculated: 0x{:02X}", calculated);
                            }
                        }
                    } else {
                        println!("    Checksum: ⚠️  Unable to verify");
                    }
                } else {
                    println!(
                        "    Checksum: 0x{:02X}",
                        factory_data[factory_data.len() - 1]
                    );
                }
            }
        } else {
            println!("    ❌ Not found or invalid");
        }
    }

    if detailed {
        println!("\n{}", "🔬 Detailed Analysis:".blue().bold());

        // Look for OTA partitions
        let mut ota_count = 0;
        for i in 0..16 {
            let ota_offset = 0x110000 + (i * 0x100000);
            if image_size > ota_offset {
                if let Some(ota_data) =
                    get_component_at_offset(&image_data, ota_offset, ota_offset + 0x100000)
                {
                    if ota_data.len() > 1000 && ota_data[0] == 0xE9 {
                        // Valid ESP32 app
                        ota_count += 1;
                        println!("  🔄 OTA Partition {} (offset 0x{:X}):", i, ota_offset);
                        println!("    Size: {} bytes", format_size(ota_data.len() as u32));

                        if verify_checksums {
                            if let Ok(verified) =
                                esp32_image_composer_rs::esp32::EspChecksum::verify_checksum(
                                    &ota_data,
                                )
                            {
                                println!(
                                    "    Checksum: {}",
                                    if verified { "✅".green() } else { "❌".red() }
                                );
                            }
                        }
                    }
                }
            }
        }

        if ota_count == 0 {
            println!("  🔄 OTA Partitions: None found");
        }

        // Show memory usage analysis
        println!("\n  📊 Memory Usage:");
        let used_bytes = find_last_used_byte(&image_data);
        if used_bytes > 0 {
            let usage_percent = (used_bytes as f64 / image_size as f64) * 100.0;
            println!(
                "    Used bytes: {} ({:.1}%)",
                format_size(used_bytes as u32),
                usage_percent
            );
            println!("    Last used offset: 0x{:X}", used_bytes);
        } else {
            println!("    Used bytes: Unable to determine");
        }
    }

    println!("\n{}", "✅ Image inspection completed".green().bold());
    Ok(())
}

fn get_component_at_offset(
    image_data: &[u8],
    start_offset: usize,
    next_component_offset: usize,
) -> Option<&[u8]> {
    if start_offset >= image_data.len() {
        return None;
    }

    // Check if we have valid ESP32 magic bytes at start
    if start_offset < image_data.len() && image_data[start_offset] != 0xE9 {
        return None;
    }

    // Parse ESP32 image header to get actual component size
    if start_offset + 24 <= image_data.len() {
        // ESP32 image header structure:
        // bytes 0-3: magic (0xE9)
        // bytes 4-7: segment count
        // bytes 8-11: flash mode, size, frequency
        // bytes 12-15: entry point
        // bytes 16-23: extended header (for newer chips)

        let segment_count = u32::from_le_bytes([
            image_data[start_offset + 4],
            image_data[start_offset + 5],
            image_data[start_offset + 6],
            image_data[start_offset + 7],
        ]) as usize;

        if segment_count > 0 && segment_count <= 16 {
            // Calculate the size by reading segment headers
            let mut total_size = 24; // Header size

            // Add extended header size if present (ESP32-P4 has this)
            if image_data[start_offset + 3] & 0x80 != 0 {
                total_size += 16; // Extended header
            }

            // Add segment headers (8 bytes each)
            total_size += segment_count * 8;

            // Add segment data sizes
            let mut pos = start_offset + total_size;
            for _seg in 0..segment_count {
                if pos + 8 <= image_data.len() {
                    // Each segment header: offset (4 bytes) + size (4 bytes)
                    let seg_size = u32::from_le_bytes([
                        image_data[pos + 4],
                        image_data[pos + 5],
                        image_data[pos + 6],
                        image_data[pos + 7],
                    ]);

                    total_size += seg_size as usize;
                    pos += 8;
                }
            }

            // Add checksum byte
            total_size += 1;

            // Make sure we don't exceed the image bounds
            let end_offset = (start_offset + total_size).min(image_data.len());
            if end_offset > start_offset {
                return Some(&image_data[start_offset..end_offset]);
            }
        }
    }

    // Fallback: use next component offset or end of file
    let end_offset = if next_component_offset < image_data.len() {
        next_component_offset
    } else {
        image_data.len()
    };

    if end_offset > start_offset {
        Some(&image_data[start_offset..end_offset])
    } else {
        None
    }
}

fn find_last_used_byte(image_data: &[u8]) -> usize {
    for (i, &byte) in image_data.iter().enumerate().rev() {
        if byte != 0xFF {
            return i + 1;
        }
    }
    0
}

fn format_hex(value: u32) -> String {
    format!("0x{:X}", value)
}
