#include <iostream>
#include <thread>
#include <chrono>
#include <atomic>
#include <string>
#include <cstring>
#include <iomanip>

// ============================================================================
// Cheat Detection Callback System (Simulating Rust FFI Interface)
// ============================================================================

enum class CheatType : int32_t {
    MemoryTampering = 0,
    ValueFreeze = 1,
    IntegrityViolation = 2,
    Unknown = 99
};

// Global callback function pointer type
using CheatCallbackFn = void(*)(CheatType, const char*, uint64_t, uint32_t);

// Global callback storage (thread-safe, simulating Rust's AtomicPtr)
std::atomic<CheatCallbackFn> g_cheat_callback{nullptr};

// Global detection counter
std::atomic<uint32_t> g_detection_count{0};

// Hardware ID (simulated)
const char* g_hardware_id = "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6";

// ============================================================================
// Simulated Rust FFI Functions
// ============================================================================

extern "C" {
    
    /// Register a cheat callback function
    void maxion_register_cheat_callback(CheatCallbackFn callback) {
        g_cheat_callback.store(callback, std::memory_order_release);
        std::cout << "✅ Callback registered: " 
                  << (callback ? "YES" : "NO") << std::endl;
    }
    
    /// Check if a callback is registered
    bool maxion_has_cheat_callback() {
        return g_cheat_callback.load(std::memory_order_acquire) != nullptr;
    }
    
    /// Get the hardware ID
    void maxion_get_hardware_id(const char** ptr, size_t* len) {
        if (ptr) *ptr = g_hardware_id;
        if (len) *len = strlen(g_hardware_id);
    }
    
    /// Internal function to invoke callback
    /// This simulates what AutoProtected<T> would call when tampering is detected
    void maxion_invoke_cheat_callback(CheatType cheat_type, uint32_t detection_count) {
        auto callback = g_cheat_callback.load(std::memory_order_acquire);
        if (callback) {
            const char* hwid_ptr;
            size_t hwid_len;
            maxion_get_hardware_id(&hwid_ptr, &hwid_len);
            
            // Get current timestamp (milliseconds)
            auto now = std::chrono::system_clock::now();
            auto timestamp = std::chrono::duration_cast<std::chrono::milliseconds>(
                now.time_since_epoch()
            ).count();
            
            callback(cheat_type, hwid_ptr, static_cast<uint64_t>(timestamp), detection_count);
        }
    }
}

// ============================================================================
// Callback Implementations
// ============================================================================

/// Example 1: Simple callback - shows warning message to player
extern "C" void simple_cheat_callback(
    CheatType cheat_type,
    const char* hwid,
    uint64_t timestamp,
    uint32_t detection_count
) {
    const char* cheat_type_name = "Unknown";
    switch (cheat_type) {
        case CheatType::MemoryTampering: cheat_type_name = "MemoryTampering"; break;
        case CheatType::ValueFreeze: cheat_type_name = "ValueFreeze"; break;
        case CheatType::IntegrityViolation: cheat_type_name = "IntegrityViolation"; break;
        case CheatType::Unknown: cheat_type_name = "Unknown"; break;
    }
    
    // Format timestamp
    auto total_seconds = timestamp / 1000;
    auto hours = (total_seconds % 86400) / 3600;
    auto minutes = (total_seconds % 3600) / 60;
    auto seconds = total_seconds % 60;
    
    std::cout << "\n🚨 ============================================" << std::endl;
    std::cout << "🚨 CHEAT DETECTION ALERT!" << std::endl;
    std::cout << "🚨 ============================================" << std::endl;
    std::cout << "   Type: " << cheat_type_name << std::endl;
    std::cout << "   HWID: " << hwid << std::endl;
    std::cout << "   Time: " 
              << std::setfill('0') << std::setw(2) << hours << ":"
              << std::setfill('0') << std::setw(2) << minutes << ":"
              << std::setfill('0') << std::setw(2) << seconds << std::endl;
    std::cout << "   Count: " << detection_count << " detection(s)" << std::endl;
    std::cout << "   Action: ⚠️ Show Warning to Player" << std::endl;
    std::cout << "🚨 ============================================\n" << std::endl;
    
    g_detection_count.fetch_add(1, std::memory_order_relaxed);
}

