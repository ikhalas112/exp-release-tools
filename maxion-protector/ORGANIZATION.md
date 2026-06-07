# Project Organization

This document describes the organizational structure and conventions used throughout the Maxion Protector project.

## Table of Contents

- [Project Structure](#project-structure)
- [File Organization Standards](#file-organization-standards)
- [Naming Conventions](#naming-conventions)
- [Automation Scripts](#automation-scripts)
- [Best Practices](#best-practices)

## Project Structure

```
maxion-protector/
├── .github/                  # GitHub Actions CI/CD workflows
├── benchmark_results/        # Performance benchmark outputs
├── crates/                   # Rust workspace crates
│   ├── maxion-core/         # Core library
│   ├── maxion-injector/     # PE injection logic
│   ├── maxion-packer/       # Packer binary
│   ├── maxion-stub/         # Stub library
│   └── maxion-loader-stub/  # Loader stub
├── docs/                    # Documentation
├── examples/                # Example applications
│   └── hello-world/         # Simple demo
├── plans/                   # Development plans
├── scripts/                 # Utility scripts
│   ├── windows/             # Windows-specific scripts
│   └── *.sh                 # Bash scripts
├── target/                  # Build outputs
│   ├── benchmarks/          # Benchmark metrics
│   ├── e2e/                # End-to-end test outputs
│   └── release/            # Release binaries
├── test_artifacts/          # Test executable outputs
├── test_assets/             # Test input data
├── tests/                   # Integration tests
├── Cargo.toml              # Workspace configuration
├── Cargo.lock              # Dependency lock file
└── README.md               # Project overview
```

## File Organization Standards

### Test Artifacts

The `test_artifacts/` directory contains generated test executables and outputs from the packer.

**Purpose:**
- Reference outputs for regression testing
- Validation test cases for different configurations
- Development milestones and feature demonstrations

**Naming Convention:**
```
{NUMBER}_{DESCRIPTION}.maxion
```

**Numbering Ranges:**

| Range | Category | Description |
|-------|----------|-------------|
| 001-099 | Basic | Foundational test cases |
| 010-099 | Archive | Archive-specific tests |
| 020-099 | Features | Feature toggle tests |
| 100-899 | Experimental | Development experiments |
| 900-999 | Milestone | Release milestones |

**Examples:**
- `001_basic_test.maxion` - Initial basic protection test
- `010_archive_v1.maxion` - Archive test version 1
- `020_archive_feature_enabled.maxion` - Archive with feature enabled
- `999_final.maxion` - Final milestone test

**Documentation:**
- See `test_artifacts/README.md` for detailed usage
- See `test_artifacts/CHANGELOG.md` for version history

### Benchmark Results

The `benchmark_results/` directory contains performance benchmark outputs.

**Purpose:**
- Track performance over time
- Compare before/after optimizations
- Identify performance regressions

**Naming Convention:**
```
{NUMBER}_benchmark.txt
```

**Auto-Numbering:**
- Automatically assigns next available number (001, 002, 003...)
- Prevents file conflicts
- Maintains chronological order

**Documentation:**
- See `benchmark_results/README.md` for usage and performance targets
- See `benchmark_results/CHANGELOG.md` for version history

## Naming Conventions

### Files

**General Rules:**
- Use lowercase letters
- Separate words with underscores (`_`)
- Be descriptive and concise
- Include version numbers for iterations
- Use appropriate file extensions

**Examples:**
```
✅ Good:
- archive_v1.maxion
- feature_enabled_test.maxion
- compression_fast_mode.maxion

❌ Bad:
- ArchiveV1.Maxion
- feature enabled test.maxion
- test-file.maxion
```

### Directories

**General Rules:**
- Use lowercase letters
- Separate words with underscores (`_`)
- Use plural form for collections
- Be descriptive of contents

**Examples:**
```
✅ Good:
- benchmark_results/
- test_artifacts/
- integration_tests/

❌ Bad:
- BenchmarkResults/
- testartifacts/
- integrationTest/
```

## Automation Scripts

### Benchmark Scripts

**Windows (PowerShell):**
```powershell
# Run simple benchmark with auto-numbering
.\scripts\windows\run_simple_bench.ps1

# With custom prefix
.\scripts\windows\run_simple_bench.ps1 -OutputPrefix "optimization"

# Debug build
.\scripts\windows\run_simple_bench.ps1 -Release:$false
```

**Linux/macOS (Bash):**
```bash
# Run simple benchmark with auto-numbering
./scripts/run_simple_bench.sh

# With custom prefix
./scripts/run_simple_bench.sh --prefix "optimization"

# Debug build
./scripts/run_simple_bench.sh --debug
```

### Organization Scripts

**Windows (PowerShell):**
```powershell
# Organize test artifacts
.\scripts\windows\organize_test_artifacts.ps1

# Preview changes
.\scripts\windows\organize_test_artifacts.ps1 -DryRun

# Interactive mode
.\scripts\windows\organize_test_artifacts.ps1 -Interactive
```

**Linux/macOS (Bash):**
```bash
# Organize test artifacts
./scripts/organize_test_artifacts.sh

# Preview changes
./scripts/organize_test_artifacts.sh --dry-run

# Interactive mode
./scripts/organize_test_artifacts.sh --interactive
```

## Best Practices

### 1. Consistency

- Follow established patterns for new files
- Use consistent naming across similar artifacts
- Maintain chronological ordering with numbers

### 2. Documentation

- Document the purpose of test artifacts
- Track changes with changelogs
- Update README files when structure changes

### 3. Cleanup

- Remove obsolete test artifacts regularly
- Archive important milestones
- Use `.gitignore` to control version control

### 4. Automation

- Use provided scripts for organization
- Automate repetitive tasks
- Validate file naming conventions

### 5. Version Control

- Track important test artifacts
- Ignore temporary/experimental files
- Document what's tracked and what's not

### 6. Performance Tracking

- Run benchmarks before major changes
- Compare results using same scenarios
- Document performance targets

## Quick Reference

### Creating a New Test Artifact

1. Run packer with desired configuration
2. Identify appropriate category range
3. Use next available number in range
4. Name with descriptive pattern
5. Move to `test_artifacts/` directory

**Example:**
```powershell
# Create test artifact
cargo run --release -p maxion-packer -- protect `
  --input test.exe `
  --assets assets/ `
  --output test_new.maxion

# Organize
mv test_new.maxion test_artifacts/015_new_feature.maxion
```

### Running Benchmarks

1. Choose appropriate script for your OS
2. Run benchmark - it auto-numbers output
3. Review quick summary displayed
4. Access detailed results in `benchmark_results/`

**Example:**
```powershell
# Run benchmark
.\scripts\windows\run_simple_bench.ps1

# Output: benchmark_results/005_benchmark.txt
```

### Organizing Messy Files

1. Run organization script with dry-run first
2. Review proposed changes
3. Run script to organize files
4. Verify results

**Example:**
```powershell
# Preview
.\scripts\windows\organize_test_artifacts.ps1 -DryRun

# Organize
.\scripts\windows\organize_test_artifacts.ps1 -Interactive
```

## Related Documentation

- [Project README](README.md) - Project overview and getting started
- [Benchmark Plan](plans/003_benchmark.md) - Detailed benchmark implementation
- [Test Artifacts README](test_artifacts/README.md) - Test artifacts detailed guide
- [Benchmark Results README](benchmark_results/README.md) - Benchmark results guide
- [Development Workflow](docs/WORKFLOW.md) - Development process and conventions

## Maintenance

When making structural changes:
1. Update this document
2. Create/update changelogs
3. Update README files
4. Test organization scripts
5. Document breaking changes

---

**Last Updated:** 2025-01-15  
**Maintainer:** Development Team  
**Version:** 1.0.0