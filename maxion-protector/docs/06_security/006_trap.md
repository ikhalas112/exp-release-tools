# Honeypot Anti-Cheat System

## Overview

The Maxion honeypot anti-cheat system provides protection against memory tampering by cheat engines (Cheat Engine, ArtMoney, etc.). It uses a combination of encrypted values and decoy "trap" values to detect when memory has been modified.

**Key Features:**
- ✅ Detects memory scanning and value modification
- ✅ Prevents value freezing attacks
- ✅ Automatic cheat detection and reporting
- ✅ Thread-safe implementation available
- ✅ Support for common game data types (i32, f32, tuples, etc.)

## How It Works

### Core Concept

Each `Protected<T>` value stores data in two forms:

1. **Trap Value**: Plain text, easily searchable by Cheat Engine
2. **Real Value**: Encrypted with XOR, hard to find and modify

When the protected value is read:
1. Decrypt the real value using the current encryption key
2. Read the trap value (volatile read to prevent optimization)
3. Compare them - if mismatch → CHEAT DETECTED!

When the protected value is written:
1. Generate a new random encryption key (prevents freezing)
2. Encrypt the new value with the new key
3. Update both trap and real values

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

## Usage Example

### Basic Usage

```rust
use maxion_core::Protected;

// Create protected game state
let health = Protected::new(100i32);
let ammo = Protected::new(30i32);
let score = Protected::new(0i32);

// Read values (automatically checks for tampering)
let current_health = health.get();
assert_eq!(current_health, 100);

// Update values (rotates encryption key)
health.set(75);
ammo.set(29);
score.set(100);

// Values are protected from memory tampering
// Cheat Engine modifications will be detected
```

### Thread-Safe Usage

```rust
use maxion_core::ProtectedSync;
use std::sync::Arc;

// Thread-safe protected value
let health = Arc::new(ProtectedSync::new(100i32));

// Share across threads
let health_clone = Arc::clone(&health);
std::thread::spawn(move || {
    health_clone.set(75);
}).join().unwrap();

assert_eq!(health.get(), 75);
```

### Game State Integration

```rust
struct Player {
    health: Protected<i32>,
    ammo: Protected<i32>,
    position: Protected<(f32, f32, f32)>,
}

impl Player {
    fn take_damage(&self, damage: i32) {
        let current = self.health.get();
        let new_health = (current - damage).max(0);
        self.health.set(new_health);
    }

    fn fire_weapon(&self) {
        let current = self.ammo.get();
        if current > 0 {
            self.ammo.set(current - 1);
        }
    }

    fn move_to(&self, x: f32, y: f32, z: f32) {
        self.position.set((x, y, z));
    }
}
```

## API Reference

### `Protected<T>`

Main protected value type. Not thread-safe by default.

**Methods:**

- `new(val: T) -> Self` - Create new protected value
- `get(&self) -> T` - Get value (checks for tampering)
- `set(&self, val: T)` - Set value (rotates key)
- `unsafe get_unchecked(&self) -> T` - Get without checking (testing only)
- `unsafe set_real_only(&self, val: T)` - Set only real value (testing only)

**Supported Types:**
- `i32`, `i64`, `u32`, `u64`
- `f32`
- `(f32, f32, f32)` - For position coordinates

### `ProtectedSync<T>`

Thread-safe wrapper using `Mutex`.

**Methods:**

- `new(val: T) -> Self` - Create new thread-safe protected value
- `get(&self) -> T` - Get value (thread-safe)
- `set(&self, val: T)` - Set value (thread-safe)

**Traits:**
- `Clone` - Creates a new instance with the same value

### `CheatDetector`

Handles cheat detection actions.

**Methods:**

- `new() -> Self` - Create new detector
- `init(action: CheatAction, max_detections: u32)` - Configure detection
- `report_cheat(&self)` - Report detection
- `detection_count(&self) -> u32` - Get count
- `reset(&self)` - Reset count (testing only)

### `CheatAction`

Actions to take when cheat is detected.

**Variants:**

- `Panic` - Panic immediately (development/testing)
- `Log` - Log the detection (production, default)
- `RandomCrash` - Crash randomly to confuse cheater
- `FlagAccount` - Flag account for review (multiplayer)

## Performance Considerations

### Performance Overhead

