# Maxion C++ Callback System Demo (Standalone)

## Overview

This C++ demo showcases the cheat detection callback system with simulated protected values. It demonstrates how protected values can trigger callbacks when tampering is detected, allowing the game to respond appropriately (show warnings, log events, take disciplinary action).

**Note**: This is a **standalone demonstration** that simulates the Rust FFI interface. It does not require the Rust backend to run, making it ideal for understanding the callback system concepts and testing different scenarios.

## Quick Comparison

| Feature | Protected Values | Unprotected Values |
|---------|-----------------|-------------------|
| **Detection** | Detects tampering automatically ✅ | No detection mechanism ❌ |
| **Callback** | Triggers callback on tampering ✅ | No callback triggered ❌ |
| **User Warning** | Shows warning to player ✅ | Silent failure - no warning ❌ |
| **Cheat Outcome** | Logged and preventable ✅ | Cheat succeeds silently ❌ |
| **Production Ready** | Yes (with Rust backend) | No - never use in production |
| **Performance** | Minimal overhead (~10-50ns) | Zero overhead (but insecure) |
| **Thread Safety** | Thread-safe ✅ | Not thread-safe ❌ |

**Key Insight**: Protected values provide automatic cheat detection with minimal performance cost, while unprotected values have zero protection even though they have zero overhead.

## Key Concepts

### Simulated Protected Values
- **SimulatedAutoProtected**: Simulates the behavior of Rust's `Protected<T>` wrapper
- Demonstrates callback triggering when tampering is detected
- In production, this would be replaced with actual `Protected<T>` from Rust backend

### Callback System
- **CheatCallbackFn**: Function pointer type for receiving cheat notifications
- **Thread-safe registration**: Uses atomic operations for safe concurrent access
- **Multiple callback modes**: Warning, silent logging, or no callback at all

### Cheat Types
- `MemoryTampering`: Player modified protected memory addresses
- `ValueFreeze`: Player attempted to freeze game values (e.g., with Cheat Engine)
- `IntegrityViolation`: Code or memory integrity compromised
- `Unknown`: Unclassified cheat type

## Building the Demo

### Prerequisites
- C++17 compatible compiler (GCC 7+, Clang 5+, MSVC 2017+)
- CMake 3.10+ (optional, for build automation)
- **No Rust backend required** - this is a standalone demo!

### Compilation

**Using g++:**
```bash
g++ -std=c++17 -O2 -pthread \
    -I../protected-cpp-example \
    callback_demo.cpp \
    -o callback_demo
```

**Using clang++:**
```bash
clang++ -std=c++17 -O2 -pthread \
    -I../protected-cpp-example \
    callback_demo.cpp \
    -o callback_demo
```

**Using CMake:**
```bash
mkdir build && cd build
cmake ..
make
```

### Running the Demo

```bash
./callback_demo
```

## Demo Scenarios

### Demo 1: Simple Callback with Warning
Shows the basic callback registration and invocation. When cheating is detected, a warning message is displayed to the player.

**Key Points:**
- Register callback with `maxion_register_cheat_callback()`
- Callback receives cheat type, HWID, timestamp, and detection count
- User-friendly warning message shown to player

### Demo 2: Advanced Callback with Type-Specific Actions
Demonstrates different responses based on the type of cheat detected.

**Key Points:**
- MemoryTampering → Log for review
- ValueFreeze → Immediate warning UI
- IntegrityViolation → Disconnect player + Ban review

### Demo 3: Protected vs Unprotected Comparison
**CRITICAL DEMO** - Shows the difference between protected and unprotected values.

**Protected Value (SimulatedAutoProtected):**
- Detects tampering automatically
- Triggers callback → Shows warning ✅
- Player is notified of cheat attempt

**Unprotected Value:**
- No detection mechanism
- Cheat succeeds silently ❌
- No warning shown to player
- Silent failure - player gains unfair advantage

**Note**: In production, replace `SimulatedAutoProtected` with Rust's `Protected<T>`.

### Demo 4: Different Callback Modes
Shows three different modes of operation:

1. **Warning Mode**: Shows visible warnings to player
   - Best for: Player education, transparent anti-cheat
   
