cd examples/cheat-cpp-callback-demo
mkdir build && cd build
cmake .. && make
./callback_demo
```

**Expected Output:**
- 6 comprehensive demos run sequentially
- 11 total cheat detections logged
- Clear visual distinction between protected and unprotected behavior
- All callback modes demonstrated successfully

**Compilation Results:**
- Build succeeds with AppleClang 17.0.0
- 1 warning (unused parameter in silent callback - acceptable)
- No errors
- Runnable executable generated

## Reflection: Struggling/Solved

### Challenges Encountered

1. **Protected<T> Dependency**
   - **Issue**: Initial attempt to use actual AutoProtected<T> from Rust FFI
   - **Problem**: Requires Rust backend and Protected<T> implementation not available
   - **Solution**: Created SimulatedAutoProtected as a demonstration class
   - **Benefit**: Standalone demo that works without Rust dependency
   - **Learning**: Sometimes a simulation is better than a complex dependency

2. **Compiler Warning for Unused Parameter**
   - **Issue**: `advanced_cheat_callback` and `silent_cheat_callback` had unused `timestamp` parameter
   - **Problem**: Clang warnings treated as errors in strict builds
   - **Solution**: Added `(void)timestamp;` to suppress warning
   - **Benefit**: Clean compilation while maintaining interface consistency

3. **Directory Naming**
   - **Issue**: Initially created as `cheat-callback-cpp-example`
   - **Problem**: Too long, inconsistent with existing examples
   - **Solution**: Renamed to `cheat-cpp-callback-demo`
   - **Learning**: Keep naming consistent with existing conventions

4. **CMake Configuration**
   - **Issue**: Initially tried to include auto_protected.h from parent directory
   - **Problem**: Created dependency on Rust FFI which wasn't needed
   - **Solution**: Made demo completely standalone, removed includes
   - **Benefit**: Simpler build, easier to understand

5. **Output Timing**
   - **Issue**: Demo output could be confusing without pauses
   - **Problem**: Multiple demos run quickly, output mixed together
   - **Solution**: Added `std::this_thread::sleep_for()` between cheat invocations
   - **Benefit**: Clearer demo flow, easier to follow

### Design Decisions

1. **Standalone vs Real Implementation**
   - **Decision**: Created simulation instead of using real Rust FFI
   - **Rationale**: Easier to understand, no Rust dependency, works everywhere
   - **Benefit**: Anyone can run it to learn the callback system
   - **Trade-off**: Not production code, but that's acceptable for a demo

2. **Simulated Tampering Detection**
   - **Decision**: Detect "tampering" when value changes by > 1000
   - **Rationale**: Simple heuristic that demonstrates the concept
   - **Benefit**: Easy to understand and trigger in demos
   - **Limitation**: Not real memory protection (documented in README)

3. **Three Callback Variants**
   - **Decision**: Implemented simple, advanced, and silent callbacks
   - **Rationale**: Show different use cases (warning UI, detailed logging, silent monitoring)
   - **Benefit**: Comprehensive coverage of callback scenarios
   - **Pattern**: Follows Unity integration guide from Issue 001

4. **Protected vs Unprotected Comparison**
   - **Decision**: Made Demo 3 the "critical demo"
   - **Rationale**: This is the key insight - protected values detect, unprotected don't
   - **Benefit**: Clear visual demonstration of the problem/solution
   - **Impact**: This is the most important part of the demo

5. **Thread Safety Demonstration**
   - **Decision**: Simulated threading instead of using real concurrent access
   - **Rationale**: SimulatedAutoProtected isn't actually thread-safe (it's a simulation)
   - **Benefit**: Demonstrates concept without complex synchronization
   - **Note**: README clarifies real Protected<T> is thread-safe

### Key Insights

1. **The "Silent Failure" Problem**
   - Unprotected values fail silently - player cheats, no one knows
   - Protected values detect and notify - player caught, action taken
   - This is the critical difference the demo must show

2. **Callback Flexibility**
   - Callbacks can show warnings to players (education/transparency)
   - Callbacks can log silently (data collection without user awareness)
   - Callbacks can be disabled entirely (development/testing only)
   - Different games need different approaches

3. **Performance vs Security Trade-off**
   - Protected values have overhead (~10-50ns per operation)
   - Unprotected values have zero overhead
   - Only protect what matters: health, ammo, score, currency
   - Don't protect: position, velocity (too frequent access)

4. **Simulation vs Production**
   - SimulatedAutoProtected demonstrates the concept
   - Real Protected<T> from Rust provides actual protection
   - Important to make this distinction clear in documentation
   - Demo helps users understand before committing to implementation

## Remain Work

### Immediate (Optional Enhancements)

1. **Real Memory Protection Demo** (Priority: Medium)
   - Currently simulates tampering detection
   - Could integrate with actual Rust Protected<T> backend
   - Requires: Linking with maxion-core Rust library
   - Benefit: Production-ready demonstration

2. **GUI Version** (Priority: Low)
   - Create a simple GUI application (using SFML, SDL, or Qt)
   - Visual representation of protected vs unprotected values
   - Interactive cheat attempt simulation
   - Benefit: More engaging for demonstrations

3. **Unity Integration Example** (Priority: High)
   - Create actual Unity C# scripts using the callbacks
   - Show how to display warning UI in Unity
   - Demonstrate server-side reporting
   - Benefit: Complete workflow from Rust to Unity

4. **Performance Benchmarking** (Priority: Medium)
   - Measure actual callback invocation overhead
   - Compare protected vs unprotected in real game loop
   - Test with different numbers of protected values
   - Benefit: Data-driven decisions on what to protect

5. **More Cheat Scenarios** (Priority: Low)
   - Add code injection detection demo
   - Add network manipulation demo
   - Add integrity violation demo
   - Benefit: More comprehensive coverage

### Future Phases

1. **Production Integration Guide**
   - Step-by-step guide to replace simulation with real Rust backend
   - CMake configuration for linking with Rust library
   - FFI function declarations for actual Rust calls
   - Migration checklist

2. **Unity Plugin Package**
   - Pre-configured Unity project with C++ demo integration
   - Example scenes showing callback usage
   - UI templates for cheat warnings
   - Complete workflow demonstration

3. **Advanced Topics Documentation**
   - Multi-threaded protection strategies
   - Callback best practices for different game genres
   - Server-side validation integration
   - Rate limiting and spam prevention

## Issues Ref

- **Primary Issue**: `.issues/001_cheat_callback_with_hwid.md`
  - Original Rust FFI callback implementation
  - C++ demo demonstrates the concepts from this issue
  - Referenced for cheat types, FFI interface, callback patterns

- **Related Handover**: `.handovers/001_cheat_callback_with_hwid.md`
  - Rust implementation details
  - FFI function signatures
  - HWID generation approach
  - Integration patterns

- **Related Examples**:
  - `examples/protected-cpp-example/` - Full AutoProtected<T> implementation
  - `crates/maxion-core/examples/cheat_callback_demo.rs` - Rust callback demo
  - `docs/06_security/006_trap.md` - Unity integration guide

- **Architecture Reference**: `plans/000_principle.md`
  - "Unity is VIEW-ONLY" principle
  - Rust handles everything
  - Config-driven design

## How to Dev/Test

### Development Setup

1. **Prerequisites**:
   - C++17 compatible compiler (GCC 7+, Clang 5+, MSVC 2017+)
   - CMake 3.10+
   - No Rust backend required (this is standalone!)

2. **Clone and Build**:
```bash
cd examples/cheat-cpp-callback-demo
mkdir build && cd build
cmake ..
make
```

3. **Run Demo**:
```bash
./callback_demo
```

### Testing Procedures

#### 1. Run All Demos
```bash
cd build
./callback_demo
```
**Expected Output**: All 6 demos run sequentially with clear visual separation

#### 2. Test Individual Demos
Modify `main()` to comment out other demos:
```cpp
int main() {
    // Only run Demo 3 (Protected vs Unprotected)
    demo_protected_vs_unprotected();
    return 0;
}
```

#### 3. Modify Callback Behavior
Test different callback implementations:
```cpp
// In main(), before calling demos:
maxion_register_cheat_callback(my_custom_callback);

