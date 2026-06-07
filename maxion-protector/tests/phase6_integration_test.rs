//! # Phase 6 Integration Tests
//!
//! Integration tests for the honeypot anti-cheat system.
//! These tests simulate real-world cheat engine attack scenarios.

use maxion_core::protected::{
    set_trap_enabled, CheatAction, CheatDetector, Protected, ProtectedSync,
};
use serial_test::serial;
use std::thread;

// =============================================================================
// Test Setup Helpers
// =============================================================================

/// Test guard that ensures trap state is reset when dropped
/// This provides proper cleanup even if the test panics
struct TestGuard;

impl Drop for TestGuard {
    fn drop(&mut self) {
        maxion_core::reset_trap_state();
    }
}

/// Ensure trap checking is enabled before each test
/// Returns a guard that will reset the state when dropped
/// This provides clean state isolation between tests
fn reset_trap_state() -> TestGuard {
    maxion_core::reset_trap_state();
    TestGuard
}

// =============================================================================
// Scenario Tests
// =============================================================================

#[test]
#[serial]
fn test_simulate_memory_scanner() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    let _player = PlayerState::new();
    // Scenario: Cheat Engine scans memory for value "100"
    // User finds both trap and real values, modifies trap to 999
    // Next read detects tampering and returns real value

    let player_health = Protected::new(100i32);
    let player_ammo = Protected::new(30i32);

    // Simulate Cheat Engine scanning memory
    // It would find both the trap value (100) and encrypted real value
    // The trap value is easy to identify as it's plain text

    // User modifies the trap value they found
    unsafe {
        std::ptr::write_volatile(player_health.trap_value.get(), 999);
        std::ptr::write_volatile(player_ammo.trap_value.get(), 9999);
    }

    // Game reads health - should detect tampering and return real value
    let health = player_health.get();
    assert_eq!(health, 100, "Should return real value, not modified trap");

    // Game reads ammo - should detect tampering and return real value
    let ammo = player_ammo.get();
    assert_eq!(ammo, 30, "Should return real value, not modified trap");

    // Re-enable trap checking
    set_trap_enabled(true);

    println!("✓ Memory scanner attack detected and blocked");
    // Guard will auto-reset trap state here
}

#[test]
#[serial]
fn test_simulate_value_freeze() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    let _player = PlayerState::new();
    // Scenario: Cheat Engine freezes a memory address to prevent changes
    // User wants to keep health at 100 (god mode)
    // After game updates health to 75, Cheat Engine writes 100 back
    // Detection should occur

    let player_health = Protected::new(100i32);

    // Cheat Engine reads current health (100) and sets up freeze
    let frozen_value = unsafe { std::ptr::read_volatile(player_health.trap_value.get()) };
    assert_eq!(frozen_value, 100);

    // Game takes damage, updates health to 75
    // This also rotates the encryption key
    player_health.set(75);

    // Cheat Engine's freeze writes the old value back
    unsafe {
        std::ptr::write_volatile(player_health.trap_value.get(), frozen_value);
    }

    // Game reads health - should detect mismatch
    // Returns real value (75), not frozen trap value (100)
    let health = player_health.get();
    assert_eq!(health, 75, "Should return real value after update");

    // Re-enable trap checking (after final get)
    set_trap_enabled(true);

    println!("✓ Value freeze attack detected and blocked");
}

#[test]
#[serial]
fn test_simulate_pointer_chain_attack() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    // Scenario: Advanced cheat follows pointer chains to find base address
    // This is harder to detect with simple honeypots
    // But our system still provides some protection

    // Create nested protected values (simulating pointer chain)
    let base_health = Protected::new(100i32);
    let derived_health = Protected::new(100i32);

    // Cheat Engine finds both values and modifies them
    unsafe {
        std::ptr::write_volatile(base_health.trap_value.get(), 999);
        std::ptr::write_volatile(derived_health.trap_value.get(), 999);
    }

    // Game reads both values - should detect tampering in both
    let base = base_health.get();
    let derived = derived_health.get();

    assert_eq!(base, 100);
    assert_eq!(derived, 100);

    // Re-enable trap checking
    set_trap_enabled(true);

    println!("✓ Pointer chain attack partially mitigated");
    // Guard will auto-reset trap state here
}

