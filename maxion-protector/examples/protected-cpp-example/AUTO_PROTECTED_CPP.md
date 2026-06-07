# AutoProtected<T> - C++ Anti-Cheat Protection

## Overview

`AutoProtected<T>` provides automatic anti-cheat protection for C++ game state by wrapping values in encrypted storage with trap mechanisms. This eliminates the need to manually implement protection logic, making it easy to secure critical game values from memory tampering by cheat engines like Cheat Engine and ArtMoney.

**Key Features:**
- ✅ Automatic encryption of protected values
- ✅ Trap values that detect memory modifications
- ✅ Key rotation on every write (prevents value freezing)
- ✅ Automatic cheat detection and reporting
- ✅ Simple API: `.get()` and `.set()`
- ✅ Works with all supported types (int32_t, int64_t, uint32_t, uint64_t, float)
- ✅ Copyable and assignable (creates independent protected instances)

**Document ID:** AUTO_PROTECTED_CPP  
**Status:** Production Ready ✅  
**Last Updated:** 2025-02-19  
**Version:** 1.0  
**Authors:** Maxion Team

## How It Works

### Protection Mechanism

Each `AutoProtected<T>` value stores data using the same underlying `Protected<T>` implementation:

1. **Dual Storage:**
   - **Trap Value**: Plain text, easily searchable by cheat engines
   - **Real Value**: XOR encrypted, hard to find and modify

2. **Read Operation:**
   ```
   user calls value.get()
   → Decrypt real value using current encryption key
   → Read trap value (volatile to prevent optimization)
   → Compare them
   → If mismatch → CHEAT DETECTED!
   → Return real value
   ```

3. **Write Operation:**
   ```
   user calls value.set(75)
   → Generate new random encryption key (prevents freezing)
   → Encrypt new value with new key
   → Update both trap and real values
   ```

### Attack Detection

**Memory Scanner Attack:**
```
Cheat Engine scans for value "100"
→ Finds both trap (100) and encrypted real value
→ User modifies trap to 999
→ Next read detects mismatch (100 != 999)
→ CHEAT DETECTED! ✓
```

**Value Freeze Attack:**
```
Cheat Engine freezes health at 100 (god mode)
→ Game updates health to 75 (key rotates)
→ Cheat Engine writes 100 back to trap
→ Next read detects mismatch (75 != 100)
→ CHEAT DETECTED! ✓
```

## Usage

### Basic Usage

```cpp
#include "auto_protected.h"

using namespace Maxion;

// Create protected values
AutoProtected<int32_t> health(100);
AutoProtected<int32_t> ammo(30);
AutoProtected<int32_t> score(0);
AutoProtected<float> x(1.0f);

// Read values (automatically checks for tampering)
int32_t current_health = health.get();
int32_t current_ammo = ammo.get();
float current_x = x.get();

// Update values (rotates encryption key)
health.set(75);
ammo.set(25);
x.set(10.5f);
```

### Struct with Protected Fields (Manual Approach - Recommended)

```cpp
struct Player {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;
    AutoProtected<int32_t> score;

    // Constructor
    Player(int32_t h, int32_t a, int32_t s)
        : health(h), ammo(a), score(s) {}

    // Optional convenience getters/setters
    int32_t get_health() const { return health.get(); }
    void set_health(int32_t value) { health.set(value); }

    int32_t get_ammo() const { return ammo.get(); }
    void set_ammo(int32_t value) { ammo.set(value); }

    int32_t get_score() const { return score.get(); }
    void set_score(int32_t value) { score.set(value); }
};

// Usage
Player player(100, 30, 0);
int32_t hp = player.health.get();  // Direct access
player.set_health(75);             // Via setter
```

### Using AUTO_PROTECTED_FIELD Macro

