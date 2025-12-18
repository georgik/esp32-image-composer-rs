# ESP32 Image Composer (Rust)

A command-line tool for creating ESP32 flash images from firmware binaries with dynamic partition table generation. Built specifically for ESP32-P4 multi-stage bootloader systems and production firmware deployment.

## Purpose

This tool addresses the limitations of static partition tables by dynamically generating ESP-IDF compatible partition tables based on actual firmware sizes.

## ESP32-P4 Support

The ESP32 Image Composer provides comprehensive support for ESP32-P4 devices with specific optimizations and considerations:

### ESP32-P4 Memory Layout

ESP32-P4 uses a different memory layout compared to other ESP32 variants:

```
ESP32-P4 Flash Layout (from ESP-IDF flash_args):
0x02000  +-------------------+  [Bootloader - 32KB]
         |   bootloader      |
0x10000  +-------------------+  [Partition Table - 4KB]
         | partition-table   |
0x11000  +-------------------+  [Padding/Reserved]
0x20000  +-------------------+  [Factory App - 576KB+]
         |  factory app      |
         | (02-*.bin)       |
0x120000 +-------------------+  [OTA_0 - variable]
         |       ota_0       |
         |    (03-*.bin)     |
         +-------------------+  [...]
```

### Key ESP32-P4 Differences

- **Bootloader Offset**: `0x2000` (unlike ESP32's `0x1000` or ESP32-S3's `0x0`)
- **Partition Table**: `0x10000` (provides more space between bootloader and partitions)
- **Factory App**: `0x20000` (64KB-aligned for optimal performance)
- **Checksum Handling**: Preserves original ESP-IDF calculated checksums

### ESP32-P4 Best Practices

1. **Preserve Original Checksums**: The tool automatically preserves checksums calculated by ESP-IDF build system
2. **64KB Alignment**: All application partitions are automatically aligned to 64KB boundaries
3. **Minimal Padding**: Uses minimal image size by default, avoiding unnecessary 0xFF padding
4. **Header Preservation**: Maintains original ESP32 image headers and flags

### ESP32-P4 Development Workflow

```bash
# Build ESP32-P4 firmware (in your ESP-IDF project)
idf.py build

# Use this tool to create flash image
esp32-image-composer-rs \
    --firmware-dir ./build \
    --output ./firmware-flash.bin \
    --flash-size 16MB \
    --verbose

# Verify the image before flashing
esp32-image-composer-rs inspect ./firmware-flash.bin --verify-checksums

# Flash to device
esptool.py --chip esp32p4 write_flash 0x2000 ./firmware-flash.bin
```

### ESP32-P4 Troubleshooting

**"Checksum failure. Calculated 0x00 stored 0xff"**
- Ensure firmware binaries are built with ESP-IDF v5.5+
- Verify original binaries have valid checksums before using this tool
- The tool preserves checksums - if original is broken, result will be broken

**"invalid header: 0x40b2d7a0"**
- Check that bootloader is placed at correct offset `0x2000`
- Verify ESP32-P4 specific layout requirements
- Use `--dry-run` to preview layout before generating

**Image hash failed**
- Factory application checksums are now preserved automatically
- Ensure original app binaries are complete and uncorrupted
- Check that partition table correctly identifies factory partition

## Features

- **Dynamic Partition Generation**: Automatically sizes partitions based on firmware binary requirements
- **ESP-IDF Compatibility**: Uses the proven `esp_idf_part` crate for ESP-IDF compliant partition tables
- **Multi-OTA Support**: Handles up to 16 OTA partitions with proper alignment
- **Flexible Flash Sizes**: Support for 8MB, 16MB, and 32MB flash configurations
- **Comprehensive Validation**: Partition overlap detection and flash space validation
- **CLI Interface**: Full-featured command-line tool with multiple operation modes

## Quick Start

### Building

```bash
cargo build --release
```

### Basic Usage

```bash
# Generate flash image from firmware directory
./target/release/esp32-image-composer-rs --firmware-dir ./firmwares

# Dry run to see partition layout without creating files
./target/release/esp32-image-composer-rs --firmware-dir ./firmwares --dry-run
```

### Firmware Directory Structure

The tool expects firmware binaries with numerical prefixes:

