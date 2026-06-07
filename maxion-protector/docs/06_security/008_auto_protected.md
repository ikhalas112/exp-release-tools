# Auto-Protected Attribute

## Overview

The `#[auto_protected]` attribute macro provides automatic anti-cheat protection for all struct fields by wrapping them in `Protected<T>`. This eliminates the need to manually wrap each field, reducing boilerplate code and making it easier to protect game state from memory tampering.

**Key Features:**
- ✅ Automatic field wrapping in `Protected<T>`
- ✅ Generated constructor with all fields
- ✅ Type-safe protection with zero runtime overhead for the macro itself
- ✅ Works with all `Protected<T>` supported types (i32, i64, u32, u64, f32, tuples)
- ✅ Minimal code changes - just add `#[auto_protected]` to your struct

## Document Metadata

**Document ID**: 008_auto_protected  
**Status**: Production Ready ✅  
**Last Updated**: 2025-02-19  
**Version**: 1.0  
**Authors**: Maxion Team

## How It Works

### Macro Expansion

The `#[auto_protected]` attribute transforms a regular struct at compile time:

**Before:**
```rust
#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}
```

**After (generated code):**
```rust
pub struct Player {
    pub health: Protected<i32>,
    pub ammo: Protected<i32>,
    pub score: Protected<i32>,
}

impl Player {
    pub fn new(health: i32, ammo: i32, score: i32) -> Self {
        Self {
            health: Protected::new(health),
            ammo: Protected::new(ammo),
            score: Protected::new(score),
        }
    }
}
```

### Protection Mechanism

Each protected field uses the same `Protected<T>` implementation from the trap system:

1. **Storage**: Two values - trap (plain text) and real (XOR encrypted)
2. **Access**: Decrypts real value and compares with trap on read
3. **Update**: Rotates encryption key and updates both values on write
4. **Detection**: Detects mismatch = cheat detected!

## Usage

### Basic Usage

```rust
use maxion_core::{auto_protected, CheatAction, CheatDetector};

// Initialize cheat detector
CheatDetector::init(CheatAction::Log, 10);

#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}

// Create player with protected fields
let player = Player::new(100, 30, 0);

// Read values (automatically checks for tampering)
let current_health = player.health.get();
let current_ammo = player.ammo.get();

// Update values (rotates encryption key)
player.health.set(75);
player.ammo.set(25);
```

### Complex Structs

```rust
#[auto_protected]
struct GameState {
    player_health: i32,
    player_ammo: i32,
    player_score: i32,
    player_position: (f32, f32, f32),
    enemy_count: i32,
    currency: i32,
    level_complete: bool,
}

let mut game = GameState::new(
    100,               // health
    50,                // ammo
    0,                 // score
    (0.0, 0.0, 0.0),   // position
    10,                // enemies
    1000,              // currency
    false,             // complete
);

// Access and update fields
game.player_health.set(85);
game.player_position.set((10.5, 20.3, 5.7));
let pos = game.player_position.get();
```

### Multiple Instances

```rust
#[auto_protected]
struct Enemy {
    health: i32,
    damage: i32,
}

// Each instance is independently protected
let enemy1 = Enemy::new(100, 10);
let enemy2 = Enemy::new(150, 20);

// Modifications to one don't affect the other
enemy1.health.set(50);
assert_eq!(enemy1.health.get(), 50);
assert_eq!(enemy2.health.get(), 150);
```

## API Reference

### `#[auto_protected]` Attribute

Automatically wraps all struct fields in `Protected<T>` and generates a constructor.

**Syntax:**
```rust
#[auto_protected]
struct StructName {
    field1: Type1,
    field2: Type2,
    // ...
}
```

**Generated Methods:**

- `new(field1: Type1, field2: Type2, ...) -> Self` - Constructor that initializes all protected fields

**Field Access:**

All fields are public and use the `Protected<T>` API:
- `field.get() -> T` - Read the protected value (checks for tampering)
- `field.set(value: T)` - Write a new value (rotates encryption key)

**Supported Types:**

All types supported by `Protected<T>`:
- Integers: `i32`, `i64`, `u32`, `u64`
- Floats: `f32`
- Tuples: `(T1, T2, ...)`
- Nested tuples work for complex data structures

## Security Benefits

### What It Protects Against

✅ **Detected Attacks:**
- Memory scanning for values (Cheat Engine, ArtMoney)
- Value modification (changing health to 999)
- Value freezing (god mode, unlimited ammo)
- Basic pointer chasing

✅ **Prevention Mechanisms:**
- XOR encryption of all protected values
- Trap values that detect modifications
- Key rotation on every write
- Automatic cheat detection and reporting

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