Protected values have higher overhead than regular values:

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

4. **Choose appropriate detection action:**
   ```rust
   // Development: Catch bugs early
   CheatDetector::init(CheatAction::Panic, 5);
   
   // Production: Log and monitor
   CheatDetector::init(CheatAction::Log, 10);
   
   // Multiplayer: Flag accounts
   CheatDetector::init(CheatAction::FlagAccount, 3);
   ```

## Security Considerations

### What It Protects Against

✅ **Detected:**
- Memory scanning for values
- Value modification (changing health to 999)
- Value freezing (god mode, unlimited ammo)
- Basic pointer chasing

⚠️ **Partially Protected:**
- Advanced pointer chain attacks
- Code injection attacks
- Memory patching attacks

❌ **Not Protected Against:**
- Network manipulation (requires server-side validation)
- Graphics hacks (wallhacks, aimbots)
- Input manipulation (macros, scripting)

### Limitations

1. **Requires knowledge of Protected<T> API:**
   - Advanced cheaters who understand the system can modify all three fields
   - But this requires reverse engineering and is much harder

2. **Not a complete solution:**
   - Should be combined with server-side validation for multiplayer
   - Use with other anti-cheat measures for best results

3. **Performance impact:**
   - Not suitable for all game state
   - Profile before using extensively

## Unity Integration

### Overview

The Maxion anti-cheat system provides FFI callbacks to notify Unity when cheats are detected. This follows the "Unity is VIEW-ONLY" principle - Rust handles detection and HWID generation, Unity decides what to display or do.

### Setup

#### 1. Register Callback in Unity

```csharp
using System;
using System.Runtime.InteropServices;

public class AntiCheat : MonoBehaviour
{
    // Callback delegate signature
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    public delegate void CheatCallbackDelegate(
        int cheatType,      // CheatType enum value
        IntPtr hwidPtr,     // Pointer to HWID string (UTF-8)
        int hwidLen,        // Length of HWID string
        long timestamp,     // Unix timestamp in milliseconds
        uint detectionCount // Number of detections
    );

    // Import FFI functions
    [DllImport("maxion", CallingConvention = CallingConvention.Cdecl)]
    private static extern void MaxionRegisterCheatCallback(CheatCallbackDelegate callback);

    [DllImport("maxion", CallingConvention = CallingConvention.Cdecl)]
    private static extern void MaxionGetHardwareId(out IntPtr ptr, out int len);

    [DllImport("maxion", CallingConvention = CallingConvention.Cdecl)]
    private static extern bool MaxionHasCheatCallback();

    // Cheat types
    public enum CheatType
    {
        MemoryTampering = 0,
        ValueFreeze = 1,
        IntegrityViolation = 2,
        Unknown = 99
    }

    // Static callback reference (prevents GC)
    private static CheatCallbackDelegate _callbackDelegate;

    void Start()
    {
        // Register callback
        _callbackDelegate = new CheatCallbackDelegate(OnCheatDetected);
        MaxionRegisterCheatCallback(_callbackDelegate);

        // Get hardware ID
        IntPtr hwidPtr;
        int hwidLen;
        MaxionGetHardwareId(out hwidPtr, out hwidLen);
        string hwid = Marshal.PtrToStringUTF8(hwidPtr, hwidLen);
        
        Debug.Log($"Anti-cheat initialized. HWID: {hwid}");
    }

    void OnDestroy()
    {
        // Unregister callback
        MaxionRegisterCheatCallback(null);
        _callbackDelegate = null;
    }

    // Callback invoked by Rust when cheat is detected
    private void OnCheatDetected(int cheatType, IntPtr hwidPtr, int hwidLen, long timestamp, uint detectionCount)
    {
        // Convert HWID to string
        string hwid = Marshal.PtrToStringUTF8(hwidPtr, hwidLen);
        
        // Convert timestamp to DateTime
        DateTime eventTime = DateTimeOffset.FromUnixTimeMilliseconds(timestamp).DateTime;

        // Handle different cheat types
        switch ((CheatType)cheatType)
        {
            case CheatType.MemoryTampering:
                Debug.LogError($"[Anti-Cheat] Memory tampering detected! HWID: {hwid}, Count: {detectionCount}");
                ShowWarningToPlayer("Unfair play detected. Continued violations will result in a ban.");
                SendToServer(hwid, "memory_tampering", detectionCount);
                break;

            case CheatType.ValueFreeze:
                Debug.LogWarning($"[Anti-Cheat] Value freeze detected! HWID: {hwid}");
                // Maybe show UI warning but don't ban immediately
                break;

            case CheatType.IntegrityViolation:
                Debug.LogError($"[Anti-Cheat] Integrity violation! HWID: {hwid}");
                KickPlayer();
                break;

            default:
                Debug.LogWarning($"[Anti-Cheat] Unknown cheat type: {cheatType}");
                break;
        }

        // Log to analytics
        Analytics.CheatDetected((CheatType)cheatType, hwid, eventTime, detectionCount);
    }

    private void SendToServer(string hwid, string cheatType, uint count)
    {
        // Send to your game server for validation
        StartCoroutine(PostCheatReport(hwid, cheatType, count));
    }

    private IEnumerator PostCheatReport(string hwid, string cheatType, uint count)
    {
        // Example: Send to server
        var payload = new
        {
            hwid = hwid,
            cheatType = cheatType,
            detectionCount = count,
            timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()
        };

        string json = JsonUtility.ToJson(payload);
        byte[] postData = System.Text.Encoding.UTF8.GetBytes(json);

        using (UnityWebRequest request = new UnityWebRequest("https://your-server.com/api/anti-cheat", "POST"))
        {
            request.uploadHandler = new UploadHandlerRaw(postData);
            request.SetRequestHeader("Content-Type", "application/json");

            yield return request.SendWebRequest();

            if (request.result == UnityWebRequest.Result.Success)
            {
                Debug.Log($"Cheat report sent successfully: {request.downloadHandler.text}");
            }
            else
            {
                Debug.LogError($"Failed to send cheat report: {request.error}");
            }
        }
    }

    private void ShowWarningToPlayer(string message)
    {
        // Show UI warning to player
        // This is VIEW-ONLY - no business logic
        UIManager.Instance.ShowWarning(message, 5f);
    }

    private void KickPlayer()
    {
        // Disconnect player from game
        NetworkManager.Instance.Disconnect();
    }
}
```