2. **Silent Mode**: Logs only, no visible warnings
   - Best for: Data collection, stealth monitoring
   
3. **No Callback**: No protection at all
   - Best for: Development/testing only (NEVER use in production!)

### Demo 5: Multiple Cheat Types
Tests all supported cheat types with appropriate responses:

- MemoryTampering detection
- ValueFreeze detection  
- IntegrityViolation detection

### Demo 6: Thread-Safe Protection
Demonstrates concurrent access to protected values with callback invocation from multiple threads.

**Key Points:**
- Thread-safe callback registration
- Callback can be invoked from any thread
- In production, use `ProtectedSync<T>` for thread-safe value access

## Integration with Rust Backend

This C++ demo **simulates** the Rust FFI interface for demonstration purposes. For production use with the actual Rust backend:

### Required Rust Functions
```rust
#[no_mangle]
pub extern "C" fn maxion_register_cheat_callback(
    callback: Option<extern "C" fn(CheatType, *const u8, usize, u64, u32)>
);

#[no_mangle]
pub extern "C" fn maxion_get_hardware_id(ptr: *mut *const u8, len: *mut usize);

#[no_mangle]
pub extern "C" fn maxion_has_cheat_callback() -> bool;
```

### C++ FFI Declarations
```cpp
extern "C" {
    // Register callback function pointer
    void maxion_register_cheat_callback(
        void(*)(int32_t, const char*, uint64_t, uint32_t)
    );
    
    // Get hardware ID (pointer + length)
    void maxion_get_hardware_id(const char** ptr, size_t* len);
    
    // Check if callback is registered
    bool maxion_has_cheat_callback();
}
```

**This demo includes simulated versions of these functions** for standalone testing.

## Usage Examples

### Basic Usage with Protected Values
```cpp
// In this demo (standalone):
#include "SimulatedAutoProtected.h"

// Register callback
maxion_register_cheat_callback(my_cheat_callback);

// Create protected values
SimulatedAutoProtected health("health", 100);
SimulatedAutoProtected ammo("ammo", 30);
SimulatedAutoProtected score("score", 0);

// Normal gameplay
health.set(75);  // No callback triggered

// Cheat attempt (simulated tampering detection)
// Callback automatically invoked: maxion_invoke_cheat_callback()

// In production (with Rust backend):
#include "auto_protected.h"  // From Rust FFI

// Use Protected<T> instead
Protected<int32_t> health = Protected::new(100);
```

### Custom Callback Implementation
```cpp
extern "C" void my_callback(
    int32_t cheat_type,
    const char* hwid,
    uint64_t timestamp,
    uint32_t detection_count
) {
    // Your custom handling
    switch (cheat_type) {
        case 0: // MemoryTampering
            ShowWarningUI("Cheat detected!");
            SendToServer(hwid, cheat_type, timestamp);
            break;
        case 1: // ValueFreeze
            DisconnectPlayer("Unfair play detected");
            break;
    }
}
```

### Performance Considerations

**Note**: These benchmarks are for the actual Rust `Protected<T>` implementation. This standalone demo has minimal overhead since it simulates protection.

### Protected<T> Overhead (Rust Backend)
- **Read operations**: ~5-10ns overhead (decryption + integrity check)
- **Write operations**: ~15-25ns overhead (encryption + key rotation)
- **Callback invocation**: ~50-100ns (if registered)
- **Thread-safe operations**: ~20-30ns additional overhead

### When to Use Protected Values

✅ **DO Use For:**
- Critical game state (health, ammo, score, currency)
- Multiplayer-sensitive values (rank, matchmaking rating)
- Anti-cheat enabled game modes (competitive, ranked)
- Values accessed less than 10,000 times per second

❌ **DON'T Use For:**
- Position/velocity updates (60-120+ FPS)
- Rendering data (too frequent)
- Temporary calculations (short-lived values)
- Development/debug builds (unless testing anti-cheat)

### Optimization Tips
1. **Use sparingly**: Protect only critical values (health, ammo, score)
2. **Avoid hot loops**: Don't use AutoProtected in tight loops
3. **Batch updates**: Use `ProtectedSync<T>` for thread-safe bulk updates
4. **Selective protection**: Not all values need protection