Protected values have significant overhead compared to regular values:

```
Regular i32:      ~364 µs for 100,000 operations
Protected<i32>:   ~28.7 ms for 100,000 operations
Overhead:         ~78x slower (7,800%)
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
   ```rust
   // ✅ Good: Protect cheatable values
   #[auto_protected]
   struct CriticalState {
       health: i32,
       ammo: i32,
       score: i32,
       currency: i32,
   }

   // ❌ Bad: Protect everything
   #[auto_protected]
   struct Everything {
       // Too many fields = too much overhead
       health: i32,
       ammo: i32,
       score: i32,
       currency: i32,
       temp_counter: i32,
       flag1: bool,
       flag2: bool,
       // ... more fields
   }
   ```

2. **Batch updates when possible:**
   ```rust
   // ✅ Good: Update once per frame
   player.health.set(new_health);
   player.ammo.set(new_ammo);
   player.score.set(new_score);

   // ❌ Bad: Update in tight loops
   for _ in 0..1000 {
       player.health.set(player.health.get() + 1); // Very slow!
   }
   ```

3. **Separate protected and unprotected state:**
   ```rust
   // ✅ Good: Separate concerns
   #[auto_protected]
   struct ProtectedState {
       health: i32,
       ammo: i32,
   }

   struct NormalState {
       animation_frame: i32,
       temp_counter: i32,
   }
   ```

## Comparison with Manual Protection

### Manual Protection

```rust
// Without #[auto_protected] - verbose
struct Player {
    health: Protected<i32>,
    ammo: Protected<i32>,
    score: Protected<i32>,
}

impl Player {
    fn new(health: i32, ammo: i32, score: i32) -> Self {
        Self {
            health: Protected::new(health),
            ammo: Protected::new(ammo),
            score: Protected::new(score),
        }
    }
}
```

### Automatic Protection

```rust
// With #[auto_protected] - clean
#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}
// Constructor generated automatically!
```

### When to Use Each

| Scenario | Recommended |
|----------|-------------|
| Simple structs with few fields | `#[auto_protected]` |
| Complex structs with custom logic | Manual |
| Need custom constructors | Manual |
| Want to protect only some fields | Manual |
| Want zero boilerplate | `#[auto_protected]` |

**Manual protection example (partial field protection):**
```rust
struct MixedProtection {
    health: Protected<i32>,      // Protected manually
    ammo: Protected<i32>,        // Protected manually
    temp_counter: i32,           // Not protected
    animation_frame: i32,        // Not protected
}
```

## Supported Types

### Fully Supported

All types that work with `Protected<T>`:

```rust
#[auto_protected]
struct AllTypes {
    // Integers
    i32_field: i32,
    i64_field: i64,
    u32_field: u32,
    u64_field: u64,
    
    // Floats
    f32_field: f32,
    
    // Tuples
    position: (f32, f32, f32),
    velocity: (f32, f32),
    complex: ((i32, i32), (f32, f32)),
}
```

### Not Supported

Types that don't implement `Protected<T>`:

```rust
#[auto_protected]
struct NotSupported {
    // ❌ Will fail to compile
    string_field: String,           // Not supported
    vec_field: Vec<i32>,             // Not supported
    custom_struct: MyStruct,         // Not supported (unless it's a supported type)
    bool_field: bool,                // Not supported (use i32 with 0/1)
    f64_field: f64,                  // Not supported (use f32)
}
```

### Workarounds

**For boolean values:**
```rust
// Use i32 with 0/1 instead of bool
#[auto_protected]
struct GameFlags {
    is_complete: i32,      // 0 = false, 1 = true
    has_powerup: i32,     // 0 = false, 1 = true
}

// Helper methods
impl GameFlags {
    fn is_complete(&self) -> bool {
        self.is_complete.get() != 0
    }
    
    fn set_complete(&self, value: bool) {
        self.is_complete.set(if value { 1 } else { 0 });
    }
}
```

**For complex data structures:**
```rust
// Use tuple to store related values
#[auto_protected]
struct PlayerStats {
    // Instead of custom struct, use tuple
    health_and_mana: (i32, i32),  // (health, mana)
    position_and_rotation: ((f32, f32, f32), (f32, f32, f32)),
}
```

## Integration with Cheat Detection

### Setting Up Cheat Detection

```rust
use maxion_core::{auto_protected, CheatAction, CheatDetector};

// Initialize at game startup
fn init_game() {
    // Configure cheat detection action
    CheatDetector::init(CheatAction::Log, 10);
    
    // Other options:
    // CheatDetector::init(CheatAction::Panic, 5);     // Panic on detection
    // CheatDetector::init(CheatAction::RandomCrash, 10); // Random crashes
    // CheatDetector::init(CheatAction::FlagAccount, 3); // Flag for review
}

// Use protected structs
#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
}
```

