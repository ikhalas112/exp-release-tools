#ifndef MAXION_AUTO_PROTECTED_H
#define MAXION_AUTO_PROTECTED_H

#include <memory>
#include <type_traits>

namespace Maxion {

// Forward declaration of Protected<T>
template<typename T>
class Protected;

/**
 * @brief Auto-protected wrapper that provides automatic protection for values.
 * 
 * This template class automatically wraps values in the Protected<T> system
 * to provide anti-cheat protection with minimal code changes.
 * 
 * @tparam T The type of value to protect (must be supported by Protected<T>)
 * 
 * @note Supported types: int32_t, int64_t, uint32_t, uint64_t, float, and tuples
 * 
 * @example
 * AutoProtected<int32_t> health(100);
 * AutoProtected<float> x(1.0f);
 * 
 * // Read value
 * int32_t current = health.get();
 * 
 * // Write value
 * health.set(75);
 */
template<typename T>
class AutoProtected {
private:
    std::unique_ptr<Protected<T>> protected_value;

public:
    /**
     * @brief Construct a new AutoProtected object with the given initial value.
     * 
     * @param value The initial value to protect
     */
    explicit AutoProtected(const T& value) 
        : protected_value(new Protected<T>(value)) {
    }

    /**
     * @brief Construct a new AutoProtected object with default value.
     */
    AutoProtected() 
        : protected_value(new Protected<T>(T{})) {
    }

    /**
     * @brief Copy constructor (creates a new protected instance).
     * 
     * @param other The AutoProtected to copy from
     */
    AutoProtected(const AutoProtected<T>& other) 
        : protected_value(new Protected<T>(other.get())) {
    }

    /**
     * @brief Move constructor.
     * 
     * @param other The AutoProtected to move from
     */
    AutoProtected(AutoProtected<T>&& other) noexcept = default;

    /**
     * @brief Copy assignment operator.
     * 
     * @param other The AutoProtected to copy from
     * @return AutoProtected<T>& Reference to this object
     */
    AutoProtected<T>& operator=(const AutoProtected<T>& other) {
        if (this != &other) {
            set(other.get());
        }
        return *this;
    }

    /**
     * @brief Move assignment operator.
     * 
     * @param other The AutoProtected to move from
     * @return AutoProtected<T>& Reference to this object
     */
    AutoProtected<T>& operator=(AutoProtected<T>&& other) noexcept = default;

    /**
     * @brief Destructor.
     */
    ~AutoProtected() = default;

    /**
     * @brief Get the protected value.
     * 
     * This method decrypts the value and checks for tampering.
     * 
     * @return T The protected value
     */
    T get() const {
        return protected_value->get();
    }

    /**
     * @brief Set a new value.
     * 
     * This method encrypts the new value with a fresh key (key rotation).
     * 
     * @param value The new value to protect
     */
    void set(const T& value) {
        protected_value->set(value);
    }

    /**
     * @brief Get the raw protected value (advanced usage only).
     * 
     * This provides access to the underlying Protected<T> for advanced operations.
     * 
     * @return Protected<T>* Pointer to the protected value
     */
    Protected<T>* get_raw() {
        return protected_value.get();
    }

    /**
     * @brief Get the raw protected value (advanced usage only, const version).
     * 
     * This provides access to the underlying Protected<T> for advanced operations.
     * 
     * @return const Protected<T>* Pointer to the protected value
     */
    const Protected<T>* get_raw() const {
        return protected_value.get();
    }
};

/**
 * @brief Macro to declare a struct with auto-protected members.
 * 
 * This macro simplifies creating structs with protected fields by generating
 * the struct definition and constructor automatically.
 * 
 * @param STRUCT_NAME The name of the struct
 * @param FIELDS The field declarations (type name, ...)
 * 
 * @example
 * DECLARE_AUTO_PROTECTED_STRUCT(Player,
 *     int32_t health,
 *     int32_t ammo,
 *     int32_t score
 * )
 * 
 * // Usage:
 * Player player(100, 30, 0);
 * int32_t current_health = player.health.get();
 * player.health.set(75);
 */
#define DECLARE_AUTO_PROTECTED_STRUCT(STRUCT_NAME, ...) \
struct STRUCT_NAME { \
    __VA_ARGS__; \
    \
    struct FieldInitializer { \
        template<typename T> \
        static AutoProtected<T> init(const T& value) { \
            return AutoProtected<T>(value); \
        } \
    }; \
    \
    DECLARE_AUTO_PROTECTED_STRUCT_IMPL(STRUCT_NAME, __VA_ARGS__) \
}

// Helper macro to implement constructor
#define DECLARE_AUTO_PROTECTED_STRUCT_IMPL(STRUCT_NAME, ...) \
    STRUCT_NAME(FN_LIST(__VA_ARGS__)) : \
        FN_INIT_LIST(__VA_ARGS__) \
    {} \
    \
    DECLARE_AUTO_PROTECTED_STRUCT_GETTERS(STRUCT_NAME, __VA_ARGS__)

// Helper to get field names from field declarations
#define GET_FIELD_NAME(field) GET_FIRST_TOKEN(field)

// Helper to get field type from field declarations
#define GET_FIELD_TYPE(field) GET_SECOND_TOKEN(field)

// Macro to create parameter list for constructor
#define FN_LIST(...) \
    FN_LIST_IMPL(__VA_ARGS__,)

// Recursive implementation to expand field list
#define FN_LIST_IMPL(field, ...) \
    GET_FIELD_TYPE(field) GET_FIELD_NAME(field) \
    IF_ELSE(EMPTY_ARG(__VA_ARGS__),, FN_LIST_IMPL(__VA_ARGS__,))

// Macro to create initializer list for constructor
#define FN_INIT_LIST(...) \
    FN_INIT_LIST_IMPL(__VA_ARGS__,)

// Recursive implementation to create initializer list
#define FN_INIT_LIST_IMPL(field, ...) \
    GET_FIELD_NAME(field)(GET_FIELD_NAME(field)) \
    IF_ELSE(EMPTY_ARG(__VA_ARGS__),, FN_INIT_LIST_IMPL(__VA_ARGS__,))

// Macro to create getter methods for each field
#define DECLARE_AUTO_PROTECTED_STRUCT_GETTERS(STRUCT_NAME, ...) \
    __DECLARE_GETTERS_IMPL(STRUCT_NAME, __VA_ARGS__,)

// Recursive implementation to create getters
#define __DECLARE_GETTERS_IMPL(STRUCT_NAME, field, ...) \
    inline GET_FIELD_TYPE(field) get_##GET_FIELD_NAME(field)() const { \
        return GET_FIELD_NAME(field).get(); \
    } \
    inline void set_##GET_FIELD_NAME(field)(const GET_FIELD_TYPE(field)& value) { \
        GET_FIELD_NAME(field).set(value); \
    } \
    IF_ELSE(EMPTY_ARG(__VA_ARGS__),, __DECLARE_GETTERS_IMPL(STRUCT_NAME, __VA_ARGS__,))

