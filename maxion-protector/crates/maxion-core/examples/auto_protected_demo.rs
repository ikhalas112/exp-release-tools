use maxion_core::{auto_protected, CheatAction, CheatDetector};

/// Example 1: Basic Player struct with protected fields
#[auto_protected]
struct Player {
    health: i32,
    ammo: i32,
    score: i32,
}

/// Example 2: Game state with float fields
#[auto_protected]
struct GameState {
    player_position: (f32, f32, f32),
    enemy_count: i32,
    game_time: f32,
}

/// Example 3: Complex game state with mixed types
#[auto_protected]
#[allow(clippy::too_many_arguments)]
struct ComplexGameState {
    player_health: i32,
    player_ammo: i32,
    player_score: i32,
    player_x: f32,
    player_y: f32,
    player_z: f32,
    enemy_count: i32,
    currency: i32,
}

fn main() {
    println!("=== #[auto_protected] Attribute Demo ===\n");

    // Initialize cheat detector to log detected cheats
    CheatDetector::init(CheatAction::Log, 10);

    demo_basic_usage();
    demo_float_fields();
    demo_complex_state();
    demo_cheat_detection();
    demo_best_practices();

    println!("\n=== Demo Complete ===");
}

fn demo_basic_usage() {
    println!("--- Demo 1: Basic Usage ---");

    // Create a player with protected fields
    let player = Player::new(100, 30, 0);

    println!("Initial state:");
    println!("  Health: {}", player.health.get());
    println!("  Ammo: {}", player.ammo.get());
    println!("  Score: {}", player.score.get());

    // Update values (key rotation on each write)
    player.health.set(90);
    player.ammo.set(25);
    player.score.set(100);

    println!("\nAfter updates:");
    println!("  Health: {}", player.health.get());
    println!("  Ammo: {}", player.ammo.get());
    println!("  Score: {}", player.score.get());

    println!("\n✅ All fields are automatically protected!\n");
}

fn demo_float_fields() {
    println!("--- Demo 2: Float and Tuple Fields ---");

    let game = GameState::new((0.0, 0.0, 0.0), 5, 0.0);

    println!("Initial state:");
    let pos = game.player_position.get();
    println!("  Position: ({}, {}, {})", pos.0, pos.1, pos.2);
    println!("  Enemies: {}", game.enemy_count.get());
    println!("  Time: {:.2}", game.game_time.get());

    // Update game state
    game.player_position.set((10.5, 20.3, 5.7));
    game.enemy_count.set(3);
    game.game_time.set(15.5);

    println!("\nAfter updates:");
    let new_pos = game.player_position.get();
    println!("  Position: ({}, {}, {})", new_pos.0, new_pos.1, new_pos.2);
    println!("  Enemies: {}", game.enemy_count.get());
    println!("  Time: {:.2}", game.game_time.get());

    println!("\n✅ Float and tuple fields work seamlessly!\n");
}

fn demo_complex_state() {
    println!("--- Demo 3: Complex Game State ---");

    let state = ComplexGameState::new(
        100,  // health
        50,   // ammo
        0,    // score
        0.0,  // x
        0.0,  // y
        0.0,  // z
        10,   // enemies
        1000, // currency
    );

    println!("Initial state:");
    print_state(&state);

    // Simulate gameplay
    state.player_health.set(85);
    state.player_ammo.set(45);
    state.player_score.set(500);
    state.player_x.set(5.5);
    state.player_y.set(10.2);
    state.player_z.set(0.0);
    state.enemy_count.set(7);
    state.currency.set(1500);

    println!("\nAfter gameplay:");
    print_state(&state);

    println!("\n✅ Complex state management works perfectly!\n");
}

fn print_state(state: &ComplexGameState) {
    println!(
        "  Health: {} | Ammo: {} | Score: {}",
        state.player_health.get(),
        state.player_ammo.get(),
        state.player_score.get()
    );
    println!(
        "  Position: ({}, {}, {}) | Enemies: {}",
        state.player_x.get(),
        state.player_y.get(),
        state.player_z.get(),
        state.enemy_count.get()
    );
    println!("  Currency: {}", state.currency.get());
}

fn demo_cheat_detection() {
    println!("--- Demo 4: Cheat Detection Mechanism ---");

    let player = Player::new(100, 30, 0);

    println!("Normal operation:");
    println!("  Reading health: {}", player.health.get());
    println!("  Setting health to 75");
    player.health.set(75);
    println!("  Reading health: {}", player.health.get());

    println!("\n🔒 Protection in action:");
    println!("  - All fields are encrypted using XOR encryption");
    println!("  - Each write rotates the encryption key");
    println!("  - Trap values detect memory modifications");
    println!("  - Cheat Engine modifications will be detected");

    println!("\nNote: Actual cheat detection requires memory tampering,");
    println!("which cannot be safely demonstrated in this demo.");
    println!("In real usage, cheat attempts would trigger CheatDetector!");

    println!("\n✅ Protection mechanism is active and ready!\n");
}

fn demo_best_practices() {
    println!("--- Demo 5: Best Practices ---");

    println!("✅ DO protect critical game state:");
    println!("   - Health, ammo, currency (cheatable values)");
    println!("   - Score, achievements, unlockables");
    println!("   - Player position (anti-teleport)");

    println!("\n❌ DON'T protect everything:");
    println!("   - Temporary variables (performance cost)");
    println!("   - Configuration data (use file protection)");
    println!("   - Non-cheatable state (flags, counters)");

    println!("\n💡 Performance tip:");
    println!("   - Protected values have ~78x overhead");
    println!("   - Only protect what cheaters would want to modify");
    println!("   - Batch updates when possible");

    println!("\n🎯 Usage pattern:");
    println!("   1. Add #[auto_protected] to your structs");
    println!("   2. Use .get() to read values");
    println!("   3. Use .set() to update values");
    println!("   4. Cheat detection is automatic!");

    println!("\n✅ Best practices applied correctly!\n");
}