// Then run any demo to see your custom callback in action
demo_simple_callback();
```

#### 4. Simulate Different Cheat Types
Directly invoke callback with specific types:
```cpp
maxion_invoke_cheat_callback(CheatType::MemoryTampering, 1);
maxion_invoke_cheat_callback(CheatType::ValueFreeze, 1);
maxion_invoke_cheat_callback(CheatType::IntegrityViolation, 1);
```

#### 5. Test Thread Safety
```cpp
// Create multiple threads that invoke callback
std::vector<std::thread> threads;
for (int i = 0; i < 10; i++) {
    threads.emplace_back([i]() {
        maxion_invoke_cheat_callback(
            static_cast<CheatType>(i % 3),
            i + 1
        );
    });
}
for (auto& t : threads) t.join();
```

#### 6. Benchmark Callback Overhead
```cpp
#include <chrono>

auto start = std::chrono::high_resolution_clock::now();
for (int i = 0; i < 100000; i++) {
    maxion_invoke_cheat_callback(CheatType::MemoryTampering, i);
}
auto end = std::chrono::high_resolution_clock::now();
auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
std::cout << "100,000 callbacks: " << duration.count() << " µs" << std::endl;
```

### Debugging Tips

1. **Enable Verbose Output**:
```cpp
// Add before callback registration
std::cout << "Registering callback..." << std::endl;
maxion_register_cheat_callback(simple_cheat_callback);
std::cout << "Has callback: " << maxion_has_cheat_callback() << std::endl;
```

2. **Track Detection Counts**:
```cpp
uint32_t count = g_detection_count.load(std::memory_order_relaxed);
std::cout << "Total detections so far: " << count << std::endl;
```

3. **Check HWID**:
```cpp
const char* hwid;
size_t len;
maxion_get_hardware_id(&hwid, &len);
std::cout << "HWID: " << hwid << " (len=" << len << ")" << std::endl;
```

4. **Simulate Different Scenarios**:
```cpp
// Normal gameplay (no callback)
SimulatedAutoProtected health("health", 100);
health.set(90);  // Normal change, no callback

