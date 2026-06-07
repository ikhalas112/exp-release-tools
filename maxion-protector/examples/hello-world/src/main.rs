use std::io::Read;
use std::path::Path;

fn main() {
    // Initialize profiler
    maxion_profiler::init_metrics("target/e2e/benchmark_metrics.json");

    println!("Hello Asset Loader Benchmark");
    println!("============================");
    println!();

    // Run benchmark scenarios
    match std::env::args().nth(1).as_deref() {
        Some("small") => benchmark_small_asset(),
        Some("medium") => benchmark_medium_bundle(),
        Some("large") => benchmark_large_stream(),
        Some("mixed") => benchmark_mixed_load(),
        _ => benchmark_all(),
    }

    // Flush metrics
    if let Err(e) = maxion_profiler::flush_metrics() {
        eprintln!("Warning: Failed to flush metrics: {}", e);
    }

    println!();
    println!("✓ Benchmark completed!");
}

/// Benchmark 1: Small Asset Load (sirref.png - 240 bytes)
fn benchmark_small_asset() {
    println!("=== Scenario 1: Small Asset Load ===");
    println!("Asset: sirref.png (240 bytes)");
    println!();

    // Use assets directory relative to executable location
    let exe_path = std::env::current_exe().expect("Failed to get executable path");
    let exe_dir = exe_path
        .parent()
        .expect("Failed to get executable directory");
    let asset_path = exe_dir.join("assets").join("sirref.png");

    // Measure file existence check
    let timer = maxion_profiler::Timer::start("check_exists");
    let exists = asset_path.exists();
    timer.stop();
    assert!(exists, "Asset not found");

    // Measure metadata retrieval
    let timer = maxion_profiler::Timer::start("get_metadata");
    let metadata = std::fs::metadata(&asset_path).expect("Failed to read metadata");
    timer.stop();

    println!("✓ File size: {} bytes", metadata.len());

    // Measure image loading
    let timer = maxion_profiler::Timer::start("load_image");
    let img = image::open(asset_path).expect("Failed to load image");
    timer.stop();

    println!("✓ Image loaded successfully");
    println!("  Dimensions: {}x{}", img.width(), img.height());
    println!("  Color type: {:?}", img.color());
}

/// Benchmark 2: Medium Asset Bundle (multiple small files)
fn benchmark_medium_bundle() {
    println!("=== Scenario 2: Medium Asset Bundle ===");
    println!("Loading multiple small assets...");
    println!();

    // Use assets directory relative to executable location
    let exe_path = std::env::current_exe().expect("Failed to get executable path");
    let exe_dir = exe_path
        .parent()
        .expect("Failed to get executable directory");
    let assets_dir = exe_dir.join("assets");

    // Create multiple test files if they don't exist
    create_test_assets(&assets_dir, 10, 1024); // 10 files of 1KB each

    // Measure loading all files
    let timer = maxion_profiler::Timer::start("load_bundle");

    let mut total_size = 0u64;
    let mut files_loaded = 0;

    for entry in std::fs::read_dir(assets_dir).expect("Failed to read assets dir") {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();

        if path.is_file()
            && path
                .extension()
                .map(|e| e == "png" || e == "bin")
                .unwrap_or(false)
        {
            // Measure individual file load
            let file_timer =
                maxion_profiler::Timer::start(&format!("load_file_{}", path.display()));

            if let Ok(metadata) = std::fs::metadata(&path) {
                total_size += metadata.len();
            }

            // Try to load as image or read as binary
            if path.extension().map(|e| e == "png").unwrap_or(false) {
                let _ = image::open(&path);
            } else {
                let _ = std::fs::read(&path);
            }

            file_timer.stop();
            files_loaded += 1;
        }
    }
    timer.stop();

    println!("✓ Loaded {} files", files_loaded);
    println!("✓ Total size: {} bytes", total_size);
    println!(
        "✓ Average per file: {} bytes",
        total_size / files_loaded.max(1)
    );
}

