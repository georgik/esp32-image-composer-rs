use crate::Result;
use crate::config::Config;
use crate::esp32::Esp32P4Processor;
use crate::firmware::FirmwareBinary;
use crate::partition::PartitionGenerator;
use esp_idf_part::PartitionTable;
use log::info;

pub struct ImageBuilder;

impl ImageBuilder {
    pub fn build_flash_image(firmwares: &[FirmwareBinary], config: &Config) -> Result<Vec<u8>> {
        info!("Building flash image...");

        // Generate partition table
        let partition_table = PartitionGenerator::generate_table(firmwares, config)?;

        if config.pad_flash {
            // Create full flash-size buffer with 0xFF padding
            let flash_size = config.flash_size.size_bytes();
            let mut flash_image = vec![0xFF; flash_size as usize];

            // Write components to the full buffer
            Self::write_components_to_buffer(
                &mut flash_image,
                firmwares,
                &partition_table,
                config,
            )?;

            info!(
                "Flash image built successfully: {} bytes (full flash size)",
                flash_image.len()
            );
            Ok(flash_image)
        } else {
            // Create minimal buffer that grows as needed
            let mut flash_image = Vec::new();
            Self::write_components_to_minimal_buffer(
                &mut flash_image,
                firmwares,
                &partition_table,
                config,
            )?;

            info!(
                "Flash image built successfully: {} bytes (minimal size)",
                flash_image.len()
            );
            Ok(flash_image)
        }
    }

    /// Write components to a pre-allocated full-size flash buffer
    fn write_components_to_buffer(
        flash_image: &mut Vec<u8>,
        firmwares: &[FirmwareBinary],
        partition_table: &PartitionTable,
        config: &Config,
    ) -> Result<()> {
        // Process and write bootloader (first firmware)
        if !firmwares.is_empty() {
            let bootloader = &firmwares[0];
            info!("Processing bootloader: {} bytes", bootloader.size);

            let mut bootloader_data = bootloader.data.clone();
            Esp32P4Processor::process_bootloader_image(&mut bootloader_data)?;

            info!(
                "Writing processed bootloader: {} bytes",
                bootloader_data.len()
            );
            Self::write_to_flash(
                flash_image,
                crate::config::defaults::BOOTLOADER_OFFSET,
                &bootloader_data,
            )?;
        }

        // Write partition table
        info!("Writing partition table");
        let partition_table_data = Self::serialize_partition_table(partition_table)?;
        Self::write_to_flash(
            flash_image,
            crate::config::defaults::PARTITION_TABLE_OFFSET,
            &partition_table_data,
        )?;

        // Process and write factory app (second firmware)
        if firmwares.len() >= 2 {
            let factory_app = &firmwares[1];
            info!("Processing factory app: {} bytes", factory_app.size);

            let mut factory_app_data = factory_app.data.clone();
            Esp32P4Processor::process_app_image(&mut factory_app_data, false)?;
            Esp32P4Processor::verify_alignment(crate::config::defaults::FACTORY_OFFSET, true)?;

            info!(
                "Writing processed factory app: {} bytes",
                factory_app_data.len()
            );
            Self::write_to_flash(
                flash_image,
                crate::config::defaults::FACTORY_OFFSET,
                &factory_app_data,
            )?;
        }

        // Process and write OTA partitions (remaining firmwares)
        for (i, firmware) in firmwares.iter().skip(2).enumerate() {
            let ota_name = format!("ota_{}", i);
            if let Some(partition) = partition_table.find(&ota_name) {
                info!("Processing OTA partition {}: {} bytes", i, firmware.size);

                let mut ota_app_data = firmware.data.clone();
                Esp32P4Processor::process_app_image(&mut ota_app_data, false)?;
                Esp32P4Processor::verify_alignment(partition.offset(), true)?;

                info!(
                    "Writing processed OTA partition {}: {} bytes at 0x{:X}",
                    i,
                    ota_app_data.len(),
                    partition.offset()
                );
                Self::write_to_flash(flash_image, partition.offset(), &ota_app_data)?;
            }
        }

        Ok(())
    }

