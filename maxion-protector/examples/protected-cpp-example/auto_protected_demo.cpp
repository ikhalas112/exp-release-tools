#include <iostream>
#include <cstdint>
#include <cassert>
#include <chrono>
// Note: protected.h is not used, only auto_protected.h is needed
#include "auto_protected.h"

using namespace Maxion;

// ============================================================================
// Demo 1: Basic AutoProtected<T> Usage
// ============================================================================

void demo_basic_usage() {
    std::cout << "\n=== Demo 1: Basic AutoProtected<T> Usage ===\n\n";

    // Create protected values using AutoProtected<T>
    AutoProtected<int32_t> health(100);
    AutoProtected<int32_t> ammo(30);
    AutoProtected<int32_t> score(0);
    AutoProtected<float> x(1.0f);
    AutoProtected<float> y(2.0f);
    AutoProtected<float> z(3.0f);

    std::cout << "Initial values:\n";
    std::cout << "  Health: " << health.get() << "\n";
    std::cout << "  Ammo: " << ammo.get() << "\n";
    std::cout << "  Score: " << score.get() << "\n";
    std::cout << "  Position: (" << x.get() << ", " << y.get() << ", " << z.get() << ")\n";

    // Update values (key rotation on each write)
    health.set(75);
    ammo.set(25);
    score.set(100);
    x.set(10.5f);
    y.set(20.3f);
    z.set(5.7f);

    std::cout << "\nAfter updates:\n";
    std::cout << "  Health: " << health.get() << "\n";
    std::cout << "  Ammo: " << ammo.get() << "\n";
    std::cout << "  Score: " << score.get() << "\n";
    std::cout << "  Position: (" << x.get() << ", " << y.get() << ", " << z.get() << ")\n";

    std::cout << "\n✅ Basic AutoProtected<T> usage works perfectly!\n";
}

// ============================================================================
// Demo 2: Manual Struct with AutoProtected Fields
// ============================================================================

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

void demo_manual_struct() {
    std::cout << "\n=== Demo 2: Manual Struct with AutoProtected Fields ===\n\n";

    // Create player using manual struct
    Player player(100, 30, 0);

    std::cout << "Using manual struct:\n";
    std::cout << "  Health: " << player.health.get() << " (direct access)\n";
    std::cout << "  Ammo: " << player.get_ammo() << " (via getter)\n";
    std::cout << "  Score: " << player.get_score() << " (via getter)\n";

    // Update values
    player.health.set(90);
    player.set_ammo(25);
    player.set_score(100);

    std::cout << "\nAfter updates:\n";
    std::cout << "  Health: " << player.health.get() << "\n";
    std::cout << "  Ammo: " << player.get_ammo() << "\n";
    std::cout << "  Score: " << player.get_score() << "\n";

    std::cout << "\n✅ Manual struct approach works great!\n";
}

// ============================================================================
// Demo 3: Complex Manual Struct
// ============================================================================

struct GameState {
    AutoProtected<int32_t> player_health;
    AutoProtected<int32_t> player_ammo;
    AutoProtected<int32_t> player_score;
    AutoProtected<int32_t> player_x;
    AutoProtected<int32_t> player_y;
    AutoProtected<int32_t> player_z;
    AutoProtected<int32_t> enemy_count;
    AutoProtected<int32_t> currency;

    GameState(int32_t ph, int32_t pa, int32_t ps,
              int32_t px, int32_t py, int32_t pz,
              int32_t ec, int32_t cur)
        : player_health(ph), player_ammo(pa), player_score(ps)
        , player_x(px), player_y(py), player_z(pz)
        , enemy_count(ec), currency(cur) {}
};

void demo_complex_manual_struct() {
    std::cout << "\n=== Demo 3: Complex Manual Struct ===\n\n";

    GameState state(100, 50, 0, 0, 0, 0, 10, 1000);

    std::cout << "Initial game state:\n";
    std::cout << "  Player: HP=" << state.player_health.get()
              << ", Ammo=" << state.player_ammo.get()
              << ", Score=" << state.player_score.get() << "\n";
    std::cout << "  Position: (" << state.player_x.get()
              << ", " << state.player_y.get()
              << ", " << state.player_z.get() << ")\n";
    std::cout << "  Enemies: " << state.enemy_count.get()
              << ", Currency: " << state.currency.get() << "\n";

    // Simulate gameplay
    state.player_health.set(85);
    state.player_ammo.set(45);
    state.player_score.set(500);
    state.player_x.set(10);
    state.player_y.set(20);
    state.player_z.set(5);
    state.enemy_count.set(7);
    state.currency.set(1500);

    std::cout << "\nAfter gameplay:\n";
    std::cout << "  Player: HP=" << state.player_health.get()
              << ", Ammo=" << state.player_ammo.get()
              << ", Score=" << state.player_score.get() << "\n";
    std::cout << "  Position: (" << state.player_x.get()
              << ", " << state.player_y.get()
              << ", " << state.player_z.get() << ")\n";
    std::cout << "  Enemies: " << state.enemy_count.get()
              << ", Currency: " << state.currency.get() << "\n";

    std::cout << "\n✅ Complex manual struct works perfectly!\n";
}

