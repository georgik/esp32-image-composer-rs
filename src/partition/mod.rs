use crate::Result;
use crate::config::{Config, defaults::*};
use crate::firmware::FirmwareBinary;
use anyhow::anyhow;
use esp_idf_part::{AppType, DataType, Flags, Partition, PartitionTable, SubType, Type};
use log::info;

pub struct PartitionGenerator;

impl PartitionGenerator {
    pub fn generate_table(firmwares: &[FirmwareBinary], config: &Config) -> Result<PartitionTable> {
        info!(
            "Generating partition table for {} firmwares",
            firmwares.len()
        );

        let mut partitions = Vec::new();

        // Add bootloader partition (ESP32-P4 specific offset)
        partitions.push(Partition::new(
            "bootloader".to_string(),
            Type::App,
            SubType::App(AppType::Factory),
            BOOTLOADER_OFFSET,
            BOOTLOADER_SIZE,
            Flags::empty(),
        ));

        // Add partition table
        partitions.push(Partition::new(
            "partition-table".to_string(),
            Type::Data,
            SubType::Data(DataType::Phy),
            PARTITION_TABLE_OFFSET,
            PARTITION_TABLE_SIZE,
            Flags::empty(),
        ));

        // Add NVS partition
        partitions.push(Partition::new(
            "nvs".to_string(),
            Type::Data,
            SubType::Data(DataType::Nvs),
            NVS_OFFSET,
            NVS_SIZE,
            Flags::empty(),
        ));

        // Add OTA data partition
        partitions.push(Partition::new(
            "otadata".to_string(),
            Type::Data,
            SubType::Data(DataType::Ota),
            OTADATA_OFFSET,
            OTADATA_SIZE,
            Flags::empty(),
        ));

        // Add factory partition (first firmware should be bootloader, second should be factory app)
        if firmwares.len() >= 2 {
            let factory_firmware = &firmwares[1]; // Second firmware is factory app
            let factory_size = Self::align_up(factory_firmware.size, OTA_ALIGNMENT);

            partitions.push(Partition::new(
                "factory".to_string(),
                Type::App,
                SubType::App(AppType::Factory),
                FACTORY_OFFSET,
                factory_size,
                Flags::empty(),
            ));
        }

        // Calculate remaining space for OTA partitions
        let flash_size = config.flash_size.size_bytes();
        let mut current_offset = FACTORY_OFFSET + FACTORY_SIZE;

        // Add OTA partitions for remaining firmwares (starting from index 2)
        let ota_partitions: Vec<_> = firmwares
            .iter()
            .skip(2) // Skip bootloader and factory
            .take(config.max_ota_partitions)
            .enumerate()
            .map(|(i, firmware)| {
                let ota_size = Self::align_up(firmware.size, OTA_ALIGNMENT);
                let partition_name = format!("ota_{}", i);
                let ota_subtype = match i {
                    0 => AppType::Ota_0,
                    1 => AppType::Ota_1,
                    2 => AppType::Ota_2,
                    3 => AppType::Ota_3,
                    4 => AppType::Ota_4,
                    5 => AppType::Ota_5,
                    6 => AppType::Ota_6,
                    7 => AppType::Ota_7,
                    8 => AppType::Ota_8,
                    9 => AppType::Ota_9,
                    10 => AppType::Ota_10,
                    11 => AppType::Ota_11,
                    12 => AppType::Ota_12,
                    13 => AppType::Ota_13,
                    14 => AppType::Ota_14,
                    15 => AppType::Ota_15,
                    _ => AppType::Ota_0, // Default fallback
                };

                (partition_name, ota_subtype, ota_size, firmware.size)
            })
            .collect();

        for (name, subtype, aligned_size, actual_size) in ota_partitions {
            if current_offset + aligned_size > flash_size {
                return Err(anyhow!(
                    "Not enough flash space for OTA partition '{}' ({} bytes needed, {} bytes available)",
                    name,
                    aligned_size,
                    flash_size - current_offset
                ));
            }

            partitions.push(Partition::new(
                name.clone(),
                Type::App,
                SubType::App(subtype),
                current_offset,
                aligned_size,
                Flags::empty(),
            ));

            info!(
                "Added OTA partition '{}' at 0x{:X} ({} bytes, firmware: {} bytes)",
                name, current_offset, aligned_size, actual_size
            );

            current_offset += aligned_size;
        }

        let partition_table = PartitionTable::new(partitions);

        // Validate the partition table
        Self::validate_partition_table(&partition_table, flash_size)?;

        info!("Partition table generated successfully");
        Ok(partition_table)
    }

