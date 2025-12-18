use crate::Result;
use crate::config::Config;
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

        // Calculate total flash size
        let flash_size = config.flash_size.size_bytes();

        // Create flash image buffer
        let mut flash_image = vec![0xFF; flash_size as usize];

        // Write bootloader (first firmware)
        if !firmwares.is_empty() {
            let bootloader = &firmwares[0];
            info!("Writing bootloader: {} bytes", bootloader.size);
            Self::write_to_flash(
                &mut flash_image,
                crate::config::defaults::BOOTLOADER_OFFSET,
                &bootloader.data,
            )?;
        }

        // Write partition table
        info!("Writing partition table");
        let partition_table_data = Self::serialize_partition_table(&partition_table)?;
        Self::write_to_flash(&mut flash_image, 0x8000, &partition_table_data)?;

        // Write factory app (second firmware)
        if firmwares.len() >= 2 {
            let factory_app = &firmwares[1];
            info!("Writing factory app: {} bytes", factory_app.size);
            Self::write_to_flash(
                &mut flash_image,
                crate::config::defaults::FACTORY_OFFSET,
                &factory_app.data,
            )?;
        }

        // Write OTA partitions (remaining firmwares)
        for (i, firmware) in firmwares.iter().skip(2).enumerate() {
            // Find corresponding OTA partition
            let ota_name = format!("ota_{}", i);
            if let Some(partition) = partition_table.find(&ota_name) {
                info!(
                    "Writing OTA partition {}: {} bytes at 0x{:X}",
                    i,
                    firmware.size,
                    partition.offset()
                );
                Self::write_to_flash(&mut flash_image, partition.offset(), &firmware.data)?;
            }
        }

        info!(
            "Flash image built successfully: {} bytes",
            flash_image.len()
        );
        Ok(flash_image)
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
    fn test_build_flash_image() -> Result<()> {
        let firmwares = vec![
            create_test_firmware("bootloader", 32 * 1024, 1),
            create_test_firmware("factory_app", 100 * 1024, 2),
            create_test_firmware("ota_app", 200 * 1024, 3),
        ];

        let config = Config {
            flash_size: FlashSize::Size16MB,
            max_ota_partitions: 4,
            ..Default::default()
        };

        let flash_image = ImageBuilder::build_flash_image(&firmwares, &config)?;

        // Should have full flash size worth of data
        assert_eq!(flash_image.len(), 16 * 1024 * 1024);

        // Check bootloader at offset 0
        assert_eq!(&flash_image[0..10], &(0..10).collect::<Vec<_>>());

        // Check factory app at FACTORY_OFFSET
        let factory_offset = crate::config::defaults::FACTORY_OFFSET as usize;
        assert_eq!(
            &flash_image[factory_offset..factory_offset + 10],
            &(0..10).collect::<Vec<_>>()
        );

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

        // Should still create a full flash image with just partition table
        assert_eq!(flash_image.len(), 16 * 1024 * 1024);

        // Partition table should be at offset 0x8000
        let pt_offset = 0x8000 as usize;
        // Should not be all 0xFF in partition table area
        let pt_area = &flash_image[pt_offset..pt_offset + 1000];
        assert!(!pt_area.iter().all(|&b| b == 0xFF));

        Ok(())
    }

    #[test]
    fn test_build_flash_image_only_bootloader() -> Result<()> {
        let firmwares = vec![create_test_firmware("bootloader", 32 * 1024, 1)];

        let config = Config::default();

        let flash_image = ImageBuilder::build_flash_image(&firmwares, &config)?;

        // Should have full flash size
        assert_eq!(flash_image.len(), 16 * 1024 * 1024);

        // Check bootloader at offset 0
        assert_eq!(&flash_image[0..10], &(0..10).collect::<Vec<_>>());

        Ok(())
    }
}