    /// Write components to a minimal buffer that grows as needed
    fn write_components_to_minimal_buffer(
        flash_image: &mut Vec<u8>,
        firmwares: &[FirmwareBinary],
        partition_table: &PartitionTable,
        config: &Config,
    ) -> Result<()> {
        let mut end_offset = 0u32;

        // Process and write bootloader (first firmware)
        if !firmwares.is_empty() {
            let bootloader = &firmwares[0];
            info!("Processing bootloader: {} bytes", bootloader.size);

            let mut bootloader_data = bootloader.data.clone();
            Esp32P4Processor::process_bootloader_image(&mut bootloader_data)?;

            let bootloader_offset = crate::config::defaults::BOOTLOADER_OFFSET;
            let bootloader_end = bootloader_offset + bootloader_data.len() as u32;

            // Ensure buffer is large enough
            if flash_image.len() < bootloader_end as usize {
                flash_image.resize(bootloader_end as usize, 0xFF);
            }

            // Write bootloader data
            let start = bootloader_offset as usize;
            let end = start + bootloader_data.len();
            flash_image[start..end].copy_from_slice(&bootloader_data);

            end_offset = end_offset.max(bootloader_end);
            info!(
                "Written processed bootloader: {} bytes at 0x{:X}",
                bootloader_data.len(),
                bootloader_offset
            );
        }

        // Write partition table
        info!("Writing partition table");
        let partition_table_data = Self::serialize_partition_table(partition_table)?;
        let pt_offset = crate::config::defaults::PARTITION_TABLE_OFFSET;
        let pt_end = pt_offset + partition_table_data.len() as u32;

        // Ensure buffer is large enough
        if flash_image.len() < pt_end as usize {
            flash_image.resize(pt_end as usize, 0xFF);
        }

        // Write partition table data
        let start = pt_offset as usize;
        let end = start + partition_table_data.len();
        flash_image[start..end].copy_from_slice(&partition_table_data);

        end_offset = end_offset.max(pt_end);

        // Process and write factory app (second firmware)
        if firmwares.len() >= 2 {
            let factory_app = &firmwares[1];
            info!("Processing factory app: {} bytes", factory_app.size);

            let mut factory_app_data = factory_app.data.clone();
            Esp32P4Processor::process_app_image(&mut factory_app_data, false)?;
            Esp32P4Processor::verify_alignment(crate::config::defaults::FACTORY_OFFSET, true)?;

            let factory_offset = crate::config::defaults::FACTORY_OFFSET;
            let factory_end = factory_offset + factory_app_data.len() as u32;

            // Ensure buffer is large enough
            if flash_image.len() < factory_end as usize {
                flash_image.resize(factory_end as usize, 0xFF);
            }

            // Write factory app data
            let start = factory_offset as usize;
            let end = start + factory_app_data.len();
            flash_image[start..end].copy_from_slice(&factory_app_data);

            end_offset = end_offset.max(factory_end);
            info!(
                "Written processed factory app: {} bytes at 0x{:X}",
                factory_app_data.len(),
                factory_offset
            );
        }

        // Process and write OTA partitions (remaining firmwares)
        for (i, firmware) in firmwares.iter().skip(2).enumerate() {
            let ota_name = format!("ota_{}", i);
            if let Some(partition) = partition_table.find(&ota_name) {
                info!("Processing OTA partition {}: {} bytes", i, firmware.size);

                let mut ota_app_data = firmware.data.clone();
                Esp32P4Processor::process_app_image(&mut ota_app_data, false)?;
                Esp32P4Processor::verify_alignment(partition.offset(), true)?;

                let ota_end = partition.offset() + ota_app_data.len() as u32;

                // Ensure buffer is large enough
                if flash_image.len() < ota_end as usize {
                    flash_image.resize(ota_end as usize, 0xFF);
                }

                // Write OTA app data
                let start = partition.offset() as usize;
                let end = start + ota_app_data.len();
                flash_image[start..end].copy_from_slice(&ota_app_data);

                end_offset = end_offset.max(ota_end);
                info!(
                    "Written processed OTA partition {}: {} bytes at 0x{:X}",
                    i,
                    ota_app_data.len(),
                    partition.offset()
                );
            }
        }

        info!(
            "Minimal image ends at offset 0x{:X} ({} bytes)",
            end_offset,
            flash_image.len()
        );
        Ok(())
    }

    pub fn build_partition_table_only(config: &Config) -> Result<Vec<u8>> {
        // Create a minimal firmware set just for partition table generation
        // We need at least bootloader to generate a valid table
        let dummy_bootloader = FirmwareBinary::new(
            "bootloader".to_string(),
            config.firmware_dir.join("dummy-bootloader.bin"),
            vec![0; 32 * 1024],
            1,
        );

        let dummy_factory = FirmwareBinary::new(
            "factory".to_string(),
            config.firmware_dir.join("dummy-factory.bin"),
            vec![0; 1 * 1024 * 1024],
            2,
        );

        let dummy_firmwares = vec![dummy_bootloader, dummy_factory];
        let partition_table = PartitionGenerator::generate_table(&dummy_firmwares, config)?;
        Self::serialize_partition_table(&partition_table)
    }

    fn serialize_partition_table(table: &PartitionTable) -> Result<Vec<u8>> {
        // Use the esp_idf_part crate to serialize to binary format
        let data = table.to_bin()?;
        Ok(data)
    }