```cpp
#include "auto_protected.h"

using namespace Maxion;

struct Enemy {
    AUTO_PROTECTED_FIELD(int32_t, health);
    AUTO_PROTECTED_FIELD(int32_t, damage);
    AUTO_PROTECTED_FIELD(int32_t, speed);

    Enemy(int32_t h, int32_t d, int32_t s)
        : health(h), damage(d), speed(s) {}

    AUTO_PROTECTED_GETTER_SETTER(Enemy, int32_t, health)
    AUTO_PROTECTED_GETTER_SETTER(Enemy, int32_t, damage)
    AUTO_PROTECTED_GETTER_SETTER(Enemy, int32_t, speed)
};

// Usage
Enemy enemy(150, 25, 10);
int32_t hp = enemy.get_health();
enemy.set_damage(30);
```

### Complex Game State

```cpp
struct GameState {
    AutoProtected<int32_t> player_health;
    AutoProtected<int32_t> player_ammo;
    AutoProtected<int32_t> player_score;
    AutoProtected<int32_t> currency;

    // Non-protected state (for performance)
    int32_t animation_frame;
    int32_t temp_counter;

    GameState(int32_t ph, int32_t pa, int32_t ps, int32_t cur)
        : player_health(ph), player_ammo(pa), player_score(ps)
        , currency(cur), animation_frame(0), temp_counter(0) {}

    void take_damage(int32_t damage) {
        int32_t current = player_health.get();
        player_health.set((current - damage) > 0 ? current - damage : 0);
    }

    void fire_weapon() {
        int32_t current = player_ammo.get();
        if (current > 0) {
            player_ammo.set(current - 1);
        }
    }
};

// Usage
GameState game(100, 30, 0, 1000);
game.take_damage(10);
game.fire_weapon();
```

### Multiple Independent Instances

```cpp
struct Player {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;

    Player(int32_t h, int32_t a) : health(h), ammo(a) {}
};

// Each instance is independently protected
Player player1(100, 30);
Player player2(50, 20);

// Modifications to player1 don't affect player2
player1.health.set(90);
assert(player1.health.get() == 90);
assert(player2.health.get() == 50);
```

## API Reference

### `AutoProtected<T>`

Template wrapper class that provides automatic protection for values.

#### Constructors

```cpp
// Construct with initial value
AutoProtected<int32_t> value(100);

// Construct with default value
AutoProtected<int32_t> value;

// Copy constructor (creates new protected instance)
AutoProtected<int32_t> value2(value1);

// Move constructor
AutoProtected<int32_t> value3(std::move(value1));
```

#### Methods

```cpp
// Get the protected value (checks for tampering)
T get() const;

// Set a new value (rotates encryption key)
void set(const T& value);

// Get raw protected value (advanced usage)
Protected<T>* get_raw();

// Get raw protected value (const version, advanced usage)
const Protected<T>* get_raw() const;
```

#### Supported Types

`AutoProtected<T>` works with all types supported by `Protected<T>`:

- `int32_t` - 32-bit signed integer
- `int64_t` - 64-bit signed integer
- `uint32_t` - 32-bit unsigned integer
- `uint64_t` - 64-bit unsigned integer
- `float` - 32-bit floating point

### Macros

#### `AUTO_PROTECTED_FIELD(TYPE, NAME)`

Mark a struct field as auto-protected.

```cpp
struct Player {
    AUTO_PROTECTED_FIELD(int32_t, health);
    AUTO_PROTECTED_FIELD(int32_t, ammo);
};
```

Expands to:
```cpp
struct Player {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;
};
```

#### `AUTO_PROTECTED_GETTER_SETTER(STRUCT_NAME, TYPE, NAME)`

Generate getter and setter methods for a protected field.

```cpp
struct Player {
    AUTO_PROTECTED_FIELD(int32_t, health);
    
    Player(int32_t h) : health(h) {}
    
    AUTO_PROTECTED_GETTER_SETTER(Player, int32_t, health)
};
```

Expands to:
```cpp
inline int32_t get_health() const {
    return health.get();
}

inline void set_health(const int32_t& value) {
    health.set(value);
}
```

## Security Benefits

### What It Protects Against

✅ **Detected Attacks:**
- Memory scanning for values (Cheat Engine, ArtMoney)
- Value modification (changing health to 999)
- Value freezing (god mode, unlimited ammo)
- Basic pointer chasing