/// Benchmark 3: Large Asset Stream (simulated large file reads)
fn benchmark_large_stream() {
    println!("=== Scenario 3: Large Asset Stream ===");
    println!("Simulating large asset streaming...");
    println!();

    // Use assets directory relative to executable location
    let exe_path = std::env::current_exe().expect("Failed to get executable path");
    let exe_dir = exe_path
        .parent()
        .expect("Failed to get executable directory");
    let large_asset_path = exe_dir.join("assets").join("large_asset.bin");

    // Create large test file if it doesn't exist (5MB)
    if !large_asset_path.exists() {
        create_large_asset(&large_asset_path, 5 * 1024 * 1024);
    }

    let metadata = std::fs::metadata(&large_asset_path).expect("Failed to read metadata");
    println!(
        "✓ File size: {} bytes ({:.2} MB)",
        metadata.len(),
        metadata.len() as f64 / (1024.0 * 1024.0)
    );
    println!();

    // Measure streaming read in chunks
    const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks
    let total_chunks = (metadata.len() as usize + CHUNK_SIZE - 1) / CHUNK_SIZE;

    let timer = maxion_profiler::Timer::start("stream_read_large");
    let mut total_bytes_read = 0usize;
    let mut chunks_read = 0;

    let file = std::fs::File::open(&large_asset_path).expect("Failed to open file");
    let mut reader = std::io::BufReader::new(file);

    let mut buffer = vec![0u8; CHUNK_SIZE];

    loop {
        let chunk_timer = maxion_profiler::Timer::start("read_chunk");
        let bytes_read = reader.read(&mut buffer).expect("Failed to read chunk");
        chunk_timer.stop();

        if bytes_read == 0 {
            break;
        }

        total_bytes_read += bytes_read;
        chunks_read += 1;

        if chunks_read % 10 == 0 {
            println!(
                "  Progress: {} / {} chunks ({:.1}%)",
                chunks_read,
                total_chunks,
                (chunks_read * 100 / total_chunks.max(1))
            );
        }
    }
    timer.stop();

    println!(
        "✓ Streamed {} bytes in {} chunks",
        total_bytes_read, chunks_read
    );
}

/// Benchmark 4: Mixed Asset Load (various sizes and types)
fn benchmark_mixed_load() {
    println!("=== Scenario 4: Mixed Asset Load ===");
    println!("Loading assets of various sizes and types...");
    println!();

    // Use assets directory relative to executable location
    let exe_path = std::env::current_exe().expect("Failed to get executable path");
    let exe_dir = exe_path
        .parent()
        .expect("Failed to get executable directory");
    let assets_dir = exe_dir.join("assets");

    // Create mixed test assets
    create_test_assets(&assets_dir, 5, 512); // 5 small files (512B)
    create_test_assets(&assets_dir, 3, 8192); // 3 medium files (8KB)
    create_test_assets(&assets_dir, 2, 65536); // 2 large files (64KB)

    // Categorize assets by size
    let mut small_files = Vec::new();
    let mut medium_files = Vec::new();
    let mut large_files = Vec::new();

    for entry in std::fs::read_dir(assets_dir).expect("Failed to read assets dir") {
        let entry = entry.expect("Failed to read entry");
        let path = entry.path();

        if path.is_file() {
            if let Ok(metadata) = std::fs::metadata(&path) {
                let size = metadata.len();

                if size <= 1024 {
                    small_files.push(path);
                } else if size <= 1024 * 10 {
                    medium_files.push(path);
                } else {
                    large_files.push(path);
                }
            }
        }
    }

    println!("Found:");
    println!("  {} small files (<= 1KB)", small_files.len());
    println!("  {} medium files (1KB - 10KB)", medium_files.len());
    println!("  {} large files (> 10KB)", large_files.len());
    println!();

    // Load and measure each category
    load_asset_category(&small_files, "small");
    load_asset_category(&medium_files, "medium");
    load_asset_category(&large_files, "large");
}

/// Run all benchmark scenarios
fn benchmark_all() {
    println!("Running all benchmark scenarios...");
    println!();

    benchmark_small_asset();
    println!();

    benchmark_medium_bundle();
    println!();

    benchmark_large_stream();
    println!();

    benchmark_mixed_load();
}

/// Helper: Load a category of assets and measure timing
fn load_asset_category(files: &[std::path::PathBuf], category: &str) {
    let timer = maxion_profiler::Timer::start(&format!("load_{}_category", category));
    let mut total_bytes = 0u64;

    for path in files {
        if let Ok(metadata) = std::fs::metadata(path) {
            total_bytes += metadata.len();
        }

        let file_timer = maxion_profiler::Timer::start(&format!("load_{}", category));

        // Try to load based on extension
        match path.extension().and_then(|e| e.to_str()) {
            Some("png") => {
                let _ = image::open(path);
            }
            Some(_) => {
                let _ = std::fs::read(path);
            }
            None => {
                let _ = std::fs::read(path);
            }
        }

        file_timer.stop();
    }

    timer.stop();

    println!(
        "✓ {} category: {} files, {} total bytes",
        category,
        files.len(),
        total_bytes
    );
}

/// Helper: Create test assets
fn create_test_assets(dir: &Path, count: usize, size: usize) {
    // Don't create test assets - they should exist already
    // This function is kept for reference but disabled
    // to avoid creating files in examples/ during benchmarking

    for i in 0..count {
        let path = dir.join(format!("test_asset_{}_{}.bin", size, i));
        if !path.exists() {
            println!("  Warning: Test asset not found: {}", path.display());
            println!("  Run ./scripts/generate_test_assets.sh to create test assets");
        }
    }
}

/// Helper: Create large test asset
fn create_large_asset(path: &Path, _size: usize) {
    // Don't create test assets - they should exist already
    // This function is kept for reference but disabled
    // to avoid creating files in examples/ during benchmarking

    if !path.exists() {
        println!("Warning: Large test asset not found: {}", path.display());
        println!("  Run ./scripts/generate_test_assets.sh to create test assets");
    }
}