    fn validate_partition_table(table: &PartitionTable, flash_size: u32) -> Result<()> {
        // Check if any partitions exceed flash size
        for partition in table.partitions() {
            if partition.offset() + partition.size() > flash_size {
                return Err(anyhow!(
                    "Partition '{}' exceeds flash size (ends at 0x{:X}, flash size: 0x{:X})",
                    partition.name(),
                    partition.offset() + partition.size(),
                    flash_size
                ));
            }
        }

        // Check for overlapping partitions
        let mut partitions: Vec<_> = table.partitions().into_iter().collect();
        partitions.sort_by_key(|p| p.offset());

        for window in partitions.windows(2) {
            let current = window[0];
            let next = window[1];

            if current.offset() + current.size() > next.offset() {
                return Err(anyhow!(
                    "Partition '{}' overlaps with partition '{}'",
                    current.name(),
                    next.name()
                ));
            }
        }

        Ok(())
    }

    fn align_up(size: u32, alignment: u32) -> u32 {
        ((size + alignment - 1) / alignment) * alignment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FlashSize;
    use std::path::PathBuf;

    fn create_test_firmware(name: &str, size: u32, prefix: u32) -> FirmwareBinary {
        FirmwareBinary::new(
            name.to_string(),
            PathBuf::from(format!("{}.bin", name)),
            vec![0; size as usize],
            prefix,
        )
    }

    #[test]
    fn test_align_up() {
        assert_eq!(PartitionGenerator::align_up(1000, 1024), 1024);
        assert_eq!(PartitionGenerator::align_up(1024, 1024), 1024);
        assert_eq!(PartitionGenerator::align_up(1025, 1024), 2048);
        assert_eq!(
            PartitionGenerator::align_up(64 * 1024 + 1, 64 * 1024),
            128 * 1024
        );
    }

    #[test]
    fn test_generate_basic_partition_table() -> Result<()> {
        let firmwares = vec![
            create_test_firmware("bootloader", 32 * 1024, 1),
            create_test_firmware("factory_app", 500 * 1024, 2),
        ];

        let config = Config {
            flash_size: FlashSize::Size16MB,
            max_ota_partitions: 4,
            ..Default::default()
        };

        let table = PartitionGenerator::generate_table(&firmwares, &config)?;

        // Should have bootloader, partition-table, nvs, otadata, factory
        assert_eq!(table.partitions().len(), 5);

        let partition_names: Vec<_> = table.partitions().into_iter().map(|p| p.name()).collect();
        assert!(partition_names.iter().any(|s| s == "bootloader"));
        assert!(partition_names.iter().any(|s| s == "partition-table"));
        assert!(partition_names.iter().any(|s| s == "nvs"));
        assert!(partition_names.iter().any(|s| s == "otadata"));
        assert!(partition_names.iter().any(|s| s == "factory"));

        Ok(())
    }

    #[test]
    fn test_generate_partition_table_with_ota() -> Result<()> {
        let firmwares = vec![
            create_test_firmware("bootloader", 32 * 1024, 1),
            create_test_firmware("factory_app", 500 * 1024, 2),
            create_test_firmware("ota_app_1", 800 * 1024, 3),
            create_test_firmware("ota_app_2", 1_200 * 1024, 4),
        ];

        let config = Config {
            flash_size: FlashSize::Size16MB,
            max_ota_partitions: 4,
            ..Default::default()
        };

        let table = PartitionGenerator::generate_table(&firmwares, &config)?;

        // Should have bootloader, partition-table, nvs, otadata, factory, ota_0, ota_1
        assert_eq!(table.partitions().len(), 7);

        let partition_names: Vec<_> = table.partitions().into_iter().map(|p| p.name()).collect();
        assert!(partition_names.iter().any(|s| s == "ota_0"));
        assert!(partition_names.iter().any(|s| s == "ota_1"));

        Ok(())
    }

    #[test]
    fn test_partition_overflow() {
        let firmwares = vec![
            create_test_firmware("bootloader", 32 * 1024, 1),
            create_test_firmware("factory_app", 500 * 1024, 2),
            create_test_firmware("huge_app", 20 * 1024 * 1024, 3), // 20MB app
        ];

        let config = Config {
            flash_size: FlashSize::Size16MB, // Only 16MB flash
            max_ota_partitions: 1,
            ..Default::default()
        };

        let result = PartitionGenerator::generate_table(&firmwares, &config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Not enough flash space")
        );
    }

    #[test]
    fn test_empty_firmwares() -> Result<()> {
        let config = Config::default();
        let result = PartitionGenerator::generate_table(&[], &config);
        // Should work - empty firmware set just creates basic partitions
        assert!(result.is_ok());
        Ok(())
    }

    #[test]
    fn test_only_bootloader() {
        let firmwares = vec![create_test_firmware("bootloader", 32 * 1024, 1)];

        let config = Config::default();
        let result = PartitionGenerator::generate_table(&firmwares, &config);
        // This should work but won't have a factory partition
        assert!(result.is_ok());
    }
}