⚠️ **Partially Protected:**
- Advanced pointer chain attacks (requires more effort)
- Code injection attacks (requires additional measures)

❌ **Not Protected Against:**
- Network manipulation (requires server-side validation)
- Graphics hacks (wallhacks, aimbots)
- Input manipulation (macros, scripting)

### Security Guarantees

1. **Encryption**: All values are XOR encrypted with random keys
2. **Integrity**: Trap values detect any modification
3. **Freshness**: Keys rotate on every write (prevents freezing)
4. **Detection**: All accesses check for tampering

### Attack Scenarios

**Scenario 1: Memory Scanner**
```
Attacker scans memory for value "100"
→ Finds trap value "100" and encrypted real value
→ Modifies trap to 999
→ Next read detects mismatch (100 != 999)
→ CHEAT DETECTED! ✓
```

**Scenario 2: Value Freeze**
```
Attacker freezes health at 100 (god mode)
→ Game updates health to 75 (key rotates)
→ Cheat Engine writes 100 back to trap
→ Next read detects mismatch (75 != 100)
→ CHEAT DETECTED! ✓
```

**Scenario 3: Direct Memory Write**
```
Attacker writes directly to memory location
→ Overwrites trap value but not encrypted real value
→ Next read detects mismatch
→ CHEAT DETECTED! ✓
```

## Performance Considerations

### Performance Overhead

`AutoProtected<T>` has significant overhead compared to regular values:

```
Regular int32_t:      ~364 µs for 100,000 operations
Protected<int32_t>:   ~28.7 ms for 100,000 operations
Overhead:             ~78x slower (7,800%)
```

**Why the overhead?**
- Volatile memory operations (prevents compiler optimization)
- XOR encryption/decryption on every access
- Random key generation on every write
- Trap value comparison on every read

### Performance Impact Analysis

| Field Count | Read Operations/sec | Write Operations/sec | Total Overhead |
|-------------|-------------------|---------------------|----------------|
| 1 field     | ~3,500 ops/ms     | ~3,400 ops/ms       | ~78x           |
| 5 fields     | ~700 ops/ms       | ~680 ops/ms         | ~78x per field |
| 10 fields    | ~350 ops/ms       | ~340 ops/ms         | ~78x per field |

### Best Practices for Performance

1. **Protect only critical values:**
   ```cpp
   // ✅ Good: Protect cheatable values
   struct CriticalState {
       AutoProtected<int32_t> health;
       AutoProtected<int32_t> ammo;
       AutoProtected<int32_t> score;
   };
   
   // ❌ Bad: Protect everything
   struct Everything {
       AutoProtected<int32_t> health;
       AutoProtected<int32_t> ammo;
       AutoProtected<int32_t> temp1;  // Unnecessary
       AutoProtected<int32_t> temp2;  // Unnecessary
   };
   ```

2. **Batch updates when possible:**
   ```cpp
   // ✅ Good: Update once per frame
   player.health.set(new_health);
   player.ammo.set(new_ammo);
   
   // ❌ Bad: Update in tight loops
   for (int32_t i = 0; i < 1000; i++) {
       player.health.set(player.health.get() + 1); // Very slow!
   }
   ```

3. **Separate protected and unprotected state:**
   ```cpp
   // ✅ Good: Separate concerns
   struct GameState {
       AutoProtected<int32_t> health;
       AutoProtected<int32_t> ammo;
       int32_t animation_frame;  // Not protected
       int32_t temp_counter;     // Not protected
   };
   ```

## Comparison with Rust `#[auto_protected]`

### Rust Version

```rust
use maxion_core::auto_protected;

#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}

// Usage
let player = Player::new(100, 30, 0);
player.health.set(75);
```

### C++ Version (Manual Approach - Recommended)

```cpp
#include "auto_protected.h"

using namespace Maxion;

struct Player {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;
    AutoProtected<int32_t> score;

    Player(int32_t h, int32_t a, int32_t s)
        : health(h), ammo(a), score(s) {}
};

// Usage
Player player(100, 30, 0);
player.health.set(75);
```

### Key Differences

