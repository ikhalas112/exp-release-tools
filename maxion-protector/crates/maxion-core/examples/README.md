# Maxion Core Examples

This directory contains example programs demonstrating various features of the Maxion Core library.

## Examples

### 1. `auto_protected_demo.rs`

**Demonstrates:** Automatic protection using the `#[auto_protected]` procedural macro.

**Features:**
- Automatic generation of protected struct wrappers
- Support for various data types (i32, f32, tuples)
- Complex game state management
- Best practices for using protected values

**Run:**
```bash
cargo run --package maxion-core --example auto_protected_demo
```

**What you'll see:**
- Creating players with protected health, ammo, and score
- Using protected float fields for positions
- Managing complex game state
- Cheat detection mechanism explanation

**Best for:** Learning how to use the `#[auto_protected]` attribute in your game.

---

### 2. `cheat_callback_demo.rs`

**Demonstrates:** FFI callback system for cheat detection notifications with HWID.

**Features:**
- Register callbacks to receive cheat notifications
- Hardware ID (HWID) generation and caching
- Thread-safe protected values
- Simple and advanced callback handlers
- Interactive mode for testing with real Cheat Engine

**Run:**
```bash
cargo run --package maxion-core --example cheat_callback_demo
```

**What you'll see:**
- Hardware ID generation (32-char MD5 hash)
- Simple callback registration
- Advanced callback with detailed handling
- Thread-safe protection across multiple threads
- Simulated cheat detection alerts
- Interactive mode for real Cheat Engine testing

**Interactive Mode:**
The demo includes an interactive mode that lets you test with real Cheat Engine:

1. Run the example
2. When you reach Demo 5, open Cheat Engine
3. Select the `cheat_callback_demo` process
4. Search for value `100` (4 bytes)
5. Modify the value to `999`
6. Press ENTER in the terminal
7. Watch the callback be triggered!

**Best for:** Learning how to integrate cheat detection with Unity or other game engines.

---

### 3. `protected_bench.rs`

**Demonstrates:** Performance benchmarking of protected values.

**Features:**
- Measure performance overhead of protected values
- Compare with unprotected values
- Impact of cheat detection on performance
- Encryption/decryption overhead

**Run:**
```bash
cargo run --package maxion-core --example protected_bench
```

**What you'll see:**
- Benchmark results for various operations
- Performance comparisons
- Impact of encryption key rotation
- Recommendations for optimal usage

**Best for:** Understanding performance characteristics and optimizing your anti-cheat implementation.

---

### 4. `simple_bench.rs`

**Demonstrates:** Simple benchmarking for basic operations.

**Run:**
```bash
cargo run --package maxion-core --example simple_bench
```

**Best for:** Quick performance testing and baseline measurements.

---

### 5. `unprotected_bench.rs`

**Demonstrates:** Baseline performance without protection.

**Run:**
```bash
cargo run --package maxion-core --example unprotected_bench
```

**Best for:** Comparing protected vs unprotected performance.

---

## Running Examples

### Run a specific example:
```bash
cargo run --package maxion-core --example <example_name>
```

### Run all examples:
```bash
# List all available examples
cargo run --package maxion-core --example --help

# Run each example individually
cargo run --package maxion-core --example auto_protected_demo
cargo run --package maxion-core --example cheat_callback_demo
cargo run --package maxion-core --example protected_bench
```

### Run with logging:
```bash
RUST_LOG=debug cargo run --package maxion-core --example cheat_callback_demo
```

### Build in release mode (better performance for benchmarks):
```bash
cargo run --release --package maxion-core --example protected_bench
```

---

## Testing Anti-Cheat with Cheat Engine

### Prerequisites
- **Cheat Engine** (or ArtMoney, GameGuardian, etc.)
- Running example executable

### Step-by-Step Guide

#### For `cheat_callback_demo.rs`:

1. **Start the example:**
   ```bash
   cargo run --package maxion-core --example cheat_callback_demo
   ```

2. **Wait for Demo 5 (Interactive Mode)**

3. **Open Cheat Engine:**
   - Click "Select a process to open"
   - Select `cheat_callback_demo` process

4. **Find the value:**
   - Value: `100` (current health)
   - Type: `4 Bytes`
   - Click "First Scan"

5. **Modify the value:**
   - Select the found address
   - Change value to `999`
   - Click OK

