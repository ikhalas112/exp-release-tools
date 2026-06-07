use maxion_core::{
    cheat_callback::CheatType,
    protected::{Protected, ProtectedSync},
    CheatAction, CheatDetector,
};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Example: Cheat Callback System Demo
///
/// This example demonstrates how to:
/// 1. Register a callback to receive cheat notifications
/// 2. Use protected values in your game
/// 3. Detect and respond to cheating attempts
/// 4. Use hardware ID for player identification
/// 5. Test with real Cheat Engine (interactive mode)
///
/// Global detection counter for demo purposes
static CHEAT_DETECTION_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Example 1: Simple callback function
extern "C" fn simple_cheat_callback(
    cheat_type: i32,
    hwid_ptr: *const u8,
    hwid_len: usize,
    timestamp: u64,
    detection_count: u32,
) {
    // Convert HWID to string for display
    let hwid = unsafe {
        if !hwid_ptr.is_null() && hwid_len > 0 {
            let slice = std::slice::from_raw_parts(hwid_ptr, hwid_len);
            std::str::from_utf8(slice).unwrap_or("<invalid>")
        } else {
            "<none>"
        }
    };

    let cheat_type_name = match cheat_type {
        0 => "MemoryTampering",
        1 => "ValueFreeze",
        2 => "IntegrityViolation",
        _ => "Unknown",
    };

    // Convert timestamp to readable format
    let datetime = timestamp / 1000; // Convert to seconds
    let hours = datetime / 3600;
    let minutes = (datetime % 3600) / 60;
    let seconds = datetime % 60;

    println!("\n🚨 ============================================");
    println!("🚨 CHEAT DETECTION ALERT!");
    println!("🚨 ============================================");
    println!("   Type: {}", cheat_type_name);
    println!("   HWID: {}", hwid);
    println!("   Time: {:02}:{:02}:{:02}", hours, minutes, seconds);
    println!("   Count: {} detection(s)", detection_count);
    println!("   Action: Notify Unity via callback");
    println!("🚨 ============================================\n");

    // Increment global counter for demo
    CHEAT_DETECTION_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Example 2: Advanced callback with detailed handling
extern "C" fn advanced_cheat_callback(
    cheat_type: i32,
    hwid_ptr: *const u8,
    hwid_len: usize,
    timestamp: u64,
    detection_count: u32,
) {
    let cheat_type_enum = CheatType::from_int(cheat_type);

    match cheat_type_enum {
        CheatType::MemoryTampering => {
            println!("⚠️  Memory tampering detected!");
            println!("   Player modified protected memory addresses");
            println!("   Recommended action: Log and monitor");
        }
        CheatType::ValueFreeze => {
            println!("❄️  Value freeze detected!");
            println!("   Player attempted to freeze game values");
            println!("   Recommended action: Immediate warning");
        }
        CheatType::IntegrityViolation => {
            println!("🔐 Integrity violation detected!");
            println!("   Code or memory integrity compromised");
            println!("   Recommended action: Disconnect player");
        }
        CheatType::Unknown => {
            println!("❓ Unknown cheat type detected!");
            println!("   Please investigate manually");
        }
    }

    // Log the event for server-side validation
    log_cheat_event(
        cheat_type_enum,
        hwid_ptr,
        hwid_len,
        timestamp,
        detection_count,
    );
}

/// Log cheat event (simulates server-side logging)
fn log_cheat_event(
    cheat_type: CheatType,
    hwid_ptr: *const u8,
    hwid_len: usize,
    timestamp: u64,
    detection_count: u32,
) {
    let hwid = unsafe {
        if !hwid_ptr.is_null() && hwid_len > 0 {
            let slice = std::slice::from_raw_parts(hwid_ptr, hwid_len);
            std::str::from_utf8(slice).unwrap_or("<invalid>")
        } else {
            "<none>"
        }
    };

    println!("   📝 Logging to server:");
    println!("      HWID: {}", hwid);
    println!("      Type: {:?}", cheat_type);
    println!("      Timestamp: {}", timestamp);
    println!("      Detections: {}", detection_count);
}

/// Player struct with protected values
struct Player {
    health: Protected<i32>,
    ammo: Protected<i32>,
    score: Protected<i32>,
}

impl Player {
    fn new() -> Self {
        Self {
            health: Protected::new(100),
            ammo: Protected::new(30),
            score: Protected::new(0),
        }
    }

    fn take_damage(&self, damage: i32) {
        let current = self.health.get();
        let new_health = (current - damage).max(0);
        self.health.set(new_health);
        println!("   Player took {} damage. Health: {}", damage, new_health);
    }

    fn fire_weapon(&self) {
        let current = self.ammo.get();
        if current > 0 {
            self.ammo.set(current - 1);
            println!("   Fired weapon! Ammo: {}", current - 1);
        } else {
            println!("   Out of ammo!");
        }
    }

    fn add_score(&self, points: i32) {
        let current = self.score.get();
        self.score.set(current + points);
        println!(
            "   Score increased by {}. Total: {}",
            points,
            current + points
        );
    }
}

fn main() {
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║   Maxion Anti-Cheat Callback System Demo                ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // Get hardware ID
    let hwid = maxion_core::cheat_callback::get_hardware_id();
    println!("🔑 Hardware ID: {}", hwid);
    println!("   (This ID identifies the player's machine)\n");

    // Demo 1: Simple callback registration
    demo_simple_callback();

    // Demo 2: Advanced callback registration
    demo_advanced_callback();

    // Demo 3: Multi-threaded protection
    demo_multi_threaded_protection();

    // Demo 4: Simulated cheating (triggers callback)
    demo_simulated_cheating();

    // Demo 5: Interactive mode (real Cheat Engine testing)
    demo_interactive_mode();

    // Summary
    let total_detections = CHEAT_DETECTION_COUNT.load(std::sync::atomic::Ordering::Relaxed);
    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║   Demo Complete                                           ║");
    println!("╚════════════════════════════════════════════════════════╝");
    println!("   Total cheat detections: {}", total_detections);
    println!("   All callbacks were successfully invoked!\n");
}

fn demo_simple_callback() {
    println!("┌──────────────────────────────────────────────────────────┐");
    println!("│ Demo 1: Simple Callback Registration                      │");
    println!("└──────────────────────────────────────────────────────────┘\n");

    // Initialize cheat detector with NotifyUnity action
    CheatDetector::init(CheatAction::NotifyUnity, 5);

    // Register simple callback
    maxion_core::cheat_callback::maxion_register_cheat_callback(Some(simple_cheat_callback));

    // Verify callback is registered
    let has_callback = maxion_core::cheat_callback::maxion_has_cheat_callback();
    println!("✅ Callback registered: {}", has_callback);

    // Create protected player
    let player = Player::new();
    println!("\n   Player created with protected values:");
    println!("   - Health: {}", player.health.get());
    println!("   - Ammo: {}", player.ammo.get());
    println!("   - Score: {}", player.score.get());

    // Normal gameplay
    println!("\n   🎮 Normal gameplay:");
    player.take_damage(10);
    player.fire_weapon();
    player.add_score(100);

    println!("\n✅ Simple callback demo complete!\n");
}

fn demo_advanced_callback() {
    println!("┌──────────────────────────────────────────────────────────┐");
    println!("│ Demo 2: Advanced Callback with Detailed Handling           │");
    println!("└──────────────────────────────────────────────────────────┘\n");

    // Switch to advanced callback
    maxion_core::cheat_callback::maxion_register_cheat_callback(Some(advanced_cheat_callback));

    println!("✅ Switched to advanced callback\n");

    // Create new player
    let player = Player::new();

    println!("   Simulating gameplay with advanced logging:");
    player.take_damage(20);
    player.fire_weapon();
    player.add_score(250);

    println!("\n✅ Advanced callback demo complete!\n");
}

fn demo_multi_threaded_protection() {
    println!("┌──────────────────────────────────────────────────────────┐");
    println!("│ Demo 3: Thread-Safe Protected Values                     │");
    println!("└──────────────────────────────────────────────────────────┘\n");

    // Create thread-safe protected health
    let health = Arc::new(ProtectedSync::new(100));
    let health2 = Arc::clone(&health);

    println!("   Creating two threads that share protected health value...\n");

    // Thread 1: Player takes damage
    let handle1 = thread::spawn(move || {
        for _i in 0..3 {
            let current = health.get();
            let new_health = (current - 10).max(0);
            health.set(new_health);
            println!("   Thread 1: Took damage. Health: {}", new_health);
            thread::sleep(Duration::from_millis(100));
        }
    });

    // Thread 2: Player heals
    let handle2 = thread::spawn(move || {
        for _i in 0..3 {
            let current = health2.get();
            let new_health = (current + 5).min(100);
            health2.set(new_health);
            println!("   Thread 2: Healed. Health: {}", new_health);
            thread::sleep(Duration::from_millis(150));
        }
    });

    // Wait for both threads
    handle1.join().unwrap();
    handle2.join().unwrap();

    println!("\n✅ Thread-safe protection works correctly!\n");
}

fn demo_simulated_cheating() {
    println!("┌──────────────────────────────────────────────────────────┐");
    println!("│ Demo 4: Manual Callback Invocation (Simulated Cheating)   │");
    println!("└──────────────────────────────────────────────────────────┘\n");

    println!("   Note: Actual memory tampering requires external tools\n");
    println!("   (Cheat Engine, ArtMoney, etc.) that modify RAM directly.\n");
    println!("   This demo manually invokes the callback to show how\n");
    println!("   it would work in real scenarios.\n");

    // Simulate memory tampering detection
    println!("   🎯 Simulating Memory Tampering detection:");
    println!("      (Cheat Engine modified protected value)\n");

    // Manually invoke callback to demonstrate what would happen
    maxion_core::cheat_callback::report_cheat_with_callback(CheatType::MemoryTampering, 1);

    thread::sleep(Duration::from_millis(500));

    // Simulate value freeze detection
    println!("   🎯 Simulating Value Freeze detection:");
    println!("      (Player attempted to freeze health to 999)\n");

    // Invoke callback again
    maxion_core::cheat_callback::report_cheat_with_callback(CheatType::ValueFreeze, 1);

    thread::sleep(Duration::from_millis(500));

    // Simulate integrity violation
    println!("   🎯 Simulating Integrity Violation detection:");
    println!("      (Code injection detected)\n");

    maxion_core::cheat_callback::report_cheat_with_callback(CheatType::IntegrityViolation, 1);

    thread::sleep(Duration::from_millis(500));

    // Show how it works in real gameplay
    println!("   🎮 In real gameplay:\n");
    println!("   1. Cheat Engine scans memory for value \"100\"");
    println!("   2. Finds trap value (100) and encrypted real value");
    println!("   3. Player modifies trap value to 999");
    println!("   4. Next read() detects mismatch (100 != 999)");
    println!("   5. CheatDetector invokes callback automatically!");
    println!("   6. Unity receives notification with HWID and details\n");

    println!("✅ Manual callback invocation demo complete!");
    println!("   In real usage, callbacks are triggered automatically!\n");
}

fn demo_interactive_mode() {
    println!("┌──────────────────────────────────────────────────────────┐");
    println!("│ Demo 5: Interactive Mode (Real Cheat Engine Test)      │");
    println!("└──────────────────────────────────────────────────────────┘\n");

    // Re-register simple callback for interactive demo
    maxion_core::cheat_callback::maxion_register_cheat_callback(Some(simple_cheat_callback));

    // Create player with easy-to-find values
    let player = Player::new();

    println!("   🎮 Interactive Mode Ready!");
    println!("   ══════════════════════════════════════════════════\n");

    println!("   Current values (easy to find in Cheat Engine):");
    println!("   📊 Health: {}", player.health.get());
    println!("   📊 Ammo: {}", player.ammo.get());
    println!("   📊 Score: {}\n", player.score.get());

    println!("   🔧 How to test with Cheat Engine:");
    println!("   ══════════════════════════════════════════════════\n");

    println!("   1. Start Cheat Engine");
    println!("   2. Click 'Select a process to open'");
    println!("   3. Select 'cheat_callback_demo' or this terminal");
    println!("   4. First Scan: Value = 100, Type = 4 Bytes");
    println!("   5. You'll find the trap value (visible in Cheat Engine)");
    println!("   6. Modify it to 999 (or any other value)");
    println!("   7. Press ENTER here to read the value");
    println!("   8. Watch the callback be triggered! 🎉\n");

    println!("   💡 Tips:");
    println!("   - Health value is 100 (easy to find)");
    println!("   - Ammo value is 30");
    println!("   - Score value is 0");
    println!("   - Try modifying any of them!\n");

    // Wait for user input
    println!("   ⌨️  Press ENTER to continue (or Ctrl+C to exit)...\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    stdout.flush().unwrap();
    stdin.lock().lines().next();

    // Now read all values - this will detect any tampering
    println!("\n   🔍 Checking for cheat modifications...\n");

    thread::sleep(Duration::from_millis(500));

    println!("   Reading Health: {}", player.health.get());
    thread::sleep(Duration::from_millis(200));

    println!("   Reading Ammo: {}", player.ammo.get());
    thread::sleep(Duration::from_millis(200));

    println!("   Reading Score: {}", player.score.get());

    println!("\n   ══════════════════════════════════════════════════");
    println!("   ✅ Interactive mode complete!");
    println!("   ══════════════════════════════════════════════════\n");

    // Unregister callback
    maxion_core::cheat_callback::maxion_register_cheat_callback(None);
}