### Cheat Detection Actions

| Action | Use Case | Behavior |
|--------|----------|----------|
| `Panic` | Development | Immediately panics on detection |
| `Log` | Production | Logs detection to console/file |
| `RandomCrash` | Production | Crashes randomly to confuse attacker |
| `FlagAccount` | Multiplayer | Flags account for server review |

### Monitoring Cheat Detection

```rust
use maxion_core::CheatDetector;

// Check detection count
let detections = CheatDetector::detection_count();
if detections > 0 {
    println!("⚠️  Cheat detected {} times!", detections);
    
    // Take action (kick player, report to server, etc.)
    handle_cheat_detection();
}

// Reset detection count (testing only)
// CheatDetector::reset();
```

## Testing

### Unit Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use maxion_core::{auto_protected, Protected};

    #[auto_protected]
    struct TestStruct {
        value: i32,
        float_val: f32,
    }

    #[test]
    fn test_auto_protected() {
        let test = TestStruct::new(100, 1.5);
        
        assert_eq!(test.value.get(), 100);
        assert_eq!(test.float_val.get(), 1.5);
        
        test.value.set(200);
        assert_eq!(test.value.get(), 200);
    }

    #[test]
    fn test_independent_instances() {
        let test1 = TestStruct::new(100, 1.0);
        let test2 = TestStruct::new(200, 2.0);
        
        test1.value.set(150);
        assert_eq!(test1.value.get(), 150);
        assert_eq!(test2.value.get(), 200);
    }
}
```

### Cheat Simulation Tests

```rust
#[test]
fn test_cheat_detection_simulation() {
    use maxion_core::{CheatAction, CheatDetector, Protected};
    
    CheatDetector::init(CheatAction::Panic, 5);
    
    // Normal operation
    let value = Protected::new(100);
    assert_eq!(value.get(), 100);
    
    // Simulate cheat (modify internal state directly)
    // Note: This is unsafe and only for testing!
    // In real usage, you can't directly access internal state
    
    // After modification, next get() should detect mismatch
    // and trigger cheat detection
}
```

## Troubleshooting

### Compilation Errors

**Error: `#[auto_protected]` only supports structs with named fields**

```rust
// ❌ Doesn't work
#[auto_protected]
struct Point(i32, i32, i32);

// ✅ Works
#[auto_protected]
struct Point {
    x: i32,
    y: i32,
    z: i32,
}
```

**Error: Type doesn't implement Protected<T>**

```rust
// ❌ Doesn't work
#[auto_protected]
struct BadStruct {
    value: String,  // Not supported
}

// ✅ Works
#[auto_protected]
struct GoodStruct {
    value: i32,  // Supported
}
```

### Performance Issues

**Problem**: Frame rate drops after using `#[auto_protected]`

**Solution**: Reduce the number of protected fields
```rust
// Before: Too many fields
#[auto_protected]
struct SlowState {
    health: i32,
    ammo: i32,
    score: i32,
    currency: i32,
    temp1: i32,      // Remove these
    temp2: i32,      // Remove these
    temp3: i32,      // Remove these
}

// After: Only critical fields
#[auto_protected]
struct FastState {
    health: i32,
    ammo: i32,
    score: i32,
    currency: i32,
}

// Non-critical fields in separate struct
struct TempState {
    temp1: i32,
    temp2: i32,
    temp3: i32,
}
```

### Cheat Detection Not Triggered

**Problem**: Cheats not being detected

**Solutions**:
1. Verify cheat detector is initialized:
   ```rust
   CheatDetector::init(CheatAction::Log, 10);
   ```

2. Check that protected values are being used:
   ```rust
   // Correct: Use .get() and .set()
   let value = player.health.get();
   player.health.set(100);
   
   // Incorrect: Direct memory access
   // Don't do this!
   ```

3. Verify detection count:
   ```rust
   let detections = CheatDetector::detection_count();
   println!("Detections: {}", detections);
   ```

## Limitations

1. **Type Restrictions**: Only works with `Protected<T>` supported types
2. **Named Fields Only**: Doesn't work with tuple structs
3. **All or Nothing**: Protects all fields in the struct
4. **Performance Overhead**: ~78x slower than regular values
5. **Manual Required for Mixed Protection**: Can't protect only some fields

## Future Enhancements

### Planned Features

- **Selective Protection**: Allow protecting only specific fields
- **Custom Constructors**: Support user-defined constructors
- **More Type Support**: Add support for `bool`, `f64`, custom structs
- **Performance Optimizations**: Reduce overhead with SIMD
- **Tuple Struct Support**: Enable protection for tuple structs

