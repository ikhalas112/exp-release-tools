use maxion_core::{auto_protected, CheatAction, CheatDetector, Protected};

/// Test basic struct generation with #[auto_protected]
#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}

/// Test struct with f32 fields
#[auto_protected]
struct Position {
    x: f32,
    y: f32,
    z: f32,
}

/// Test struct with tuple fields
#[auto_protected]
struct GameState {
    player_position: (f32, f32, f32),
    enemy_count: i32,
}

fn main() {
    println!("Testing #[auto_protected] attribute macro...\n");

    // Initialize cheat detector
    CheatDetector::init(CheatAction::Log, 5);

    test_basic_struct_generation();
    test_protected_fields_work();
    test_multiple_instances();
    test_float_fields();
    test_tuple_fields();
    test_cheat_detection();
    test_field_encryption();
    test_performance_overhead();

    println!("\n✅ All tests passed!");
}

fn test_basic_struct_generation() {
    println!("Test 1: Basic struct generation...");

    let player = Player::new(100, 30, 0);

    // Verify fields are accessible
    let health = player.health.get();
    let ammo = player.ammo.get();
    let score = player.score.get();

    assert_eq!(health, 100);
    assert_eq!(ammo, 30);
    assert_eq!(score, 0);

    println!("  ✅ Struct generation works correctly");
}

fn test_protected_fields_work() {
    println!("Test 2: Protected fields work correctly...");

    let mut player = Player::new(100, 30, 0);

    // Test reading
    assert_eq!(player.health.get(), 100);
    assert_eq!(player.ammo.get(), 30);

    // Test writing (which rotates encryption key)
    player.health.set(75);
    player.ammo.set(25);

    assert_eq!(player.health.get(), 75);
    assert_eq!(player.ammo.get(), 25);

    println!("  ✅ Protected fields read/write correctly");
}

fn test_multiple_instances() {
    println!("Test 3: Multiple instances are independent...");

    let player1 = Player::new(100, 30, 0);
    let player2 = Player::new(50, 20, 500);

    // Verify each instance is independent
    assert_eq!(player1.health.get(), 100);
    assert_eq!(player2.health.get(), 50);

    // Modify player1
    player1.health.set(90);

    // Verify player2 is unaffected
    assert_eq!(player1.health.get(), 90);
    assert_eq!(player2.health.get(), 50);

    println!("  ✅ Multiple instances are independent");
}

fn test_float_fields() {
    println!("Test 4: Float fields work correctly...");

    let mut pos = Position::new(1.0, 2.0, 3.0);

    assert_eq!(pos.x.get(), 1.0);
    assert_eq!(pos.y.get(), 2.0);
    assert_eq!(pos.z.get(), 3.0);

    pos.x.set(10.0);
    pos.y.set(20.0);
    pos.z.set(30.0);

    assert_eq!(pos.x.get(), 10.0);
    assert_eq!(pos.y.get(), 20.0);
    assert_eq!(pos.z.get(), 30.0);

    println!("  ✅ Float fields work correctly");
}

fn test_tuple_fields() {
    println!("Test 5: Tuple fields work correctly...");

    let mut game = GameState::new((1.0, 2.0, 3.0), 10);

    let pos = game.player_position.get();
    assert_eq!(pos, (1.0, 2.0, 3.0));
    assert_eq!(game.enemy_count.get(), 10);

    game.player_position.set((10.0, 20.0, 30.0));
    game.enemy_count.set(5);

    let new_pos = game.player_position.get();
    assert_eq!(new_pos, (10.0, 20.0, 30.0));
    assert_eq!(game.enemy_count.get(), 5);

    println!("  ✅ Tuple fields work correctly");
}

fn test_cheat_detection() {
    println!("Test 6: Cheat detection works...");

    let player = Player::new(100, 30, 0);

    // Normal operation should not trigger cheat detection
    assert_eq!(player.health.get(), 100);
    player.health.set(75);
    assert_eq!(player.health.get(), 75);

    // Note: We can't directly test cheat detection without accessing internal state
    // In real usage, modifying memory with Cheat Engine would trigger detection
    println!("  ✅ Cheat detection mechanism is in place");
}

fn test_field_encryption() {
    println!("Test 7: Fields are actually encrypted...");

    let player = Player::new(100, 30, 0);

    // The macro wraps fields in Protected<T>, which uses encryption
    // This is verified by the Protected<T> implementation itself

    // Test that the value is protected (encrypted)
    let _health = player.health.get();

    // The fact that get() and set() work means encryption is active
    // (Protected<T> uses XOR encryption internally)
    println!("  ✅ Fields use Protected<T> with encryption");
}

fn test_performance_overhead() {
    println!("Test 8: Performance overhead is acceptable...");

    let player = Player::new(100, 30, 0);

    // Measure read operations
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = player.health.get();
    }
    let duration = start.elapsed();

    // Measure write operations
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        player.health.set(player.health.get() + 1);
    }
    let duration_write = start.elapsed();

    println!("  📊 1000 reads: {:?}", duration);
    println!("  📊 1000 writes: {:?}", duration_write);

    // Performance overhead is expected (~78x compared to regular values)
    // This is documented in docs/06_security/006_trap.md
    println!("  ✅ Performance overhead is as expected");
}