6. **Trigger detection:**
   - Go back to the terminal
   - Press ENTER
   - Watch the callback alert! 🎉

#### For `auto_protected_demo.rs`:

The same process works - just look for any of the displayed values (health, ammo, score).

### What's Happening

1. **Trap Value**: Cheat Engine finds the plain-text trap value (e.g., `100`)
2. **Modification**: Player modifies it to `999`
3. **Detection**: Next `get()` call detects mismatch (`100 != 999`)
4. **Callback**: CheatDetector invokes your callback with details
5. **Response**: Unity/game engine decides what to do (kick, ban, warn, etc.)

---

## Key Concepts

### Protected Values

Protected values defend against memory tampering using:

- **Trap Value**: Plain text, easily searchable by Cheat Engine
- **Real Value**: Encrypted with XOR, hard to find and modify
- **Encryption Key**: Rotated on each write to prevent freezing

### Cheat Detection

When a protected value is read:
1. Decrypt the real value
2. Read the trap value
3. Compare them - if mismatch → CHEAT DETECTED!

### Callback System

The callback system allows your game to respond to cheats:

1. **Register** a callback function during initialization
2. **Receive** notifications when cheats are detected
3. **Handle** the notification (log, kick, ban, etc.)

### Hardware ID (HWID)

- **Generated** using `machineid-rs` crate
- **Cached** after first generation (expensive operation)
- **Unique** per machine (System ID + CPU ID + Drive Serial)
- **Encrypted** with MD5 for UUID compatibility (32 chars)

---

## Performance Considerations

### Protected Values Overhead

```
Regular i32:      ~364 µs for 100,000 operations
Protected<i32>:   ~28.7 ms for 100,000 operations
Overhead:         ~78x slower (7,800%)
```

**Why the overhead?**
- Volatile memory operations (prevents optimization)
- XOR encryption/decryption
- Random key generation
- Trap value comparison

### Best Practices

1. **Protect only critical values:**
   - ✅ Health, ammo, score, currency
   - ❌ Temporary variables, counters, flags

2. **Batch updates:**
   ```rust
   // Good: Update once per frame
   health.set(new_health);
   
   // Avoid: Update in tight loops
   for _ in 0..1000 {
       health.set(health.get() + 1); // Slow!
   }
   ```

3. **Use ProtectedSync only when needed:**
   ```rust
   // Good: Thread-safe for shared state
   let shared_health = Arc::new(ProtectedSync::new(100));
   
   // Better: Use regular Protected for single-threaded
   let player_health = Protected::new(100);
   ```

---

## Further Reading

- **Documentation**: `docs/06_security/006_trap.md`
- **Architecture**: `plans/000_principle.md`
- **Issue Tracker**: `.issues/001_cheat_callback_with_hwid.md`
- **Handover**: `.handovers/001_cheat_callback_with_hwid.md`

---

## Troubleshooting

### Example doesn't run

**Problem:** Command not found
```
error: no example target named 'xxx'
```

**Solution:** List available examples
```bash
cargo run --package maxion-core --example --help
```

### Cheat Engine can't find process

**Problem:** Process not listed in Cheat Engine
```
No process found with name 'cheat_callback_demo'
```

**Solution:**
1. Make sure the example is running
2. Click "Refresh" in Cheat Engine
3. Look for terminal/console process name

### Cheat detection not triggered

**Problem:** Modified value but no callback
```
Reading Health: 999 (no alert)
```

**Solution:**
1. Check if callback is registered: `maxion_has_cheat_callback()`
2. Check if trap checking is enabled: `is_trap_enabled()`
3. Verify you're modifying the trap value (not real value)
4. Ensure you're calling `get()` after modification

### Performance is too slow

**Problem:** Frame rate drops when using protected values

**Solution:**
1. Profile to identify bottlenecks
2. Reduce number of protected values
3. Batch updates instead of frequent individual updates
4. Consider using regular values for non-critical data

---

## Contributing

To add a new example:

1. Create file: `crates/maxion-core/examples/your_example.rs`
2. Add `use maxion_core::*;` at the top
3. Demonstrate a specific feature or use case
4. Add comments explaining key concepts
5. Update this README with a new section
6. Test: `cargo run --package maxion-core --example your_example`

---

**Last Updated:** 2025-01-25  
**Maxion Core Version:** 0.1.0