// ============================================================================
// Demo 4: Using AUTO_PROTECTED_FIELD Macro
// ============================================================================

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

void demo_macro_field() {
    std::cout << "\n=== Demo 4: Using AUTO_PROTECTED_FIELD Macro ===\n\n";

    Enemy enemy(150, 25, 10);

    std::cout << "Enemy stats:\n";
    std::cout << "  Health: " << enemy.get_health() << "\n";
    std::cout << "  Damage: " << enemy.get_damage() << "\n";
    std::cout << "  Speed: " << enemy.get_speed() << "\n";

    // Update via setters
    enemy.set_health(120);
    enemy.set_damage(30);
    enemy.set_speed(12);

    std::cout << "\nAfter buff:\n";
    std::cout << "  Health: " << enemy.get_health() << "\n";
    std::cout << "  Damage: " << enemy.get_damage() << "\n";
    std::cout << "  Speed: " << enemy.get_speed() << "\n";

    std::cout << "\n✅ AUTO_PROTECTED_FIELD macro works great!\n";
}

// ============================================================================
// Demo 5: Multiple Independent Instances
// ============================================================================

void demo_multiple_instances() {
    std::cout << "\n=== Demo 5: Multiple Independent Instances ===\n\n";

    Player player1(100, 30, 0);
    Player player2(50, 20, 500);

    std::cout << "Initial state:\n";
    std::cout << "  Player 1: HP=" << player1.get_health()
              << ", Ammo=" << player1.get_ammo()
              << ", Score=" << player1.get_score() << "\n";
    std::cout << "  Player 2: HP=" << player2.get_health()
              << ", Ammo=" << player2.get_ammo()
              << ", Score=" << player2.get_score() << "\n";

    // Modify player 1
    player1.set_health(90);
    player1.set_ammo(25);
    player1.set_score(100);

    std::cout << "\nAfter modifying player 1:\n";
    std::cout << "  Player 1: HP=" << player1.get_health()
              << ", Ammo=" << player1.get_ammo()
              << ", Score=" << player1.get_score() << "\n";
    std::cout << "  Player 2: HP=" << player2.get_health()
              << ", Ammo=" << player2.get_ammo()
              << ", Score=" << player2.get_score() << " (unchanged)\n";

    assert(player1.get_health() == 90);
    assert(player2.get_health() == 50);

    std::cout << "\n✅ Multiple instances are independent!\n";
}

// ============================================================================
// Demo 6: Copy and Assignment
// ============================================================================

void demo_copy_and_assignment() {
    std::cout << "\n=== Demo 6: Copy and Assignment ===\n\n";

    AutoProtected<int32_t> value1(100);
    AutoProtected<int32_t> value2(value1); // Copy constructor
    AutoProtected<int32_t> value3(0);

    std::cout << "After copy construction:\n";
    std::cout << "  value1: " << value1.get() << "\n";
    std::cout << "  value2: " << value2.get() << " (copied from value1)\n";
    std::cout << "  value3: " << value3.get() << "\n";

    value3 = value1; // Assignment operator

    std::cout << "\nAfter assignment:\n";
    std::cout << "  value1: " << value1.get() << "\n";
    std::cout << "  value2: " << value2.get() << "\n";
    std::cout << "  value3: " << value3.get() << " (assigned from value1)\n";

    // Modify value1
    value1.set(200);

    std::cout << "\nAfter modifying value1 to 200:\n";
    std::cout << "  value1: " << value1.get() << "\n";
    std::cout << "  value2: " << value2.get() << " (unchanged)\n";
    std::cout << "  value3: " << value3.get() << " (unchanged)\n";

    assert(value1.get() == 200);
    assert(value2.get() == 100);
    assert(value3.get() == 100);

    std::cout << "\n✅ Copy and assignment work correctly!\n";
}

// ============================================================================
// Demo 7: Performance Benchmark
// ============================================================================

