# Test Artifacts Changelog

All notable changes to the test artifacts organization and management will be documented in this file.

## [Unreleased]

## [1.0.0] - 2025-01-XX

### Added
- Organized test artifacts into dedicated `test_artifacts/` directory
- Implemented category-based numbering system (001-999 ranges)
- Created Windows PowerShell script (`scripts/windows/organize_test_artifacts.ps1`) for automated organization
- Created Bash script (`scripts/organize_test_artifacts.sh`) for Linux/macOS organization
- Added comprehensive README.md with categorization system and best practices
- Implemented automatic number detection to prevent file conflicts
- Created `.gitignore` to control version control tracking of test artifacts
- Added CHANGELOG.md to track test artifacts organization changes
- Defined category ranges for different test types:
  - 001-099: Basic tests
  - 010-099: Archive tests
  - 020-099: Feature toggle tests
  - 100-899: Experimental/development
  - 900-999: Milestone tests

### Changed
- Migrated existing test artifact files from project root:
  - `test.maxion` → `test_artifacts/001_basic_test.maxion`
  - `test_archive.maxion` → `test_artifacts/010_archive_v1.maxion`
  - `test_archive2.maxion` → `test_artifacts/011_archive_v2.maxion`
  - `test_archive3.maxion` → `test_artifacts/012_archive_v3.maxion`
  - `test_archive_on.maxion` → `test_artifacts/020_archive_feature_enabled.maxion`
  - `test_archive_off.maxion` → `test_artifacts/021_archive_feature_disabled.maxion`
  - `test_on.maxion` → `test_artifacts/030_feature_enabled.maxion`
  - `test_off.maxion` → `test_artifacts/031_feature_disabled.maxion`
  - `test_final.maxion` → `test_artifacts/999_final.maxion`

### Removed
- All `.maxion` files from project root directory (moved to test_artifacts/)

## [0.0.1] - Initial State

### Structure
- Test artifacts were stored directly in project root with inconsistent naming
- No standardized naming convention
- Manual organization required
- Examples:
  - `test.maxion` - Basic test
  - `test_archive.maxion`, `test_archive2.maxion`, `test_archive3.maxion` - Archive versions
  - `test_on.maxion`, `test_off.maxion` - Feature toggles
  - `test_archive_on.maxion`, `test_archive_off.maxion` - Archive feature toggles
  - `test_final.maxion` - Final/milestone test

---

## Format

This changelog follows the [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format:
- **Added** - New features
- **Changed** - Changes in existing functionality
- **Deprecated** - Soon-to-be removed features
- **Removed** - Removed features
- **Fixed** - Bug fixes
- **Security** - Security vulnerabilities

## Versioning

- **Major version (X.0.0)** - Significant reorganization or tooling changes
- **Minor version (0.X.0)** - New features or significant enhancements
- **Patch version (0.0.X)** - Bug fixes or minor improvements

## Category Ranges Reference

| Range | Category | Description | Examples |
|-------|----------|-------------|----------|
| 001-099 | Basic | Foundational test cases | Basic tests, simple protection |
| 010-099 | Archive | Archive-specific tests | Version iterations, format tests |
| 020-099 | Features | Feature toggle tests | Enabled/disabled states |
| 100-199 | Experimental | Development experiments | New algorithms, prototypes |
| 200-299 | Compression | Compression algorithm tests | Different compressors, levels |
| 300-399 | Encryption | Encryption/decryption tests | Different ciphers, modes |
| 400-499 | Performance | Performance test outputs | Benchmark executables |
| 500-599 | Edge Cases | Boundary condition tests | Large files, empty files |
| 600-699 | Integration | Multi-component tests | Full system integration |
| 700-799 | Regression | Regression test artifacts | Known good states |
| 800-899 | Platform | Platform-specific tests | Windows, Linux, macOS |
| 900-999 | Milestone | Release milestones | Alpha, beta, final releases |