```
firmwares/
├── 01-bootloader.bin              # Stage 2 bootloader
├── 02-esp32-p4-graphical-bootloader.bin  # Factory application
├── 03-fw1.bin                   # First OTA application
├── 04-fw2.bin         # Second OTA application
└── ...
```

Files are processed in numerical order:
- `01-*.bin` → Bootloader partition
- `02-*.bin` → Factory application partition
- `03+*.bin` → OTA_0, OTA_1, ... partitions

## Commands

### Generate Flash Image (Default)

```bash
esp32-image-composer-rs [OPTIONS] <COMMAND>
```

**Options:**
- `--firmware-dir <DIR>`: Directory containing firmware binaries (default: `firmwares`)
- `--output <FILE>`: Output flash image file (default: `combined-image.bin`)
- `--flash-size <SIZE>`: Flash size [8MB|16MB|32MB] (default: `16MB`)
- `--max-ota-partitions <N>`: Maximum OTA partitions (default: `16`)
- `--verbose`: Enable detailed logging
- `--dry-run`: Show operations without creating files

### Information Commands

**Firmware Info:**
```bash
esp32-image-composer-rs info [--show-sizes]
```

**Validation:**
```bash
esp32-image-composer-rs validate [--detailed]
```

**Partition Table Only:**
```bash
esp32-image-composer-rs partition-table [--output <FILE>] [--csv]
```

## Architecture

### Core Components

```
src/
├── lib.rs              # Library interface and exports
├── main.rs             # CLI entry point and command handling
├── cli/mod.rs          # Command-line argument definitions
├── config/mod.rs       # Configuration management and ESP32-P4 constants
├── esp32.rs            # ESP32-P4 specific processing and checksum handling
├── firmware/mod.rs     # Firmware discovery and loading logic
├── partition/mod.rs    # Partition table generation using esp_idf_part
└── image/mod.rs        # Flash image assembly and binary operations
```

### ESP32-P4 Processing Module

The `esp32.rs` module contains ESP32-P4 specific optimizations:

- **Checksum Preservation**: Maintains original ESP-IDF calculated checksums
- **Header Processing**: Handles ESP32-P4 specific image headers without modification
- **Layout Verification**: Ensures proper ESP32-P4 memory layout and alignment
- **Error Recovery**: Detailed error messages for ESP32-P4 specific issues

### Partition Generation Algorithm

1. **Base Partitions**: Create essential partitions (bootloader, partition table, NVS, otadata)
2. **Factory Partition**: Place second firmware as factory application
3. **OTA Partitions**: Generate OTA_0...OTA_N partitions for remaining firmwares
4. **Size Alignment**: Align partitions to 64KB boundaries for optimal performance
5. **Validation**: Check for overlaps and flash space constraints

### Memory Layout

The tool automatically detects and uses the correct layout for different ESP32 variants:

**ESP32-P4 Layout (default):**
```
0x02000  +-------------------+  [Bootloader - 32KB]
         |   bootloader      |
0x10000  +-------------------+  [Partition Table - 4KB]
         | partition-table   |
0x20000  +-------------------+  [Factory App - variable]
         |      factory      |
         |    (02-*.bin)     |
0x120000 +-------------------+  [OTA_0 - variable]
         |       ota_0       |
         |    (03-*.bin)     |
         +-------------------+  [...]
```

**Traditional ESP32 Layout:**
```
0x01000  +-------------------+  [Bootloader - 32KB]
         |   bootloader      |
0x08000  +-------------------+  [Partition Table - 4KB]
         | partition-table   |
0x09000  +-------------------+  [NVS - 24KB]
         |       nvs         |
0x0F000  +-------------------+  [OTA Data - 8KB]
         |     otadata       |
0x18000  +-------------------+  [Factory App - 1MB+]
         |      factory      |
         |    (02-*.bin)     |
0x118000 +-------------------+  [OTA_0 - variable]
         |       ota_0       |
         |    (03-*.bin)     |
         +-------------------+  [...]
```

## Dependencies

### Runtime Dependencies

- `esp_idf_part = "0.6"`: ESP-IDF partition table handling
- `clap = "4.0"`: Command-line argument parsing
- `anyhow` & `thiserror`: Error handling
- `log` & `env_logger`: Logging infrastructure
- `colored`: Terminal output formatting
- `serde`: Configuration serialization