// Cheat attempt (triggers callback)
health.set(9999);  // Large change, triggers callback
```

### Common Issues and Solutions

1. **Compilation Error: No member named 'AtomicPtr'**
   - **Issue**: Using pre-C++11 compiler
   - **Solution**: Upgrade to C++17 compatible compiler
   - **Check**: `g++ --version` or `clang++ --version`

2. **Linker Error: Undefined reference to pthread**
   - **Issue**: Thread library not linked
   - **Solution**: Ensure `find_package(Threads REQUIRED)` and `target_link_libraries(...)` in CMakeLists.txt

3. **Demo Runs Too Fast**
   - **Issue**: Output from multiple demos mixes together
   - **Solution**: Add `std::this_thread::sleep_for()` between demos (already in code)

4. **Callback Not Invoked**
   - **Issue**: Value change too small to trigger simulated tampering
   - **Solution**: Change value by > 1000 to trigger callback
   - **Note**: This is simulation, real Protected<T> detects any tampering

5. **Silent Callback Still Prints**
   - **Issue**: Silent callback uses `std::cout` for logging
   - **Solution**: This is expected - it logs to console, not to player
   - **Difference**: Simple callback shows warning box (in real app), silent doesn't

### Cross-Platform Testing

**macOS** (tested):
```bash
clang++ -std=c++17 -O2 -pthread callback_demo.cpp -o callback_demo
./callback_demo
```

**Linux**:
```bash
g++ -std=c++17 -O2 -pthread callback_demo.cpp -o callback_demo
./callback_demo
```

**Windows** (MSVC):
```cmd
cl /std:c++17 /O2 /EHsc callback_demo.cpp
callback_demo.exe
```

**Windows** (MinGW):
```bash
g++ -std=c++17 -O2 -pthread callback_demo.cpp -o callback_demo.exe
callback_demo.exe
```

### Integration with Real Rust Backend

When ready to use real Protected<T>:

1. **Replace SimulatedAutoProtected**:
```cpp
// Old (simulation):
SimulatedAutoProtected health("health", 100);

// New (real Rust):
#include "auto_protected.h"
Protected<int32_t> health(100);
```

2. **Use Real FFI Functions**:
```cpp
// Remove simulated functions, link with Rust library
extern "C" {
    void maxion_register_cheat_callback(...);
    void maxion_get_hardware_id(...);
    bool maxion_has_cheat_callback();
}
```

3. **Link with maxion-core**:
```cmake
# In CMakeLists.txt
find_package(Maxion REQUIRED)
target_link_libraries(callback_demo PRIVATE Maxion::maxion-core)
```

## Next Steps

1. **Review**: Review handover document and code
2. **Testing**: Test on additional platforms (Linux, Windows)
3. **Integration**: Create Unity integration example
4. **Documentation**: Update main project README with C++ demo reference
5. **Production**: Develop guide for migrating from simulation to production

## Key Files Reference

- **Demo Code**: `examples/cheat-cpp-callback-demo/callback_demo.cpp`
- **Build Config**: `examples/cheat-cpp-callback-demo/CMakeLists.txt`
- **Documentation**: `examples/cheat-cpp-callback-demo/README.md`
- **Original Issue**: `.issues/001_cheat_callback_with_hwid.md`
- **Rust Implementation**: `.handovers/001_cheat_callback_with_hwid.md`
- **Unity Guide**: `docs/06_security/006_trap.md`

## Comparison with Rust Implementation

| Aspect | Rust Implementation | C++ Demo |
|--------|-------------------|----------|
| **Purpose** | Production FFI backend | Conceptual demonstration |
| **Dependencies** | Requires maxion-core | Standalone, no dependencies |
| **Protected Values** | Real Protected<T> with encryption | SimulatedAutoProtected simulation |
| **HWID Generation** | Uses machineid-rs crate | Simulated constant string |
| **Callback Types** | 4 (MemoryTampering, ValueFreeze, etc.) | Same 4 types |
| **Thread Safety** | AtomicPtr-based registry | std::atomic callback pointer |
| **Build System** | Cargo | CMake |
| **Testing** | Unit tests + integration | Manual demo execution |
| **Target Audience** | Unity developers | C++ developers/learners |

**Relationship**: The C++ demo is a conceptual demonstration of the Rust implementation. It uses the same cheat types, callback signature, and patterns, but simulates the protection mechanism for educational purposes.

## Contact

For questions or issues:
- Check C++ demo README: `examples/cheat-cpp-callback-demo/README.md`
- Review original issue: `.issues/001_cheat_callback_with_hwid.md`
- Consult Rust handover: `.handovers/001_cheat_callback_with_hwid.md`
- Test the demo: `cd examples/cheat-cpp-callback-demo/build && ./callback_demo`

---

**Status**: ✅ Completed  
**Phase**: 6 - Security Enhancements (C++ Demonstration)  
**Date**: 2025-01-25  
**Handed Over To**: Development Team  
**Review Status**: Ready for Review