    fn write_to_flash(flash_image: &mut [u8], offset: u32, data: &[u8]) -> Result<()> {
        let start = offset as usize;
        let end = start + data.len();

        if end > flash_image.len() {
            return Err(anyhow::anyhow!(
                "Write exceeds flash image bounds: offset={}, size={}, image_size={}",
                offset,
                data.len(),
                flash_image.len()
            ));
        }

        flash_image[start..end].copy_from_slice(data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FlashSize;
    use std::path::PathBuf;

    fn create_test_firmware(name: &str, size: usize, prefix: u32) -> FirmwareBinary {
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        FirmwareBinary::new(
            name.to_string(),
            PathBuf::from(format!("{}.bin", name)),
            data,
            prefix,
        )
    }

    #[test]
    fn test_write_to_flash() -> Result<()> {
        let mut flash_image = vec![0xFF; 1024];
        let data = vec![0x42, 0x43, 0x44];

        ImageBuilder::write_to_flash(&mut flash_image, 10, &data)?;

        assert_eq!(flash_image[8..13], [0xFF, 0xFF, 0x42, 0x43, 0x44]);
        assert_eq!(flash_image[0..8], [0xFF; 8]);
        assert_eq!(flash_image[13..], [0xFF; 1024 - 13]);

        Ok(())
    }

    #[test]
    fn test_write_to_flash_overflow() {
        let mut flash_image = vec![0xFF; 100];
        let data = vec![0x42; 10];

        let result = ImageBuilder::write_to_flash(&mut flash_image, 95, &data);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_partition_table_only() -> Result<()> {
        let config = Config {
            flash_size: FlashSize::Size16MB,
            ..Default::default()
        };

        let partition_table_data = ImageBuilder::build_partition_table_only(&config)?;

        // Should have some data (partition tables are typically a few KB)
        assert!(!partition_table_data.is_empty());
        assert!(partition_table_data.len() > 100);
        assert!(partition_table_data.len() < 10 * 1024); // Shouldn't be too large

        Ok(())
    }

    #[test]
    fn test_build_flash_image_minimal_size() -> Result<()> {
        let firmwares = vec![
            create_test_firmware("bootloader", 32 * 1024, 1),
            create_test_firmware("factory_app", 100 * 1024, 2),
        ];

        let config = Config {
            flash_size: FlashSize::Size16MB,
            max_ota_partitions: 4,
            pad_flash: false, // Minimal size
            ..Default::default()
        };

        let flash_image = ImageBuilder::build_flash_image(&firmwares, &config)?;

        // Should be much smaller than full flash size
        assert!(flash_image.len() < 16 * 1024 * 1024);
        assert!(flash_image.len() > 100 * 1024); // Should contain the firmware

        // Check bootloader at offset 0
        assert_eq!(&flash_image[0..10], &(0..10).collect::<Vec<_>>());

        // Check factory app at FACTORY_OFFSET
        let factory_offset = crate::config::defaults::FACTORY_OFFSET as usize;
        assert_eq!(
            &flash_image[factory_offset..factory_offset + 10],
            &(0..10).collect::<Vec<_>>()
        );

        Ok(())
    }

    #[test]
    fn test_build_flash_image_padded_size() -> Result<()> {
        let firmwares = vec![
            create_test_firmware("bootloader", 32 * 1024, 1),
            create_test_firmware("factory_app", 100 * 1024, 2),
        ];

        let config = Config {
            flash_size: FlashSize::Size16MB,
            max_ota_partitions: 4,
            pad_flash: true, // Full flash size
            ..Default::default()
        };

        let flash_image = ImageBuilder::build_flash_image(&firmwares, &config)?;

        // Should be exactly full flash size
        assert_eq!(flash_image.len(), 16 * 1024 * 1024);

        // Check bootloader at offset 0
        assert_eq!(&flash_image[0..10], &(0..10).collect::<Vec<_>>());

        // Check that most of the flash is still 0xFF (empty)
        let ff_count = flash_image.iter().filter(|&&b| b == 0xFF).count();
        assert!(ff_count > flash_image.len() / 2); // More than half should be empty

        Ok(())
    }

    #[test]
    fn test_build_flash_image_no_firmwares() -> Result<()> {
        let firmwares = vec![];
        let config = Config::default();

        let flash_image = ImageBuilder::build_flash_image(&firmwares, &config)?;

        // Should still create an image with just partition table
        assert!(flash_image.len() > 1000); // At least the partition table
        assert!(flash_image.len() < 16 * 1024 * 1024); // Minimal size

        Ok(())
    }

    #[test]
    fn test_build_flash_image_only_bootloader() -> Result<()> {
        let firmwares = vec![create_test_firmware("bootloader", 32 * 1024, 1)];

        let config = Config::default();

        let flash_image = ImageBuilder::build_flash_image(&firmwares, &config)?;

        // Should have bootloader + partition table
        assert!(flash_image.len() > 32 * 1024); // Bootloader
        assert!(flash_image.len() < 16 * 1024 * 1024); // Minimal size

        // Check bootloader at offset 0
        assert_eq!(&flash_image[0..10], &(0..10).collect::<Vec<_>>());

        Ok(())
    }
}