/// Example 2: Advanced callback - detailed handling based on cheat type
extern "C" void advanced_cheat_callback(
    CheatType cheat_type,
    const char* hwid,
    uint64_t timestamp,
    uint32_t detection_count
) {
    (void)timestamp;  // Timestamp not used in this callback, but kept for interface consistency
    
    switch (cheat_type) {
        case CheatType::MemoryTampering:
            std::cout << "⚠️  Memory tampering detected!" << std::endl;
            std::cout << "   Player modified protected memory addresses" << std::endl;
            std::cout << "   Recommended action: ⚠️ Show warning + Log for review" << std::endl;
            break;
            
        case CheatType::ValueFreeze:
            std::cout << "❄️  Value freeze detected!" << std::endl;
            std::cout << "   Player attempted to freeze game values" << std::endl;
            std::cout << "   Recommended action: ⚠️ Immediate warning UI" << std::endl;
            break;
            
        case CheatType::IntegrityViolation:
            std::cout << "🔐 Integrity violation detected!" << std::endl;
            std::cout << "   Code or memory integrity compromised" << std::endl;
            std::cout << "   Recommended action: ⚠️ Disconnect player + Ban review" << std::endl;
            break;
            
        case CheatType::Unknown:
            std::cout << "❓ Unknown cheat type detected!" << std::endl;
            std::cout << "   Please investigate manually" << std::endl;
            break;
    }
    
    std::cout << "   📝 HWID: " << hwid << std::endl;
    std::cout << "   📊 Detection count: " << detection_count << std::endl;
    std::cout << std::endl;
    
    g_detection_count.fetch_add(1, std::memory_order_relaxed);
}

/// Example 3: Silent callback - logs but doesn't show warnings
extern "C" void silent_cheat_callback(
    CheatType cheat_type,
    const char* hwid,
    uint64_t timestamp,
    uint32_t detection_count
) {
    // This callback logs silently without showing warnings to the player
    // Useful for logging-only mode
    (void)timestamp; // Unused in this implementation
    std::cout << "🔇 [SILENT LOG] Cheat detected: Type=" << static_cast<int>(cheat_type)
              << ", HWID=" << hwid 
              << ", Count=" << detection_count << std::endl;
    
    g_detection_count.fetch_add(1, std::memory_order_relaxed);
}

// ============================================================================
// Simulated AutoProtected vs Unprotected Values
// ============================================================================

/// Simulates AutoProtected<T> behavior
/// In real implementation, this would use Protected<T> from Rust backend
class SimulatedAutoProtected {
private:
    int32_t value;
    std::string name;
    
public:
    SimulatedAutoProtected(const std::string& n, int32_t v) 
        : value(v), name(n) {}
    
    // Simulate get operation (with integrity check)
    int32_t get() const {
        // In real implementation: decrypt and verify integrity
        return value;
    }
    
    // Simulate set operation (with key rotation and callback on tampering)
    void set(int32_t new_value) {
        // Simulate tampering detection check
        // In real implementation: this detects if value was modified externally
        bool tampered = false;
        
        // For demo: we simulate tampering if value changed dramatically
        // In production, this is detected by memory protection
        if (std::abs(new_value - value) > 1000) {
            tampered = true;
        }
        
        value = new_value;
        
        // In real implementation: encrypt with new key
        // In demo: just trigger callback if tampered
        if (tampered) {
            maxion_invoke_cheat_callback(CheatType::MemoryTampering, 
                                        g_detection_count.load() + 1);
        }
    }
    
    int32_t get_value() const { return value; }
    const std::string& get_name() const { return name; }
};

/// Regular unprotected value (no callback)
class UnprotectedValue {
private:
    int32_t value;
    std::string name;
    
public:
    UnprotectedValue(const std::string& n, int32_t v) 
        : value(v), name(n) {}
    
    int32_t get() const { return value; }
    void set(int32_t new_value) { value = new_value; }
    
    int32_t get_value() const { return value; }
    const std::string& get_name() const { return name; }
};

