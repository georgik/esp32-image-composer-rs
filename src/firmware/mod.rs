use anyhow::{Result, anyhow};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct FirmwareBinary {
    pub name: String,
    pub path: PathBuf,
    pub data: Vec<u8>,
    pub size: u32,
    pub prefix: u32,
}

impl FirmwareBinary {
    pub fn new(name: String, path: PathBuf, data: Vec<u8>, prefix: u32) -> Self {
        let size = data.len() as u32;
        Self {
            name,
            path,
            data,
            size,
            prefix,
        }
    }
}

pub struct FirmwareLoader;

impl FirmwareLoader {
    pub fn load_from_directory<P: AsRef<Path>>(dir: P) -> Result<Vec<FirmwareBinary>> {
        let dir_path = dir.as_ref();
        if !dir_path.exists() {
            return Err(anyhow!("Firmware directory does not exist: {:?}", dir_path));
        }

        let mut firmware_map = BTreeMap::new();

        // First, find all .bin files and extract prefixes
        for entry in WalkDir::new(dir_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file() && e.path().extension().map_or(false, |ext| ext == "bin")
            })
        {
            let path = entry.path();
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow!("Invalid filename: {:?}", path))?;

            if let Some(prefix) = Self::extract_prefix(filename)? {
                if let Ok(data) = fs::read(path) {
                    let name = Self::extract_name(filename)?;
                    let firmware = FirmwareBinary::new(name, path.to_path_buf(), data, prefix);
                    firmware_map.insert(prefix, firmware);
                }
            }
        }

        // Convert to sorted vector by prefix
        let mut firmwares: Vec<FirmwareBinary> = firmware_map.into_values().collect();
        firmwares.sort_by_key(|f| f.prefix);

        if firmwares.is_empty() {
            return Err(anyhow!(
                "No valid firmware files found in directory: {:?}",
                dir_path
            ));
        }

        log::info!("Loaded {} firmware files", firmwares.len());
        for firmware in &firmwares {
            log::debug!(
                "{}: {} bytes (prefix: {:02})",
                firmware.name,
                firmware.size,
                firmware.prefix
            );
        }

        Ok(firmwares)
    }

    fn extract_prefix(filename: &str) -> Result<Option<u32>> {
        // Extract numerical prefix from filename (e.g., "01-bootloader.bin" -> 1)
        let parts: Vec<&str> = filename.split('-').collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let first_part = parts[0];
        if first_part.len() >= 2 {
            if let Ok(prefix) = u32::from_str_radix(first_part, 10) {
                return Ok(Some(prefix));
            }
        }

        Ok(None)
    }

    fn extract_name(filename: &str) -> Result<String> {
        // Remove prefix and extension to get clean name
        let name_without_ext = filename.trim_end_matches(".bin");
        if let Some(dash_pos) = name_without_ext.find('-') {
            Ok(name_without_ext[dash_pos + 1..].to_string())
        } else {
            Ok(name_without_ext.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_prefix() {
        assert_eq!(
            FirmwareLoader::extract_prefix("01-bootloader.bin").unwrap(),
            Some(1)
        );
        assert_eq!(
            FirmwareLoader::extract_prefix("02-app.bin").unwrap(),
            Some(2)
        );
        assert_eq!(
            FirmwareLoader::extract_prefix("10-final.bin").unwrap(),
            Some(10)
        );
        assert_eq!(
            FirmwareLoader::extract_prefix("no-prefix.bin").unwrap(),
            None
        );
        assert_eq!(
            FirmwareLoader::extract_prefix("abc-bootloader.bin").unwrap(),
            None
        );
    }

    #[test]
    fn test_extract_name() {
        assert_eq!(
            FirmwareLoader::extract_name("01-bootloader.bin").unwrap(),
            "bootloader"
        );
        assert_eq!(
            FirmwareLoader::extract_name("02-esp32-p4-graphical-bootloader.bin").unwrap(),
            "esp32-p4-graphical-bootloader"
        );
        assert_eq!(
            FirmwareLoader::extract_name("firmware.bin").unwrap(),
            "firmware"
        );
    }

    #[test]
    fn test_load_from_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let firmware_dir = temp_dir.path();

        // Create test firmware files
        fs::write(firmware_dir.join("01-bootloader.bin"), b"bootloader_data")?;
        fs::write(firmware_dir.join("02-app.bin"), b"app_data")?;
        fs::write(firmware_dir.join("10-final.bin"), b"final_data")?;
        fs::write(firmware_dir.join("ignored.txt"), b"ignored")?;

        let firmwares = FirmwareLoader::load_from_directory(firmware_dir)?;

        assert_eq!(firmwares.len(), 3);
        assert_eq!(firmwares[0].name, "bootloader");
        assert_eq!(firmwares[0].prefix, 1);
        assert_eq!(firmwares[1].name, "app");
        assert_eq!(firmwares[1].prefix, 2);
        assert_eq!(firmwares[2].name, "final");
        assert_eq!(firmwares[2].prefix, 10);

        Ok(())
    }

    #[test]
    fn test_load_from_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = FirmwareLoader::load_from_directory(temp_dir.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No valid firmware files found")
        );
    }

    #[test]
    fn test_load_from_nonexistent_directory() {
        let result = FirmwareLoader::load_from_directory("/nonexistent/path");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }
}