| Feature | Rust | C++ |
|---------|------|-----|
| Syntax | `#[auto_protected]` attribute | Manual struct definition |
| Constructor | Auto-generated | Manual implementation |
| Boilerplate | Zero | Minimal (constructor + getters) |
| Type Safety | Compile-time checked | Compile-time checked |
| Flexibility | All-or-nothing protection | Can mix protected/unprotected |
| Build System | Cargo (native) | Any C++ build system |
| Macro Magic | Procedural macros | Preprocessor macros |

### When to Use Each

- **Rust**: Use `#[auto_protected]` for clean, zero-boilerplate syntax
- **C++**: Use `AutoProtected<T>` manually for clarity and control

Both provide equivalent protection and security!

## Testing

### Unit Testing

```cpp
#include "auto_protected.h"
#include <cassert>
#include <iostream>

void test_auto_protected() {
    AutoProtected<int32_t> value(100);
    
    // Test initial value
    assert(value.get() == 100);
    
    // Test set and get
    value.set(200);
    assert(value.get() == 200);
    
    // Test float values
    AutoProtected<float> fvalue(1.5f);
    assert(fvalue.get() == 1.5f);
    fvalue.set(2.5f);
    assert(fvalue.get() == 2.5f);
    
    std::cout << "✅ All AutoProtected tests passed!" << std::endl;
}

void test_multiple_instances() {
    AutoProtected<int32_t> value1(100);
    AutoProtected<int32_t> value2(200);
    
    value1.set(150);
    
    assert(value1.get() == 150);
    assert(value2.get() == 200); // Unchanged
    
    std::cout << "✅ Multiple instances are independent!" << std::endl;
}

void test_copy_and_assignment() {
    AutoProtected<int32_t> value1(100);
    AutoProtected<int32_t> value2(value1); // Copy constructor
    AutoProtected<int32_t> value3(0);
    value3 = value1; // Assignment
    
    value1.set(200);
    
    assert(value1.get() == 200);
    assert(value2.get() == 100); // Unchanged
    assert(value3.get() == 100); // Unchanged
    
    std::cout << "✅ Copy and assignment work correctly!" << std::endl;
}

int main() {
    test_auto_protected();
    test_multiple_instances();
    test_copy_and_assignment();
    return 0;
}
```

### Running Tests

```bash
# Compile
g++ -std=c++17 -o test_auto_protected test_auto_protected.cpp -I/path/to/auto_protected.h

# Run
./test_auto_protected
```

## Troubleshooting

### Compilation Errors

**Error: Type not supported**

```cpp
// ❌ Doesn't work
AutoProtected<std::string> value("hello");  // Not supported
AutoProtected<bool> flag(true);              // Not supported

// ✅ Works
AutoProtected<int32_t> value(100);           // Supported
AutoProtected<float> fvalue(1.5f);           // Supported
```

**Solution**: Only use supported types (int32_t, int64_t, uint32_t, uint64_t, float).

### Performance Issues

**Problem**: Frame rate drops after using `AutoProtected<T>`

**Solution**: Reduce the number of protected fields

```cpp
// Before: Too many fields
struct SlowState {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;
    AutoProtected<int32_t> temp1;  // Remove
    AutoProtected<int32_t> temp2;  // Remove
};

// After: Only critical fields
struct FastState {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;
    
    // Non-critical fields separate
    int32_t temp1;
    int32_t temp2;
};
```

### Memory Issues

**Problem**: High memory usage with many protected values

**Solution**: Use regular types for non-critical data

```cpp
struct GameState {
    // Protect only what cheaters would modify
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;
    
    // Regular types for everything else
    std::vector<int32_t> particles;     // Not protected
    int32_t animation_frame;            // Not protected
};
```

### Cheat Detection Not Triggered

**Problem**: Cheats not being detected

**Solutions**:

1. Verify you're using `.get()` and `.set()`:
   ```cpp
   // ✅ Correct
   int32_t value = player.health.get();
   player.health.set(100);
   
   // ❌ Incorrect - direct access bypasses protection
   // Don't do this!
   ```

2. Check that protected values are actually being used:
   ```cpp
   // Verify protection is active
   AutoProtected<int32_t> test(100);
   assert(test.get() == 100);  // Protection is working
   ```