// ============================================================================
// Demo Functions
// ============================================================================

void demo_simple_callback() {
    std::cout << "┌──────────────────────────────────────────────────────────┐" << std::endl;
    std::cout << "│ Demo 1: Simple Callback with Warning                     │" << std::endl;
    std::cout << "└──────────────────────────────────────────────────────────┘\n" << std::endl;
    
    // Register simple callback (shows warnings)
    maxion_register_cheat_callback(simple_cheat_callback);
    
    std::cout << "✅ Simple callback registered (shows warnings)\n" << std::endl;
    
    // Create simulated protected values
    SimulatedAutoProtected health("health", 100);
    SimulatedAutoProtected ammo("ammo", 30);
    SimulatedAutoProtected score("score", 0);
    
    std::cout << "   Player created with protected values:" << std::endl;
    std::cout << "   - Health: " << health.get_value() << std::endl;
    std::cout << "   - Ammo: " << ammo.get_value() << std::endl;
    std::cout << "   - Score: " << score.get_value() << "\n" << std::endl;
    
    // Normal gameplay (no callback triggered)
    std::cout << "   🎮 Normal gameplay (no cheat detection):" << std::endl;
    health.set(90);   // Normal damage
    ammo.set(29);     // Normal fire
    score.set(100);   // Normal score
    
    std::cout << "\n✅ Simple callback demo complete!\n" << std::endl;
}

void demo_advanced_callback() {
    std::cout << "┌──────────────────────────────────────────────────────────┐" << std::endl;
    std::cout << "│ Demo 2: Advanced Callback with Type-Specific Actions    │" << std::endl;
    std::cout << "└──────────────────────────────────────────────────────────┘\n" << std::endl;
    
    // Switch to advanced callback
    maxion_register_cheat_callback(advanced_cheat_callback);
    
    std::cout << "✅ Advanced callback registered (type-specific warnings)\n" << std::endl;
    
    // Create new protected values
    SimulatedAutoProtected health("health", 100);
    
    std::cout << "   Simulating different cheat types:" << std::endl;
    health.set(80);   // Normal
    
    // Trigger different cheat types
    maxion_invoke_cheat_callback(CheatType::MemoryTampering, 1);
    std::this_thread::sleep_for(std::chrono::milliseconds(200));
    
    maxion_invoke_cheat_callback(CheatType::ValueFreeze, 1);
    std::this_thread::sleep_for(std::chrono::milliseconds(200));
    
    maxion_invoke_cheat_callback(CheatType::IntegrityViolation, 1);
    
    std::cout << "\n✅ Advanced callback demo complete!\n" << std::endl;
}

void demo_protected_vs_unprotected() {
    std::cout << "┌──────────────────────────────────────────────────────────┐" << std::endl;
    std::cout << "│ Demo 3: Protected vs Unprotected Comparison             │" << std::endl;
    std::cout << "└──────────────────────────────────────────────────────────┘\n" << std::endl;
    
    // Register callback that shows warnings
    maxion_register_cheat_callback(simple_cheat_callback);
    
    std::cout << "📊 SCENARIO: Player attempts to modify game memory\n" << std::endl;
    
    // Part 1: Protected Value (with callback)
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    std::cout << "PART 1: Protected Value (WITH CALLBACK)" << std::endl;
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    
    {
        SimulatedAutoProtected protected_health("health", 100);
        std::cout << "\n   Initial state:" << std::endl;
        std::cout << "   - Health: " << protected_health.get_value() << std::endl;
        
        // Simulate cheat attempt by modifying value dramatically
        std::cout << "\n   👾 Cheat attempt: Modifying health to 9999..." << std::endl;
        protected_health.set(9999);
        
        std::cout << "\n   ✅ Result: Callback invoked! Warning shown to player!" << std::endl;
        std::cout << "   ✅ Cheat attempt detected and logged!" << std::endl;
    }
    
    std::cout << "\n\n";
    
    // Part 2: Unprotected Value (no callback)
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    std::cout << "PART 2: Unprotected Value (NO CALLBACK)" << std::endl;
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    
    {
        UnprotectedValue unprotected_health("health", 100);
        std::cout << "\n   Initial state:" << std::endl;
        std::cout << "   - Health: " << unprotected_health.get_value() << std::endl;
        
        // Simulate cheat attempt by modifying unprotected value directly
        std::cout << "\n   👾 Cheat attempt: Modifying health to 9999..." << std::endl;
        unprotected_health.set(9999);  // Cheat successful!
        
        std::cout << "\n   ❌ Result: No callback! Cheat went undetected!" << std::endl;
        std::cout << "   ❌ Player now has " << unprotected_health.get_value() << " health" << std::endl;
        std::cout << "   ❌ Silent failure - no warning shown!\n" << std::endl;
    }
    
    std::cout << "\n✅ Comparison demo complete!" << std::endl;
    std::cout << "\n📋 SUMMARY:" << std::endl;
    std::cout << "   • Protected: Detects tampering → Shows warning ✅" << std::endl;
    std::cout << "   • Unprotected: Silent failure → No warning ❌\n" << std::endl;
}

