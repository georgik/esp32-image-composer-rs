# ESP32 Image Composer (Rust)

A command-line tool for creating ESP32 flash images from firmware binaries with dynamic partition table generation. Built specifically for ESP32-P4 multi-stage bootloader systems.

## Purpose

This tool addresses the limitations of static partition tables by dynamically generating ESP-IDF compatible partition tables based on actual firmware sizes. It replaces the JavaScript-based implementation with a more reliable, well-tested solution.

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
├── config/mod.rs       # Configuration management and constants
├── firmware/mod.rs     # Firmware discovery and loading logic
├── partition/mod.rs    # Partition table generation using esp_idf_part
└── image/mod.rs        # Flash image assembly and binary operations
```

### Partition Generation Algorithm

1. **Base Partitions**: Create essential partitions (bootloader, partition table, NVS, otadata)
2. **Factory Partition**: Place second firmware as factory application
3. **OTA Partitions**: Generate OTA_0...OTA_N partitions for remaining firmwares
4. **Size Alignment**: Align partitions to 64KB boundaries for optimal performance
5. **Validation**: Check for overlaps and flash space constraints

### Memory Layout

```
0x00000  +-------------------+  [Bootloader - 32KB]
         |   bootloader      |
0x08000  +-------------------+  [Partition Table - 4KB]
         | partition-table   |
0x09000  +-------------------+  [NVS - 24KB]
         |       nvs         |
0x11000  +-------------------+  [Padding]
0x0F000  +-------------------+  [OTA Data - 8KB]
         |     otadata       |
0x17000  +-------------------+  [Padding]
0x18000  +-------------------+  [Factory App - 1MB+]
         |      factory      |
         |    (02-*.bin)     |
0x118000 +-------------------+  [OTA_0 - variable]
         |       ota_0       |
         |    (03-*.bin)     |
         +-------------------+  [OTA_1 - variable]
         |       ota_1       |
         |    (04-*.bin)     |
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

### Common Issues

**"Firmware directory does not exist"**
- Ensure the `--firmware-dir` path is correct
- Directory must contain `*.bin` files with numerical prefixes

**"Partition overlaps with partition"**
- Check firmware sizes vs available flash space
- Consider using larger `--flash-size` or reducing `--max-ota-partitions`

**"Not enough flash space"**
- Increase flash size with `--flash-size 32MB`
- Reduce number of OTA partitions
- Check for unusually large firmware files

### Debug Mode

```bash
esp32-image-composer-rs --verbose --firmware-dir ./firmwares --dry-run
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

This project is part of the ESP32-P4 Graphical Bootloader project. See parent project for license details.

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Ensure all tests pass and the code follows the existing style conventions.