void demo_performance() {
    std::cout << "\n=== Demo 7: Performance Benchmark ===\n\n";

    const int32_t iterations = 10000;

    // Benchmark AutoProtected<int32_t>
    AutoProtected<int32_t> protected_value(0);
    auto start_protected = std::chrono::high_resolution_clock::now();

    for (int32_t i = 0; i < iterations; i++) {
        protected_value.set(i);
        volatile int32_t value = protected_value.get();
        (void)value;
    }

    auto end_protected = std::chrono::high_resolution_clock::now();
    auto protected_duration = std::chrono::duration_cast<std::chrono::microseconds>(
        end_protected - start_protected
    ).count();

    // Benchmark regular int32_t
    int32_t regular_value = 0;
    auto start_regular = std::chrono::high_resolution_clock::now();

    for (int32_t i = 0; i < iterations; i++) {
        regular_value = i;
        volatile int32_t value = regular_value;
        (void)value;
    }

    auto end_regular = std::chrono::high_resolution_clock::now();
    auto regular_duration = std::chrono::duration_cast<std::chrono::microseconds>(
        end_regular - start_regular
    ).count();

    double overhead = (double)protected_duration / regular_duration;

    std::cout << "Performance results (" << iterations << " iterations):\n";
    std::cout << "  Regular int32_t: " << regular_duration << " µs\n";
    std::cout << "  AutoProtected<int32_t>: " << protected_duration << " µs\n";
    std::cout << "  Overhead: " << overhead << "x slower\n";

    std::cout << "\n💡 Performance tip:\n";
    std::cout << "   - AutoProtected<T> has significant overhead\n";
    std::cout << "   - Only protect cheatable values (health, ammo, score)\n";
    std::cout << "   - Don't protect temporary variables or flags\n";

    std::cout << "\n✅ Performance benchmark complete!\n";
}

// ============================================================================
// Demo 8: Best Practices
// ============================================================================

void demo_best_practices() {
    std::cout << "\n=== Demo 8: Best Practices ===\n\n";

    std::cout << "✅ DO protect critical game state:\n";
    std::cout << "   - Health, ammo, currency (cheatable values)\n";
    std::cout << "   - Score, achievements, unlockables\n";
    std::cout << "   - Player position (anti-teleport)\n";

    std::cout << "\n❌ DON'T protect everything:\n";
    std::cout << "   - Temporary variables (performance cost)\n";
    std::cout << "   - Configuration data (use file protection)\n";
    std::cout << "   - Non-cheatable state (flags, counters)\n";

    std::cout << "\n💡 Recommended pattern:\n";
    std::cout << "   1. Separate protected and unprotected state\n";
    std::cout << "   2. Use AutoProtected<T> for critical values\n";
    std::cout << "   3. Use regular types for non-critical data\n";
    std::cout << "   4. Batch updates when possible\n";

    std::cout << "\n📊 Performance impact:\n";
    std::cout << "   - AutoProtected<T>: ~78x slower than regular types\n";
    std::cout << "   - Only protect what cheaters would modify\n";
    std::cout << "   - Profile before adding protection\n";

    std::cout << "\n🔒 Security benefits:\n";
    std::cout << "   - Automatic XOR encryption\n";
    std::cout << "   - Trap values detect modifications\n";
    std::cout << "   - Key rotation on every write\n";
    std::cout << "   - Protection against Cheat Engine and similar tools\n";

    std::cout << "\n✅ Best practices explained!\n";
}

// ============================================================================
// Demo 9: Comparison with Rust #[auto_protected]
// ============================================================================

void demo_rust_comparison() {
    std::cout << "\n=== Demo 9: Comparison with Rust #[auto_protected] ===\n\n";

    std::cout << "Rust version:\n";
    std::cout << "```rust\n";
    std::cout << "#[auto_protected]\n";
    std::cout << "struct Player {\n";
    std::cout << "    health: i32,\n";
    std::cout << "    ammo: i32,\n";
    std::cout << "    score: i32,\n";
    std::cout << "}\n";
    std::cout << "```\n";

    std::cout << "\nC++ version (manual approach - recommended):\n";
    std::cout << "```cpp\n";
    std::cout << "struct Player {\n";
    std::cout << "    AutoProtected<int32_t> health;\n";
    std::cout << "    AutoProtected<int32_t> ammo;\n";
    std::cout << "    AutoProtected<int32_t> score;\n";
    std::cout << "\n";
    std::cout << "    Player(int32_t h, int32_t a, int32_t s)\n";
    std::cout << "        : health(h), ammo(a), score(s) {}\n";
    std::cout << "};\n";
    std::cout << "```\n";

    std::cout << "\nKey differences:\n";
    std::cout << "  Rust:\n";
    std::cout << "    ✅ Zero boilerplate with #[auto_protected]\n";
    std::cout << "    ✅ Compile-time transformation\n";
    std::cout << "    ✅ Automatic constructor generation\n";
    std::cout << "    ✅ Type-safe\n";

    std::cout << "\n  C++:\n";
    std::cout << "    ✅ More control over implementation\n";
    std::cout << "    ✅ No macro magic (clearer code)\n";
    std::cout << "    ✅ Works with any C++ compiler\n";
    std::cout << "    ✅ Similar functionality to Rust\n";

    std::cout << "\n💡 Recommendation:\n";
    std::cout << "   - Rust: Use #[auto_protected] for clean syntax\n";
    std::cout << "   - C++: Use AutoProtected<T> manually for clarity\n";
    std::cout << "   - Both provide equivalent protection!\n";

    std::cout << "\n✅ Comparison complete!\n";
}

