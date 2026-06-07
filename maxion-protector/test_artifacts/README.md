# Test Artifacts

This directory contains test artifacts generated during Maxion Protector development, including protected executables and test outputs.

## Purpose

These `.maxion` files are protected executable artifacts created during testing and development. They serve as:
- Reference outputs for regression testing
- Validation test cases for different configurations
- Development milestones and feature demonstrations
- Debug and troubleshooting resources

## Naming Convention

Files follow the pattern:

```
{NUMBER}_{DESCRIPTION}.maxion
```

### Components

- **NUMBER**: Zero-padded 3-digit number for categorization and sorting
- **DESCRIPTION**: Descriptive name indicating test type or purpose
- **EXTENSION**: `.maxion` (Maxion protected executable format)

## Numbering Scheme

### 001-099: Basic Tests
Foundational and simple test cases

| File | Description |
|------|-------------|
| `001_basic_test.maxion` | Initial basic protection test |

### 010-099: Archive Tests
Archive-specific tests and version iterations

| File | Description |
|------|-------------|
| `010_archive_v1.maxion` | Archive test - Version 1 |
| `011_archive_v2.maxion` | Archive test - Version 2 |
| `012_archive_v3.maxion` | Archive test - Version 3 |

### 020-099: Feature Toggle Tests
Tests with specific features enabled/disabled

| File | Description |
|------|-------------|
| `020_archive_feature_enabled.maxion` | Archive feature enabled |
| `021_archive_feature_disabled.maxion` | Archive feature disabled |
| `030_feature_enabled.maxion` | General feature enabled |
| `031_feature_disabled.maxion` | General feature disabled |

### 100-899: Experimental/Development
Development and experimental test outputs (available slots)

### 900-999: Milestone Tests
Important milestone or final test outputs

| File | Description |
|------|-------------|
| `999_final.maxion` | Final/milestone test output |

## Organizing New Test Artifacts

### For Basic Tests
```bash
002_encryption_test.maxion
003_compression_test.maxion
```

### For Version Iterations
```bash
013_archive_v4.maxion
014_archive_v5.maxion
```

### For Feature Tests
Use the pattern:
```
{CATEGORY_ID}_{FEATURE}_{STATE}.maxion
```

Examples:
```bash
022_optimization_enabled.maxion
023_optimization_disabled.maxion
040_compression_fast.maxion
041_compression_best.maxion
```

### For Experimental Tests
```bash
100_experimental_new_algo.maxion
200_dev_branch_test.maxion
```

### For Milestone Tests
```bash
900_alpha_release.maxion
950_beta_release.maxion
999_gold_master.maxion
```

## Categories Reference

| Range | Category | Purpose |
|-------|----------|---------|
| 001-099 | Basic | Simple, foundational tests |
| 100-199 | Archive | Archive-specific tests |
| 200-299 | Features | Feature toggle/option tests |
| 300-399 | Performance | Performance benchmark outputs |
| 400-499 | Compression | Compression algorithm tests |
| 500-599 | Encryption | Encryption/decryption tests |
| 600-699 | Edge Cases | Boundary and error condition tests |
| 700-799 | Integration | Multi-component integration tests |
| 800-899 | Regression | Regression test artifacts |
| 900-999 | Milestone | Release milestones and finals |

## File Types

### .maxion Files
Protected executables containing:
- Original PE structure with injected sections
- `.maxion` section: Encrypted archive data
- `.stub` section: Stub code for VFS implementation
- `.key` section: Obfuscated encryption key
- `.dll_*` sections: Injected DLL data

## Best Practices

### Naming Guidelines
1. **Be descriptive**: Use clear, descriptive names
2. **Stay consistent**: Follow established patterns
3. **Use lowercase**: For readability
4. **Use underscores**: Separate words, not spaces or hyphens
5. **Include version**: For iterative tests (v1, v2, v3)

### Organization Guidelines
1. **Use appropriate range**: Select number range based on test type
2. **Document purpose**: Add comments or metadata if needed
3. **Clean up regularly**: Remove obsolete artifacts
4. **Archive old files**: Move to subdirectories if needed

## Cleanup and Maintenance

### Removing Old Artifacts
```bash
# Remove specific test range
rm test_artifacts/01*.maxion

# Remove category
rm test_artifacts/1*.maxion

# Archive old tests
mkdir test_artifacts/archive_2025
mv test_artifacts/0[0-2][0-9]*.maxion test_artifacts/archive_2025/
```

### Creating Subdirectories
For large numbers of artifacts:
```
test_artifacts/
├── archive_v1/
│   └── 010_archive_v1.maxion
├── feature_tests/
│   ├── 020_feature_enabled.maxion
│   └── 021_feature_disabled.maxion
└── milestones/
    └── 999_final.maxion
```

## Related Files

- `benchmark_results/` - Performance benchmark outputs
- `target/e2e/` - End-to-end test executables
- `target/benchmarks/` - Benchmark metrics and reports

## Metadata Tracking

For additional tracking, consider adding a `manifest.json`:

```json
{
  "001_basic_test.maxion": {
    "date": "2025-01-15",
    "description": "Initial basic protection test",
    "packer_version": "0.1.0",
    "compression": "enabled",
    "encryption": "enabled"
  }
}
```

## Notes

- These files are generated test outputs, not source code
- Files are platform-specific (Windows PE format)
- May contain test data and assets
- Can be large depending on embedded archive size
- Useful for regression testing and debugging
- Should be tracked in version control for important test cases

## Quick Reference

### Creating a new test artifact:
1. Run packer with test configuration
2. Identify appropriate category range
3. Use next available number in range
4. Name with descriptive pattern
5. Move to `test_artifacts/` directory

### Example workflow:
```bash
# Run packer
cargo run --release -p maxion-packer -- protect \
  --input test.exe \
  --assets assets/ \
  --output test_new.maxion

# Organize
mv test_new.maxion test_artifacts/015_new_feature.maxion
```