### Development Dependencies

- `tempfile`: Test file management
- Standard testing tools

## Development

### Running Tests

```bash
cargo test                    # All tests
cargo test --lib             # Library tests only
cargo test --bin esp32-image-composer-rs  # Integration tests
```

### Adding Features

1. **New Partition Types**: Extend `PartitionGenerator::generate_table()`
2. **Validation Rules**: Add checks in `PartitionGenerator::validate_partition_table()`
3. **CLI Commands**: Extend `cli/mod.rs` with new subcommands
4. **Output Formats**: Modify `image/mod.rs` serialization

### Code Style

- Use `Result<T>` for error handling with `anyhow::Error`
- Prefer library patterns over executable-only code
- Add comprehensive unit tests for new functionality
- Follow Rust naming conventions and idioms

## Troubleshooting

### ESP32-P4 Specific Issues

**"Checksum failure. Calculated 0x00 stored 0xff"**
- Ensure firmware binaries are built with ESP-IDF v5.5+
- Verify original binaries have valid checksums before using this tool
- The tool preserves checksums - if original is broken, result will be broken
- Check ESP-IDF build logs for any checksum warnings during compilation

**"invalid header: 0x40b2d7a0"**
- Bootloader is not at correct offset `0x2000` for ESP32-P4
- Verify ESP32-P4 specific layout requirements are met
- Use `--dry-run` to preview layout before generating final image
- Check that firmware follows ESP32-P4 naming convention (01-*.bin for bootloader)

**"Image hash failed - image is corrupt"**
- Factory application checksums are preserved automatically in latest versions
- Ensure original app binaries are complete and uncorrupted
- Check that partition table correctly identifies factory partition
- Verify ESP-IDF build completed successfully without errors

**"Partition table validation failed"**
- ESP32-P4 requires 64KB alignment for application partitions
- Check that factory app starts at `0x20000` (64KB boundary)
- Verify partition table MD5 checksum is calculated correctly
- Use `inspect` command to analyze partition table structure

### Common Issues

**"Firmware directory does not exist"**
- Ensure the `--firmware-dir` path is correct
- Directory must contain `*.bin` files with numerical prefixes
- For ESP32-P4: expect `01-bootloader.bin`, `02-*.bin`, etc.

**"Partition overlaps with partition"**
- Check firmware sizes vs available flash space
- Consider using larger `--flash-size` or reducing `--max-ota-partitions`
- ESP32-P4 has larger minimum partition sizes due to alignment requirements

**"Not enough flash space"**
- Increase flash size with `--flash-size 32MB`
- Reduce number of OTA partitions
- Check for unusually large firmware files (common with graphical applications)

### Debug Mode

```bash
# ESP32-P4 specific debugging
esp32-image-composer-rs --verbose --firmware-dir ./firmwares --dry-run

# Inspect generated image
esp32-image-composer-rs inspect ./output.bin --verify-checksums --detailed

# Validate firmware before processing
esp32-image-composer-rs validate --firmware-dir ./firmwares --detailed
```

## Integration with Build Systems

### Makefile Integration

```makefile
.PHONY: flash-image
flash-image:
	cd esp32-image-composer-rs && cargo build --release
	./esp32-image-composer-rs/target/release/esp32-image-composer-rs \
		--firmware-dir ../firmwares \
		--output ../combined-image.bin

.PHONY: validate-firmware
validate-firmware:
	cd esp32-image-composer-rs && cargo run -- \
		--firmware-dir ../firmwares validate --detailed
```

### ESP-IDF Integration

Add to `CMakeLists.txt`:

```cmake
find_program(ESP32_IMAGE_COMPOSER
    NAMES esp32-image-composer-rs
    PATHS ${CMAKE_CURRENT_SOURCE_DIR}/esp32-image-composer-rs/target/release
)

add_custom_command(
    OUTPUT ${PROJECT_BINARY_DIR}/combined-image.bin
    COMMAND ${ESP32_IMAGE_COMPOSER}
        --firmware-dir ${CMAKE_CURRENT_SOURCE_DIR}/firmwares
        --output ${PROJECT_BINARY_DIR}/combined-image.bin
    DEPENDS ${FIRMWARE_FILES}
    VERBATIM
)
```

## License

MIT