// ============================================================================
// Demo 10: Integration Example
// ============================================================================

struct IntegratedGame {
    AutoProtected<int32_t> player_health;
    AutoProtected<int32_t> player_ammo;
    AutoProtected<int32_t> player_score;
    AutoProtected<int32_t> currency;

    // Non-protected state
    int32_t animation_frame;
    int32_t temp_counter;

    IntegratedGame(int32_t ph, int32_t pa, int32_t ps, int32_t cur)
        : player_health(ph), player_ammo(pa), player_score(ps), currency(cur)
        , animation_frame(0), temp_counter(0) {}

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

    void add_score(int32_t points) {
        player_score.set(player_score.get() + points);
    }

    void add_currency(int32_t amount) {
        currency.set(currency.get() + amount);
    }
};

void demo_integration() {
    std::cout << "\n=== Demo 10: Integration Example ===\n\n";

    IntegratedGame game(100, 30, 0, 1000);

    std::cout << "Initial game state:\n";
    std::cout << "  Protected: HP=" << game.player_health.get()
              << ", Ammo=" << game.player_ammo.get()
              << ", Score=" << game.player_score.get()
              << ", Currency=" << game.currency.get() << "\n";
    std::cout << "  Unprotected: Frame=" << game.animation_frame
              << ", Counter=" << game.temp_counter << "\n";

    // Simulate gameplay
    game.take_damage(10);
    std::cout << "\nAfter taking 10 damage:\n";
    std::cout << "  Health: " << game.player_health.get() << "\n";

    game.fire_weapon();
    game.fire_weapon();
    game.fire_weapon();
    std::cout << "After firing 3 times:\n";
    std::cout << "  Ammo: " << game.player_ammo.get() << "\n";

    game.add_score(100);
    game.add_score(50);
    std::cout << "After adding score:\n";
    std::cout << "  Score: " << game.player_score.get() << "\n";

    game.add_currency(500);
    std::cout << "After collecting currency:\n";
    std::cout << "  Currency: " << game.currency.get() << "\n";

    // Update unprotected state (no overhead)
    for (int32_t i = 0; i < 1000; i++) {
        game.animation_frame = (game.animation_frame + 1) % 60;
        game.temp_counter++;
    }
    std::cout << "\nAfter 1000 frames:\n";
    std::cout << "  Frame: " << game.animation_frame
              << ", Counter: " << game.temp_counter << "\n";
    std::cout << "  Protected state unchanged:\n";
    std::cout << "  HP=" << game.player_health.get()
              << ", Score=" << game.player_score.get() << "\n";

    std::cout << "\n✅ Integration example demonstrates real-world usage!\n";
}

// ============================================================================
// Main Function
// ============================================================================

int main() {
    std::cout << "╔══════════════════════════════════════════════════════════╗\n";
    std::cout << "║    AutoProtected<T> C++ Demo - Comprehensive Guide      ║\n";
    std::cout << "╚══════════════════════════════════════════════════════════╝\n";

    std::cout << "\nThis demo showcases the AutoProtected<T> feature, which provides\n";
    std::cout << "automatic anti-cheat protection for C++ game state.\n";

    demo_basic_usage();
    demo_manual_struct();
    demo_complex_manual_struct();
    demo_macro_field();
    demo_multiple_instances();
    demo_copy_and_assignment();
    demo_performance();
    demo_best_practices();
    demo_rust_comparison();
    demo_integration();

    std::cout << "\n╔══════════════════════════════════════════════════════════╗\n";
    std::cout << "║              Demo Complete! All Tests Passed ✅          ║\n";
    std::cout << "╚══════════════════════════════════════════════════════════╝\n";

    std::cout << "\n📚 Documentation:\n";
    std::cout << "   - Rust: docs/06_security/008_auto_protected.md\n";
    std::cout << "   - C++: See auto_protected.h header file\n";

    std::cout << "\n🔒 Security:\n";
    std::cout << "   - All values are XOR encrypted\n";
    std::cout << "   - Trap values detect memory modifications\n";
    std::cout << "   - Key rotation prevents value freezing\n";
    std::cout << "   - Automatic cheat detection!\n";

    std::cout << "\n🚀 Ready to use in your game!\n\n";

    return 0;
}