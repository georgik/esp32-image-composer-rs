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