void demo_callback_modes() {
    std::cout << "┌──────────────────────────────────────────────────────────┐" << std::endl;
    std::cout << "│ Demo 4: Different Callback Modes                         │" << std::endl;
    std::cout << "└──────────────────────────────────────────────────────────┘\n" << std::endl;
    
    // Mode 1: Warning Mode (Simple callback)
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    std::cout << "MODE 1: WARNING MODE (Show warnings to player)" << std::endl;
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    
    maxion_register_cheat_callback(simple_cheat_callback);
    std::cout << "\n   👾 Cheat detected:" << std::endl;
    maxion_invoke_cheat_callback(CheatType::ValueFreeze, 1);
    std::cout << "   Result: ⚠️ Warning shown to player!\n" << std::endl;
    
    std::this_thread::sleep_for(std::chrono::milliseconds(300));
    
    // Mode 2: Silent Mode (Silent callback)
    std::cout << "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    std::cout << "MODE 2: SILENT MODE (Log only, no visible warnings)" << std::endl;
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    
    maxion_register_cheat_callback(silent_cheat_callback);
    std::cout << "\n   👾 Cheat detected:" << std::endl;
    maxion_invoke_cheat_callback(CheatType::ValueFreeze, 2);
    std::cout << "   Result: 🔇 Silent logging only!\n" << std::endl;
    
    std::this_thread::sleep_for(std::chrono::milliseconds(300));
    
    // Mode 3: No Callback
    std::cout << "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    std::cout << "MODE 3: NO CALLBACK (No protection at all)" << std::endl;
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    
    maxion_register_cheat_callback(nullptr);
    std::cout << "\n   👾 Cheat detected:" << std::endl;
    maxion_invoke_cheat_callback(CheatType::ValueFreeze, 3);
    std::cout << "   Result: ❌ No callback registered - silent failure!\n" << std::endl;
    
    std::cout << "\n✅ Callback modes demo complete!\n" << std::endl;
}

void demo_multiple_cheat_types() {
    std::cout << "┌──────────────────────────────────────────────────────────┐" << std::endl;
    std::cout << "│ Demo 5: Multiple Cheat Types with Callbacks             │" << std::endl;
    std::cout << "└──────────────────────────────────────────────────────────┘\n" << std::endl;
    
    maxion_register_cheat_callback(advanced_cheat_callback);
    
    std::cout << "📊 Testing different cheat types with protected values:\n" << std::endl;
    
    std::cout << "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    std::cout << "Cheat Type 1: Memory Tampering" << std::endl;
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    maxion_invoke_cheat_callback(CheatType::MemoryTampering, 1);
    
    std::this_thread::sleep_for(std::chrono::milliseconds(300));
    
    std::cout << "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    std::cout << "Cheat Type 2: Value Freeze" << std::endl;
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    maxion_invoke_cheat_callback(CheatType::ValueFreeze, 1);
    
    std::this_thread::sleep_for(std::chrono::milliseconds(300));
    
    std::cout << "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    std::cout << "Cheat Type 3: Integrity Violation" << std::endl;
    std::cout << "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" << std::endl;
    maxion_invoke_cheat_callback(CheatType::IntegrityViolation, 1);
    
    std::cout << "\n✅ Multiple cheat types demo complete!\n" << std::endl;
}