## Limitations

1. **Type Restrictions**: Only works with int32_t, int64_t, uint32_t, uint64_t, float
2. **Performance Overhead**: ~78x slower than regular values
3. **Not a Complete Solution**: Should be combined with server-side validation for multiplayer
4. **No Boolean Support**: Use int32_t with 0/1 instead
5. **No f64 Support**: Use float instead of double

## Best Practices

### DO ✅

1. **Protect critical game state:**
   ```cpp
   struct Player {
       AutoProtected<int32_t> health;   // Cheatable
       AutoProtected<int32_t> ammo;     // Cheatable
       AutoProtected<int32_t> score;    // Cheatable
   };
   ```

2. **Separate protected and unprotected state:**
   ```cpp
   struct GameState {
       AutoProtected<int32_t> health;  // Protected
       int32_t animation_frame;         // Not protected
   };
   ```

3. **Batch updates:**
   ```cpp
   // Update once per frame
   player.health.set(new_health);
   player.ammo.set(new_ammo);
   ```

4. **Profile before adding protection:**
   ```cpp
   // Test performance impact
   auto start = std::chrono::high_resolution_clock::now();
   // ... operations with protected values ...
   auto end = std::chrono::high_resolution_clock::now();
   auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
   ```

### DON'T ❌

1. **Don't protect everything:**
   ```cpp
   // ❌ Too much overhead
   struct Everything {
       AutoProtected<int32_t> health;
       AutoProtected<int32_t> temp1;
       AutoProtected<int32_t> temp2;
       AutoProtected<int32_t> temp3;
   };
   ```

2. **Don't protect configuration data:**
   ```cpp
   // ❌ Use file protection instead
   struct Config {
       AutoProtected<int32_t> max_fps;        // Not cheatable
       AutoProtected<int32_t> screen_width;   // Not cheatable
   };
   ```

3. **Don't update in tight loops:**
   ```cpp
   // ❌ Very slow!
   for (int32_t i = 0; i < 1000; i++) {
       player.health.set(player.health.get() + 1);
   }
   ```

4. **Don't bypass protection:**
   ```cpp
   // ❌ Never do this!
   // Direct memory access bypasses trap detection
   ```

## Migration Guide

### From Unprotected to Protected

**Before (unprotected):**
```cpp
struct Player {
    int32_t health;
    int32_t ammo;
    int32_t score;
};

Player player{100, 30, 0};
player.health = 75;
```

**After (protected):**
```cpp
struct Player {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;
    AutoProtected<int32_t> score;

    Player(int32_t h, int32_t a, int32_t s)
        : health(h), ammo(a), score(s) {}
};

Player player(100, 30, 0);
player.health.set(75);
```

**Migration Steps:**
1. Add `#include "auto_protected.h"`
2. Change field types from `T` to `AutoProtected<T>`
3. Add constructor to initialize protected fields
4. Update all field access to use `.get()` and `.set()`
5. Profile performance to ensure acceptable overhead

### From Manual Protected to AutoProtected

**Before (manual Protected<T>):**
```cpp
struct Player {
    Protected<int32_t> health;
    Protected<int32_t> ammo;

    Player(int32_t h, int32_t a) : health(h), ammo(a) {}
};
```

**After (AutoProtected<T>):**
```cpp
struct Player {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;

    Player(int32_t h, int32_t a) : health(h), ammo(a) {}
};
```

**Note**: The API is identical! Just replace `Protected<T>` with `AutoProtected<T>`.

## Examples

### Example 1: Complete Game State