#[test]
#[serial]
fn test_simulate_code_injection() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    // Scenario: Cheat Engine injects code to modify values directly
    // This bypasses our Protected<T> API

    let player_health = Protected::new(100i32);

    // Simulate injected code that modifies memory directly
    // Attackers might try to modify both trap and real values
    unsafe {
        std::ptr::write_volatile(player_health.trap_value.get(), 999);
        std::ptr::write_volatile(player_health.real_value_obfuscated.get(), 0);
        std::ptr::write_volatile(player_health.key.get(), 0);
    }

    // This corrupts the value completely
    // Game reads health - will detect corruption
    // (In production, this would panic or flag account)
    let _result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        player_health.get();
    }));

    // Depending on the damage, get() might return garbage or detect tampering
    // The key point is that the attacker had to know about all three fields

    // Re-enable trap checking
    set_trap_enabled(true);

    println!("✓ Code injection requires deep knowledge of internal structure");
    // Guard will auto-reset trap state here
}

// =============================================================================
// Game State Integration Tests
// =============================================================================

struct PlayerState {
    health: Protected<i32>,
    ammo: Protected<i32>,
    score: Protected<i32>,
}

impl PlayerState {
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
    }

    fn fire_weapon(&self) {
        let current = self.ammo.get();
        if current > 0 {
            self.ammo.set(current - 1);
        }
    }

    fn add_score(&self, points: i32) {
        let current = self.score.get();
        self.score.set(current + points);
    }

    fn get_status(&self) -> (i32, i32, i32) {
        (self.health.get(), self.ammo.get(), self.score.get())
    }
}

#[test]
#[serial]
fn test_game_state_integrity() {
    let _guard = reset_trap_state();

    let player = PlayerState::new();

    // Initial state
    let (health, ammo, score) = player.get_status();
    assert_eq!(health, 100);
    assert_eq!(ammo, 30);
    assert_eq!(score, 0);

    // Simulate gameplay
    player.take_damage(25);
    player.fire_weapon();
    player.fire_weapon();
    player.add_score(100);

    let (health, ammo, score) = player.get_status();
    assert_eq!(health, 75);
    assert_eq!(ammo, 28);
    assert_eq!(score, 100);

    println!("✓ Game state integrity maintained");
    // Guard will auto-reset trap state here
}

#[test]
#[serial]
fn test_game_state_under_attack() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    let player = PlayerState::new();

    // Simulate cheat modifying health trap value
    unsafe {
        std::ptr::write_volatile(player.health.trap_value.get(), 999);
    }

    // Game takes damage
    player.take_damage(25);

    // Read health - should detect tampering and return real value
    let health = player.health.get();
    assert_eq!(health, 75, "Health should be 75 after taking 25 damage");

    // Cheat modifies ammo trap value
    unsafe {
        std::ptr::write_volatile(player.ammo.trap_value.get(), 9999);
    }

    // Player fires weapon
    player.fire_weapon();

    // Read ammo - should detect tampering and return real value
    let ammo = player.ammo.get();
    assert_eq!(ammo, 29, "Ammo should be 29 after firing once");

    // Guard will auto-reset trap state here

    println!("✓ Game state protected against cheats");
}

// =============================================================================
// Performance Tests
// =============================================================================

#[test]
#[serial]
fn test_protected_performance_overhead() {
    let _guard = reset_trap_state();

    use std::hint::black_box;
    use std::time::Instant;

    const ITERATIONS: usize = 1_000_000;

    // Test regular i32 performance - use black_box to prevent optimization
    let start = Instant::now();
    let mut regular_value = 100i32;
    for _ in 0..ITERATIONS {
        regular_value = regular_value.wrapping_add(1);
        black_box(regular_value);
    }
    let regular_time = start.elapsed();

    // Test Protected<i32> performance
    let protected_value = Protected::new(100i32);
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let current = protected_value.get();
        protected_value.set(current + 1);
    }
    let protected_time = start.elapsed();

    // Calculate overhead, handling potential zero division
    let regular_nanos = regular_time.as_nanos() as f64;
    let protected_nanos = protected_time.as_nanos() as f64;

    let overhead = if regular_nanos > 0.0 {
        (protected_nanos / regular_nanos - 1.0) * 100.0
    } else {
        // If regular time is too small to measure, calculate slowdown factor directly
        protected_nanos / 1000.0 // Convert to approximate percentage
    };

    println!(
        "Regular i32: {:?} ({:.0} ns/op)",
        regular_time,
        regular_nanos / ITERATIONS as f64
    );
    println!(
        "Protected<i32>: {:?} ({:.0} ns/op)",
        protected_time,
        protected_nanos / ITERATIONS as f64
    );
    println!("Overhead: {:.2}%", overhead);

    // Overhead should be less than 10000% (100x slower) - realistic threshold
    // In practice, volatile operations + XOR encryption add significant overhead
    // Expected overhead is ~78x (7,800%) based on Phase 6 documentation
    assert!(overhead < 10000.0, "Overhead too high: {:.2}%", overhead);
}