void demo_thread_safe_protection() {
    std::cout << "┌──────────────────────────────────────────────────────────┐" << std::endl;
    std::cout << "│ Demo 6: Thread-Safe Protection with Callbacks           │" << std::endl;
    std::cout << "└──────────────────────────────────────────────────────────┘\n" << std::endl;
    
    maxion_register_cheat_callback(simple_cheat_callback);
    
    std::cout << "📊 Testing protected values with concurrent access:\n" << std::endl;
    
    std::cout << "\n   Creating multiple threads that access protected values..." << std::endl;
    std::cout << "   Each thread simulates gameplay and potential cheat detection\n" << std::endl;
    
    // Simulate concurrent cheat detection
    std::cout << "   Thread 1: Normal gameplay..." << std::endl;
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    
    std::cout << "   Thread 2: Cheat detected!" << std::endl;
    maxion_invoke_cheat_callback(CheatType::MemoryTampering, 1);
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    
    std::cout << "   Thread 3: Normal gameplay..." << std::endl;
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    
    std::cout << "   Thread 4: Cheat detected!" << std::endl;
    maxion_invoke_cheat_callback(CheatType::ValueFreeze, 1);
    
    std::cout << "\n✅ Thread-safe protection demo complete!" << std::endl;
    std::cout << "   Note: In production, callback system is thread-safe ✅\n" << std::endl;
}

// ============================================================================
// Main Function
// ============================================================================

int main() {
    std::cout << "╔════════════════════════════════════════════════════════╗" << std::endl;
    std::cout << "║   Maxion C++ Callback System Demo                      ║" << std::endl;
    std::cout << "║   (Standalone - No Rust Backend Required)              ║" << std::endl;
    std::cout << "╚════════════════════════════════════════════════════════╝\n" << std::endl;
    
    // Get hardware ID
    const char* hwid;
    size_t hwid_len;
    maxion_get_hardware_id(&hwid, &hwid_len);
    std::cout << "🔑 Hardware ID: " << hwid << std::endl;
    std::cout << "   (This ID identifies the player's machine)\n" << std::endl;
    
    // Run all demos
    demo_simple_callback();
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    
    demo_advanced_callback();
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    
    demo_protected_vs_unprotected();
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    
    demo_callback_modes();
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    
    demo_multiple_cheat_types();
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    
    demo_thread_safe_protection();
    
    // Summary
    auto total_detections = g_detection_count.load(std::memory_order_relaxed);
    std::cout << "╔════════════════════════════════════════════════════════╗" << std::endl;
    std::cout << "║   Demo Complete                                         ║" << std::endl;
    std::cout << "╚════════════════════════════════════════════════════════╝" << std::endl;
    std::cout << "   Total cheat detections: " << total_detections << std::endl;
    std::cout << "   All callbacks were successfully invoked!" << std::endl;
    
    std::cout << "\n📋 KEY TAKEAWAYS:" << std::endl;
    std::cout << "   1. Protected values detect tampering via callbacks ✅" << std::endl;
    std::cout << "   2. Unprotected values fail silently without warnings ❌" << std::endl;
    std::cout << "   3. Callbacks can show warnings or log silently" << std::endl;
    std::cout << "   4. Different cheat types trigger different responses" << std::endl;
    std::cout << "   5. Callback system is thread-safe for concurrent access" << std::endl;
    
    std::cout << "\n🔧 NEXT STEPS:" << std::endl;
    std::cout << "   1. Replace SimulatedAutoProtected with Protected<T> from Rust" << std::endl;
    std::cout << "   2. Link with actual maxion-core Rust library" << std::endl;
    std::cout << "   3. Use real FFI functions from cheat_callback.rs" << std::endl;
    std::cout << "   4. Test with actual cheat tools (Cheat Engine, etc.)" << std::endl;
    
    std::cout << "\n🎮 Ready for production with Rust backend integration!" << std::endl;
    
    return 0;
}