// Helper macros for token manipulation
#define GET_FIRST_TOKEN(x) GET_FIRST_TOKEN_IMPL(x)
#define GET_FIRST_TOKEN_IMPL(x, ...) x

#define GET_SECOND_TOKEN(x) GET_SECOND_TOKEN_IMPL(x)
#define GET_SECOND_TOKEN_IMPL(_, x, ...) x

#define EMPTY_ARG(...) EMPTY_ARG_IMPL(__VA_ARGS__,0)
#define EMPTY_ARG_IMPL(...) GET_11TH_ARG(__VA_ARGS__, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0)
#define GET_11TH_ARG(_1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, ...) _11

#define IF_ELSE(cond, t, f) IF_ELSE_IMPL(cond, t, f)
#define IF_ELSE_IMPL(cond, t, f) IF_ELSE_##cond(t, f)

#define IF_ELSE_0(t, f) f
#define IF_ELSE_1(t, f) t

/**
 * @brief Simplified macro for auto-protecting struct fields.
 * 
 * Use this macro inside a struct definition to mark fields as auto-protected.
 * This provides a more manual but flexible approach compared to DECLARE_AUTO_PROTECTED_STRUCT.
 * 
 * @param TYPE The type of the field
 * @param NAME The name of the field
 * 
 * @example
 * struct Player {
 *     AUTO_PROTECTED_FIELD(int32_t, health);
 *     AUTO_PROTECTED_FIELD(int32_t, ammo);
 *     AUTO_PROTECTED_FIELD(int32_t, score);
 *     
 *     Player(int32_t h, int32_t a, int32_t s)
 *         : health(h), ammo(a), score(s) {}
 * };
 */
#define AUTO_PROTECTED_FIELD(TYPE, NAME) \
    AutoProtected<TYPE> NAME;

/**
 * @brief Macro to generate getter and setter methods for auto-protected fields.
 * 
 * This macro generates convenience getter/setter methods for protected fields,
 * making the API cleaner and more intuitive.
 * 
 * @param STRUCT_NAME The name of the struct
 * @param TYPE The type of the field
 * @param NAME The name of the field
 * 
 * @example
 * struct Player {
 *     AUTO_PROTECTED_FIELD(int32_t, health);
 *     AUTO_PROTECTED_FIELD(int32_t, ammo);
 *     
 *     Player(int32_t h, int32_t a) : health(h), ammo(a) {}
 *     
 *     AUTO_PROTECTED_GETTER_SETTER(Player, int32_t, health)
 *     AUTO_PROTECTED_GETTER_SETTER(Player, int32_t, ammo)
 * };
 * 
 * // Usage:
 * Player player(100, 30);
 * int32_t h = player.get_health();
 * player.set_health(75);
 */
#define AUTO_PROTECTED_GETTER_SETTER(STRUCT_NAME, TYPE, NAME) \
    inline TYPE get_##NAME() const { \
        return NAME.get(); \
    } \
    inline void set_##NAME(const TYPE& value) { \
        NAME.set(value); \
    }