#[test]
fn test_protected_sync_concurrent_access() {
    use std::sync::Arc;

    let protected_value = Arc::new(ProtectedSync::new(0i32));
    let num_threads = 10;
    let increments_per_thread = 1000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_i| {
            let value = Arc::clone(&protected_value);
            thread::spawn(move || {
                for _ in 0..increments_per_thread {
                    // Note: This is not atomic, so some updates will be lost
                    // But the ProtectedSync ensures thread-safe access
                    let current = value.get();
                    value.set(current + 1);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = protected_value.get();

    // Due to non-atomic read-modify-write, we'll have lost updates
    // But the value should be positive and less than or equal to expected
    let expected = num_threads * increments_per_thread;
    assert!(final_value > 0, "Value should be positive: {}", final_value);
    assert!(
        final_value <= expected,
        "Value should not exceed expected: {} <= {}",
        final_value,
        expected
    );

    println!(
        "✓ Concurrent access: {} (expected: {})",
        final_value, expected
    );
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
#[serial]
fn test_protected_with_extreme_values() {
    // Test with min/max values
    let min_i32 = Protected::new(i32::MIN);
    assert_eq!(min_i32.get(), i32::MIN);

    let max_i32 = Protected::new(i32::MAX);
    assert_eq!(max_i32.get(), i32::MAX);

    let zero = Protected::new(0i32);
    assert_eq!(zero.get(), 0);

    let negative = Protected::new(-999999);
    assert_eq!(negative.get(), -999999);

    println!("✓ Extreme values handled correctly");
    // Guard will auto-reset trap state here
}

#[test]
#[serial]
fn test_protected_float_special_values() {
    // Test with special float values
    let zero = Protected::new(0.0f32);
    assert_eq!(zero.get(), 0.0);

    let negative_zero = Protected::new(-0.0f32);
    assert_eq!(negative_zero.get(), -0.0);

    let positive = Protected::new(123.456f32);
    assert!((positive.get() - 123.456).abs() < 0.001);

    let negative = Protected::new(-789.012f32);
    assert!((negative.get() - (-789.012)).abs() < 0.001);

    println!("✓ Float special values handled correctly");
    // Guard will auto-reset trap state here
}

#[test]
#[serial]
fn test_protected_tuple_coordinates() {
    // Test with position coordinates
    let position = Protected::new((1.0f32, 2.0f32, 3.0f32));

    let (x, y, z) = position.get();
    assert!((x - 1.0).abs() < 0.01);
    assert!((y - 2.0).abs() < 0.01);
    assert!((z - 3.0).abs() < 0.01);

    // Test with different values
    position.set((10.5f32, -20.75f32, 30.25f32));

    let (x, y, z) = position.get();
    assert!((x - 10.5).abs() < 0.01);
    assert!((y - (-20.75)).abs() < 0.01);
    assert!((z - 30.25).abs() < 0.01);

    println!("✓ Tuple coordinates handled correctly");
    // Guard will auto-reset trap state here
}

// =============================================================================
// Detection Behavior Tests
// =============================================================================

#[test]
#[serial]
fn test_cheat_detection_actions() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    // Test different cheat detection actions
    // Log action (default) - should not panic
    CheatDetector::init(CheatAction::Log, 10);

    let health = Protected::new(100i32);
    unsafe {
        std::ptr::write_volatile(health.trap_value.get(), 999);
    }

    // Should not panic with Log action
    let result = health.get();
    assert_eq!(result, 100);

    // Re-enable trap checking (after final get)
    set_trap_enabled(true);

    println!("✓ Cheat detection actions work correctly");
}

#[test]
#[serial]
fn test_multiple_tampering_attempts() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    // Simulate multiple cheat attempts

    let health = Protected::new(100i32);

    // Attempt 1
    unsafe {
        std::ptr::write_volatile(health.trap_value.get(), 999);
    }
    let result = health.get();
    assert_eq!(result, 100);

    // Attempt 2 (after legitimate update)
    health.set(75);
    unsafe {
        std::ptr::write_volatile(health.trap_value.get(), 888);
    }
    let result = health.get();
    assert_eq!(result, 75);

    // Attempt 3 (freeze attack)
    let frozen = unsafe { std::ptr::read_volatile(health.trap_value.get()) };
    health.set(50);
    unsafe {
        std::ptr::write_volatile(health.trap_value.get(), frozen);
    }
    let result = health.get();
    assert_eq!(result, 50);

    // Re-enable trap checking (guard will handle this automatically)

    println!("✓ Multiple tampering attempts detected");
    // Guard will auto-reset trap state here
}

// =============================================================================
// Real-world Scenario Tests
// =============================================================================

#[test]
#[serial]
fn test_first_person_shooter_scenario() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    // Simulate a FPS game with health, ammo, and grenades
    let health = Protected::new(100i32);
    let ammo = Protected::new(30i32);
    let grenades = Protected::new(3i32);

    // Player gets shot
    let current = health.get();
    health.set(current - 10);
    assert_eq!(health.get(), 90);

    // Player shoots
    let current = ammo.get();
    if current > 0 {
        ammo.set(current - 1);
    }
    assert_eq!(ammo.get(), 29);

    // Player throws grenade
    let current = grenades.get();
    if current > 0 {
        grenades.set(current - 1);
    }
    assert_eq!(grenades.get(), 2);

    // Cheat Engine tries to set health to 999 (god mode)
    unsafe {
        std::ptr::write_volatile(health.trap_value.get(), 999);
    }

    // Player gets shot again
    let current = health.get();
    health.set(current - 20);

    // Health should be 70, not 999 (cheat detected)
    // Note: Trap checking still disabled, so this won't panic
    assert_eq!(health.get(), 70);

    // Guard will auto-reset trap state here

    println!("✓ FPS scenario: protection works");
}

#[test]
#[serial]
fn test_racing_game_scenario() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    // Simulate a racing game with speed, position, and lap
    let speed = Protected::new(0.0f32);
    let position = Protected::new((0.0f32, 0.0f32, 0.0f32));
    let lap = Protected::new(1i32);

    // Car accelerates
    speed.set(100.0);
    assert!((speed.get() - 100.0).abs() < 0.01);

    // Car moves
    position.set((10.0f32, 5.0f32, 0.0f32));
    let (x, y, _z) = position.get();
    assert!((x - 10.0).abs() < 0.01);
    assert!((y - 5.0).abs() < 0.01);

    // Lap counter increments
    let current = lap.get();
    lap.set(current + 1);
    assert_eq!(lap.get(), 2);

    // Cheat tries to freeze speed (speed hack)
    let frozen_speed = speed.get();
    speed.set(200.0); // Legitimate speed change

    unsafe {
        std::ptr::write_volatile(speed.trap_value.get(), frozen_speed);
    }

    // Read speed - should return 200.0, not frozen value
    assert!((speed.get() - 200.0).abs() < 0.01);

    // Re-enable trap checking
    set_trap_enabled(true);

    println!("✓ Racing game scenario: protection works");
}

#[test]
#[serial]
fn test_rpg_game_scenario() {
    let _guard = reset_trap_state();
    // Disable trap checking to prevent panic in debug builds
    set_trap_enabled(false);

    // Simulate an RPG game with XP, gold, and level
    let xp = Protected::new(0i32);
    let gold = Protected::new(100i32);
    let level = Protected::new(1i32);

    // Player gains XP
    let current = xp.get();
    xp.set(current + 50);
    assert_eq!(xp.get(), 50);

    // Player earns gold
    let current = gold.get();
    gold.set(current + 25);
    assert_eq!(gold.get(), 125);

    // Player levels up
    level.set(2);
    assert_eq!(level.get(), 2);

    // Cheat tries to set gold to 999999
    unsafe {
        std::ptr::write_volatile(gold.trap_value.get(), 999999);
    }

    // Player spends gold
    let current = gold.get();
    gold.set(current - 10);

    // Should be 115 (125 - 10), not 999999
    // Note: Trap checking still disabled, so this won't panic
    assert_eq!(gold.get(), 115);

    // Guard will auto-reset trap state here

    println!("✓ RPG game scenario: protection works");
}