### Roadmap

**Phase 1 (Current)**
- ✅ Basic `#[auto_protected]` attribute
- ✅ All `Protected<T>` types supported
- ✅ Automatic constructor generation

**Phase 2 (Planned)**
- [ ] Selective field protection with attributes
- [ ] Custom constructor support
- [ ] Boolean type support

**Phase 3 (Future)**
- [ ] Custom struct support (with derive macros)
- [ ] Performance optimizations
- [ ] Tuple struct support

## Migration Guide

### From Manual Protection

**Before (manual):**
```rust
struct Player {
    health: Protected<i32>,
    ammo: Protected<i32>,
    score: Protected<i32>,
}

impl Player {
    fn new(health: i32, ammo: i32, score: i32) -> Self {
        Self {
            health: Protected::new(health),
            ammo: Protected::new(ammo),
            score: Protected::new(score),
        }
    }
}
```

**After (automatic):**
```rust
#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}
```

**Migration Steps:**
1. Add `use maxion_core::auto_protected;`
2. Add `#[auto_protected]` attribute to struct
3. Remove `Protected<>` wrappers from field types
4. Remove manual constructor (will be generated)
5. Update usage (same API: `.get()` and `.set()`)

### From No Protection

**Before (unprotected):**
```rust
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}

let player = Player { health: 100, ammo: 30, score: 0 };
player.health = 75;
```

**After (protected):**
```rust
#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}

let player = Player::new(100, 30, 0);
player.health.set(75);
```

**Migration Steps:**
1. Add `use maxion_core::auto_protected;`
2. Add `#[auto_protected]` attribute to struct
3. Update initialization to use `new()` constructor
4. Update all field access to use `.get()` and `.set()`
5. Initialize `CheatDetector` at startup
6. Test thoroughly to ensure no behavioral changes

## Examples

### Game State Example

```rust
use maxion_core::{auto_protected, CheatAction, CheatDetector};

fn main() {
    CheatDetector::init(CheatAction::Log, 10);

    #[auto_protected]
    struct Player {
        health: i32,
        max_health: i32,
        ammo: i32,
        max_ammo: i32,
        score: i32,
        position: (f32, f32, f32),
        currency: i32,
    }

    let mut player = Player::new(
        100,              // health
        100,              // max_health
        30,               // ammo
        100,              // max_ammo
        0,                // score
        (0.0, 0.0, 0.0),  // position
        0,                // currency
    );

    // Game loop
    loop {
        // Take damage
        let damage = 10;
        let new_health = (player.health.get() - damage).max(0);
        player.health.set(new_health);

        // Fire weapon
        if player.ammo.get() > 0 {
            player.ammo.set(player.ammo.get() - 1);
        }

        // Update position
        player.position.set((5.0, 10.0, 0.0));

        // Add score
        player.score.set(player.score.get() + 100);

        // Collect currency
        player.currency.set(player.currency.get() + 50);

        // Break after demo
        if player.health.get() == 0 {
            break;
        }
    }
}
```

### Multiplayer State Example

```rust
use maxion_core::{auto_protected, CheatAction, CheatDetector};

fn init_server() {
    // Stricter detection for multiplayer
    CheatDetector::init(CheatAction::FlagAccount, 3);
}

#[auto_protected]
struct ServerPlayer {
    player_id: u32,
    health: i32,
    ammo: i32,
    score: i32,
    position: (f32, f32, f32),
    ping: i32,
}

fn validate_player(player: &ServerPlayer) -> bool {
    // Server-side validation
    let health = player.health.get();
    let score = player.score.get();
    
    // Check for impossible values
    if health < 0 || health > 100 {
        return false;
    }
    
    if score < 0 || score > 1_000_000 {
        return false;
    }
    
    // Check cheat detection count
    if CheatDetector::detection_count() > 0 {
        return false;
    }
    
    true
}
```

## Related Documentation

- **Trap System**: `docs/06_security/006_trap.md` - Underlying `Protected<T>` implementation
- **Protected Prefix**: `docs/06_security/007_sec_prefix.md` - File-level protection
- **Benchmarks**: `docs/05_benchmark/04_trap_vs_notrap.md` - Performance analysis

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-02-19 | Initial release of `#[auto_protected]` attribute |

## Contributing

To contribute to this feature:
1. Review the implementation in `crates/maxion-macros/src/lib.rs`
2. Run tests: `cargo test --package maxion-macros`
3. Run example: `cargo run --example auto_protected_demo --package maxion-core`
4. Submit PR with documentation updates

---

**Version:** 1.0  
**Last Updated:** 2025-02-19  
**Status:** Production Ready ✅