```cpp
#include "auto_protected.h"
#include <iostream>

using namespace Maxion;

struct Player {
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> max_health;
    AutoProtected<int32_t> ammo;
    AutoProtected<int32_t> max_ammo;
    AutoProtected<int32_t> score;
    AutoProtected<int32_t> currency;

    Player(int32_t h, int32_t mh, int32_t a, int32_t ma, int32_t s, int32_t c)
        : health(h), max_health(mh), ammo(a), max_ammo(ma), score(s), currency(c) {}

    void take_damage(int32_t damage) {
        int32_t current = health.get();
        int32_t new_health = (current - damage) > 0 ? current - damage : 0;
        health.set(new_health);
    }

    void fire_weapon() {
        if (ammo.get() > 0) {
            ammo.set(ammo.get() - 1);
        }
    }

    void add_score(int32_t points) {
        score.set(score.get() + points);
    }

    void add_currency(int32_t amount) {
        currency.set(currency.get() + amount);
    }

    void print_state() {
        std::cout << "Health: " << health.get() << "/" << max_health.get() << "\n";
        std::cout << "Ammo: " << ammo.get() << "/" << max_ammo.get() << "\n";
        std::cout << "Score: " << score.get() << "\n";
        std::cout << "Currency: " << currency.get() << "\n";
    }
};

int main() {
    Player player(100, 100, 30, 100, 0, 0);

    std::cout << "Initial state:\n";
    player.print_state();

    player.take_damage(25);
    player.fire_weapon();
    player.fire_weapon();
    player.add_score(100);
    player.add_currency(50);

    std::cout << "\nAfter gameplay:\n";
    player.print_state();

    return 0;
}
```

### Example 2: Multiplayer Server Validation

```cpp
#include "auto_protected.h"
#include <cassert>

using namespace Maxion;

struct ServerPlayer {
    AutoProtected<uint32_t> player_id;
    AutoProtected<int32_t> health;
    AutoProtected<int32_t> ammo;
    AutoProtected<int32_t> score;

    ServerPlayer(uint32_t id, int32_t h, int32_t a, int32_t s)
        : player_id(id), health(h), ammo(a), score(s) {}
};

bool validate_player(const ServerPlayer& player) {
    // Server-side validation
    int32_t health = player.health.get();
    int32_t score = player.score.get();

    // Check for impossible values
    if (health < 0 || health > 100) {
        return false;
    }

    if (score < 0 || score > 1'000'000) {
        return false;
    }

    return true;
}

int main() {
    ServerPlayer player1(12345, 100, 30, 0);
    ServerPlayer player2(67890, 50, 20, 500);

    assert(validate_player(player1) == true);
    assert(validate_player(player2) == true);

    // Simulate cheat
    player1.health.set(999);
    assert(validate_player(player1) == false);  // Invalid!

    std::cout << "✅ Server validation works!" << std::endl;
    return 0;
}
```

## Related Documentation

- **Protected<T> Implementation**: See `protected.h` - Underlying protection mechanism
- **Rust `#[auto_protected]`**: `docs/06_security/008_auto_protected.md` - Rust equivalent
- **Trap System**: `docs/06_security/006_trap.md` - How trap protection works
- **Performance Benchmarks**: `docs/05_benchmark/04_trap_vs_notrap.md` - Performance analysis

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-02-19 | Initial release of `AutoProtected<T>` |

## Building and Running

### Compilation

```bash
# Using g++
g++ -std=c++17 -o auto_protected_demo auto_protected_demo.cpp -I.

# Using clang++
clang++ -std=c++17 -o auto_protected_demo auto_protected_demo.cpp -I.

# Using MSVC (Visual Studio)
cl /std:c++17 /Fe:auto_protected_demo.exe auto_protected_demo.cpp
```

### Running the Demo

```bash
# Run the demo
./auto_protected_demo
```

### Integration into Your Project

1. Copy `auto_protected.h` to your project
2. Include it in your source files
3. Use `AutoProtected<T>` for critical values
4. Link with Maxion runtime library if using full Maxion protection

## Contributing

To contribute to this feature:
1. Review the implementation in `auto_protected.h`
2. Run the demo: `./auto_protected_demo`
3. Run unit tests
4. Submit PR with documentation updates

## License

MIT License OR Apache-2.0

---

**Version:** 1.0  
**Last Updated:** 2025-02-19  
**Status:** Production Ready ✅

For more information, see:
- Rust implementation: `docs/06_security/008_auto_protected.md`
- Protected<T> API: See `protected.h` header file
- Maxion project: https://github.com/maxion-game/maxion-protector