### Usage Example

#### 1. Configure Cheat Detection

```csharp
public class GameManager : MonoBehaviour
{
    void Start()
    {
        // Initialize anti-cheat with callback
        AntiCheat.Instance.Initialize();

        // Set up protected values
        playerHealth = new ProtectedInt32(100);
        playerAmmo = new ProtectedInt32(30);
    }
}
```

#### 2. Use Protected Values

```csharp
// Reading protected value (automatically checks for tampering)
int currentHealth = playerHealth.Get();

// Updating protected value (rotates encryption key)
playerHealth.Set(75);
```

### Best Practices

1. **Register Callback Early**: Register the callback in `Awake()` or `Start()` before any protected values are used

2. **Keep Callback Reference**: Store the delegate in a static field to prevent garbage collection

3. **Unregister on Destroy**: Clean up the callback when the object is destroyed

4. **Handle Errors Gracefully**: Wrap callback logic in try-catch to prevent anti-cheat crashes

5. **Send to Server**: Always send cheat reports to your server for validation and banning

6. **Show UI Feedback**: Inform players when suspicious activity is detected

### Testing in Unity

```csharp
public class AntiCheatTest : MonoBehaviour
{
    public void TestCallback()
    {
        // Simulate cheat detection (for testing only)
        // In production, this is triggered automatically by Rust
        
        // Get HWID
        IntPtr hwidPtr;
        int hwidLen;
        MaxionGetHardwareId(out hwidPtr, out hwidLen);
        
        // Test with MemoryTampering
        long timestamp = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        string hwid = Marshal.PtrToStringUTF8(hwidPtr, hwidLen);
        
        Debug.Log($"Testing callback. HWID: {hwid}");
    }
}
```

## Testing

### Unit Tests

```bash
# Run protected module tests
cargo test --package maxion-core --lib protected

# Run cheat callback tests
cargo test --package maxion-core --lib cheat_callback

# Run integration tests
cargo test --test phase6_integration_test
```

### Unity Integration Tests

1. **Test Callback Registration**:
   - Start Unity game
   - Verify callback is registered (`MaxionHasCheatCallback()` returns `true`)
   - Check console for initialization log