/**
 * @brief Macro to declare a struct with auto-protected fields using a cleaner syntax.
 * 
 * This is an alternative to DECLARE_AUTO_PROTECTED_STRUCT that uses a field list syntax.
 * 
 * @param STRUCT_NAME The name of the struct
 * @param ... Field list as (TYPE, NAME) pairs
 * 
 * @example
 * DECLARE_AUTO_PROTECTED_STRUCT_V2(Player,
 *     (int32_t, health),
 *     (int32_t, ammo),
 *     (int32_t, score)
 * )
 */
#define DECLARE_AUTO_PROTECTED_STRUCT_V2(STRUCT_NAME, ...) \
struct STRUCT_NAME { \
    __DECLARE_FIELDS_V2(__VA_ARGS__) \
    \
    struct FieldInitializer { \
        template<typename T> \
        static AutoProtected<T> init(const T& value) { \
            return AutoProtected<T>(value); \
        } \
    }; \
    \
    __DECLARE_CONSTRUCTOR_V2(STRUCT_NAME, __VA_ARGS__) \
    __DECLARE_GETTERS_V2(STRUCT_NAME, __VA_ARGS__) \
}

// Helper macros for V2 syntax
#define __DECLARE_FIELDS_V2(...) \
    __DECLARE_FIELDS_V2_IMPL(__VA_ARGS__,)

#define __DECLARE_FIELDS_V2_IMPL(field, ...) \
    AutoProtected<GET_FIELD_TYPE_V2(field)> GET_FIELD_NAME_V2(field); \
    IF_ELSE(EMPTY_ARG(__VA_ARGS__),, __DECLARE_FIELDS_V2_IMPL(__VA_ARGS__,))

#define GET_FIELD_TYPE_V2(field) GET_FIRST_TOKEN_V2(field)
#define GET_FIELD_NAME_V2(field) GET_SECOND_TOKEN_V2(field)

#define GET_FIRST_TOKEN_V2(x) GET_FIRST_TOKEN_V2_IMPL(x)
#define GET_FIRST_TOKEN_V2_IMPL(x, ...) x

#define GET_SECOND_TOKEN_V2(x) GET_SECOND_TOKEN_V2_IMPL(x)
#define GET_SECOND_TOKEN_V2_IMPL(_, x, ...) x

#define __DECLARE_CONSTRUCTOR_V2(STRUCT_NAME, ...) \
    STRUCT_NAME(__DECLARE_PARAMS_V2(__VA_ARGS__)) \
        : __DECLARE_INIT_LIST_V2(__VA_ARGS__) \
    {}

#define __DECLARE_PARAMS_V2(...) \
    __DECLARE_PARAMS_V2_IMPL(__VA_ARGS__,)

#define __DECLARE_PARAMS_V2_IMPL(field, ...) \
    GET_FIELD_TYPE_V2(field) GET_FIELD_NAME_V2(field) \
    IF_ELSE(EMPTY_ARG(__VA_ARGS__),, __DECLARE_PARAMS_V2_IMPL(__VA_ARGS__,))

#define __DECLARE_INIT_LIST_V2(...) \
    __DECLARE_INIT_LIST_V2_IMPL(__VA_ARGS__,)

#define __DECLARE_INIT_LIST_V2_IMPL(field, ...) \
    GET_FIELD_NAME_V2(field)(GET_FIELD_NAME_V2(field)) \
    IF_ELSE(EMPTY_ARG(__VA_ARGS__),, __DECLARE_INIT_LIST_V2_IMPL(__VA_ARGS__,))

#define __DECLARE_GETTERS_V2(STRUCT_NAME, ...) \
    __DECLARE_GETTERS_V2_IMPL(STRUCT_NAME, __VA_ARGS__,)

#define __DECLARE_GETTERS_V2_IMPL(STRUCT_NAME, field, ...) \
    inline GET_FIELD_TYPE_V2(field) get_##GET_FIELD_NAME_V2(field)() const { \
        return GET_FIELD_NAME_V2(field).get(); \
    } \
    inline void set_##GET_FIELD_NAME_V2(field)(const GET_FIELD_TYPE_V2(field)& value) { \
        GET_FIELD_NAME_V2(field).set(value); \
    } \
    IF_ELSE(EMPTY_ARG(__VA_ARGS__),, __DECLARE_GETTERS_V2_IMPL(STRUCT_NAME, __VA_ARGS__,))

// Type traits for supported types
template<typename T>
struct is_auto_protected_supported : std::false_type {};

// Supported integer types
template<>
struct is_auto_protected_supported<int32_t> : std::true_type {};

template<>
struct is_auto_protected_supported<int64_t> : std::true_type {};

template<>
struct is_auto_protected_supported<uint32_t> : std::true_type {};

template<>
struct is_auto_protected_supported<uint64_t> : std::true_type {};

// Supported float type
template<>
struct is_auto_protected_supported<float> : std::true_type {};

// Static assertion helper
#define STATIC_ASSERT_AUTO_PROTECTED_SUPPORTED(TYPE) \
    static_assert( \
        Maxion::is_auto_protected_supported<TYPE>::value, \
        "AutoProtected<T> only supports int32_t, int64_t, uint32_t, uint64_t, and float types" \
    )

} // namespace Maxion

#endif // MAXION_AUTO_PROTECTED_H