### Benchmarks
| Operation | Protected | Unprotected | Overhead |
|-----------|-----------|-------------|----------|
| Read (int32_t) | ~5-10ns | ~0.5ns | ~10-20x |
| Write (int32_t) | ~15-25ns | ~0.5ns | ~30-50x |
| Callback invoke | ~50-100ns | N/A | N/A |

**Note**: The overhead is acceptable for game values accessed ~100-1000 times per second.

## Best Practices

### DO ✅
- Protect critical game state (health, ammo, score, currency)
- Register callback at game initialization
- Log all cheat detections for server analysis
- Use type-specific responses for different cheats
- Test with actual cheat tools (Cheat Engine, ArtMoney)
- Use this standalone demo to prototype and test callback logic
- Replace SimulatedAutoProtected with Rust's Protected<T> in production
- Monitor callback performance in production builds
- Implement graceful degradation if callback registration fails
- Use HWID for player identification and server-side validation

### DON'T ❌
- Protect frequently accessed values (position, velocity)
- Register/unregister callbacks frequently (once at startup is best)
- Ignore cheat detections (even minor ones can indicate larger issues)
- Use in performance-critical rendering loops
- Rely solely on client-side detection (server validation is essential)
- Assume callbacks will always be invoked (handle edge cases)
- Block the main thread in callback implementations
- Store sensitive data in unprotected values

## Troubleshooting

### Callback Not Invoked
**Symptoms**: Cheats not detected, no warnings shown

**Solutions**:
1. Verify callback is registered: `maxion_has_cheat_callback()`
2. Check if AutoProtected values are actually being used
3. Ensure Rust backend is compiled with debug checks enabled
4. Verify cheat detection threshold (default: 5 violations)

### Performance Issues
**Symptoms**: Frame rate drops, lag

**Solutions**:
1. Reduce number of AutoProtected values
2. Use ProtectedSync<T> for shared state
3. Profile to identify slow operations
4. Consider caching frequently accessed values

### False Positives
**Symptoms**: Warnings shown for legitimate gameplay

**Solutions**:
1. Increase detection threshold (via CheatDetector::init)
2. Review callback logic for edge cases
3. Use silent mode initially, then enable warnings after tuning
4. Collect logs and analyze patterns

## Real-World Testing

### Testing with Cheat Engine
1. Run the demo
2. Open Cheat Engine
3. Attach to the demo process
4. Scan for health/ammo/score values
5. Modify a value
6. Verify callback is invoked with warning

**Note**: Since this is a simulation, you'll need to trigger the callback manually or use the demo's built-in scenarios. In production with Rust's `Protected<T>`, this would happen automatically.

### Expected Results
- **With Protected values**: Callback triggered, warning shown ✅
- **Without Protected values**: No detection, silent failure ❌

### Testing with Multiple Threads
1. Create multiple game instances
2. Simulate concurrent gameplay
3. Attempt to modify values from different threads
4. Verify thread-safety of callback system

## References

- **Rust Implementation**: `crates/maxion-core/src/cheat_callback.rs`
- **Rust Demo**: `crates/maxion-core/examples/cheat_callback_demo.rs`
- **AutoProtected Header**: `examples/protected-cpp-example/auto_protected.h`
- **Related Issue**: `.issues/001_cheat_callback_with_hwid.md`
- **Security Guide**: `docs/06_security/006_trap.md`

### Related Examples
- `examples/protected-cpp-example/auto_protected_demo.cpp` - Full Protected<T> usage (requires Rust backend)
- `crates/maxion-core/examples/cheat_callback_demo.rs` - Rust version of callback system

## License

This demo is part of the Maxion Protector project.

## Contributing

When modifying this demo:
1. Keep it focused on callback and protection concepts
2. Add comments explaining the "why" not just "what"
3. Test on all target platforms (Windows, macOS, Linux)
4. Update README for any new features or behavior changes
5. Remember this is a **standalone simulation** - clarify what's real vs simulated
6. Keep the simulation simple and focused on demonstrating callback behavior

## Support

For questions or issues:
1. Check the main README
2. Review the Rust implementation for reference
3. Consult the security documentation
4. Open an issue with reproduction steps

---

**Remember**: AutoProtected values + Callbacks = Robust anti-cheat system! 🛡️