2. **Test HWID Generation**:
   - Call `MaxionGetHardwareId()`
   - Verify HWID is 32 characters (MD5 hash)
   - Verify HWID is consistent across multiple calls (cached)

3. **Test Cheat Detection**:
   - Use Cheat Engine to modify protected value
   - Verify callback is invoked
   - Verify correct cheat type, HWID, and timestamp

4. **Test Error Handling**:
   - Test with no callback registered (should not crash)
   - Test with null HWID pointer (should not crash)
   - Test callback exceptions (should not crash Rust)

## Build Instructions
### Cheat Simulation Tests

The integration tests simulate various cheat engine attacks:

- `test_simulate_memory_scanner` - Tests detection of value modification
- `test_simulate_value_freeze` - Tests detection of freeze attacks
- `test_simulate_pointer_chain_attack` - Tests partial protection
- `test_simulate_code_injection` - Tests advanced attack scenarios

## Build Instructions

### Standard Build

```bash
cargo build --release
```

### Running Tests

```bash
# All tests
cargo test --test phase6_integration_test

# Specific test
cargo test test_simulate_memory_scanner --test phase6_integration_test
```

## Troubleshooting

### Performance Too Slow

**Problem:** Protected values are causing frame rate drops

**Solution:**
1. Profile to identify bottlenecks
2. Reduce number of protected values
3. Batch updates instead of frequent individual updates
4. Consider using regular values for non-critical data

### False Positives

**Problem:** Legitimate value changes trigger detection

**Solution:**
1. Ensure all modifications go through `set()` method
2. Don't directly modify memory (unsafe)
3. Check for race conditions in multi-threaded code

### Detection Not Triggered

**Problem:** Cheats not being detected

**Solution:**
1. Verify detection action is configured correctly
2. Check logs for detection messages
3. Ensure protected values are being used correctly
4. Test with known cheat engine to verify detection

## Future Enhancements

### Phase 6+ (Planned)

1. **Polymorphic Code:**
   - Randomize code structure at runtime
   - Make reverse engineering harder

2. **Anti-Debugging:**
   - Detect debugger presence
   - Obfuscate control flow

3. **Integrity Hashing:**
   - Periodic integrity checks
   - Detect code patching

4. **GPU Acceleration:**
   - Use GPU for encryption/decryption
   - Reduce CPU overhead

## References

- Phase 6 Plan: `plans/006_honeypot.md`
- Implementation: `crates/maxion-core/src/protected.rs`
- Callback System: `crates/maxion-core/src/cheat_callback.rs`
- Integration Tests: `tests/phase6_integration_test.rs`
- Issue Tracker: `.issues/001_cheat_callback_with_hwid.md`

## API Reference - New Functions

### `maxion_register_cheat_callback(callback)`

Register a callback function to be invoked when cheats are detected.

**Arguments:**
- `callback` - Function pointer to callback, or `null` to unregister

**Callback Signature:**
```rust
extern "C" fn callback(
    cheat_type: i32,      // CheatType enum value
    hwid_ptr: *const u8,  // Pointer to HWID string (UTF-8)
    hwid_len: usize,      // Length of HWID string
    timestamp: u64,        // Unix timestamp in milliseconds
    detection_count: u32   // Number of detections for this cheat type
)
```

### `maxion_get_hardware_id(ptr, len)`

Get the cached hardware ID as UTF-8 bytes.

**Arguments:**
- `ptr` - Output pointer for HWID data (pass `null` to skip)
- `len` - Output pointer for HWID length (pass `null` to skip)

**Returns:**
- Fills `ptr` and `len` with HWID data
- HWID is a 32-character MD5 hash
- Pointer is valid for program lifetime (do not free)

### `maxion_has_cheat_callback()`

Check if a callback is currently registered.

**Returns:**
- `true` if callback is registered, `false` otherwise

### `CheatType` Enum

Types of cheats that can be detected:

- `MemoryTampering` (0) - Memory scanning and value modification
- `ValueFreeze` (1) - Value freezing (god mode, unlimited ammo)
- `IntegrityViolation` (2) - Code or memory integrity violation
- `Unknown` (99) - Unknown cheat type

---

**Version:** 1.0  
**Last Updated:** 2025-01-25  
**Phase:** 6 - Security Enhancements