use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields};

/// Automatically wraps all struct fields in `Protected<T>` for anti-cheat protection.
///
/// This attribute macro transforms a struct with regular fields into a struct
/// where all fields are wrapped in `Protected<T>`, providing automatic protection
/// against memory tampering by cheat engines.
///
/// # Example
///
/// ```rust,ignore
/// use maxion_core::Protected;
/// use maxion_macros::auto_protected;
///
/// #[auto_protected]
/// struct Player {
///     health: i32,
///     ammo: i32,
///     score: i32,
/// }
///
/// // The macro generates this struct internally:
/// struct Player {
///     health: Protected<i32>,
///     ammo: Protected<i32>,
///     score: Protected<i32>,
/// }
///
/// // And generates a constructor:
/// let player = Player::new(100, 30, 0);
///
/// // Access fields through the Protected<T> API:
/// let current_health = player.health.get();
/// player.health.set(75);
/// ```
///
/// # How It Works
///
/// 1. **Field Wrapping**: All struct fields are automatically wrapped in `Protected<T>`
/// 2. **Constructor Generation**: A `new()` constructor is generated to initialize all fields
/// 3. **Automatic Protection**: All protected values use the trap/encryption mechanism
/// 4. **Cheat Detection**: Memory tampering is automatically detected on access
///
/// # Generated API
///
/// The macro automatically generates:
/// - `new(fields...)` - Constructor that initializes all protected fields
/// - All fields are of type `Protected<T>` instead of `T`
///
/// # Limitations
///
/// - Only works on structs with named fields (not tuple structs)
/// - All types must support `Protected<T>` (currently: i32, i64, u32, u64, f32, tuples)
/// - Cannot be used on structs that already have manual `Protected<T>` fields
///
/// # Security Benefits
///
/// Using `#[auto_protected]` provides:
/// - ✅ Automatic memory protection for all fields
/// - ✅ Detection of value scanning and modification
/// - ✅ Protection against value freezing attacks
/// - ✅ Minimal code changes - just add the attribute
///
/// # Performance Considerations
///
/// Protected values have ~78x overhead compared to regular values (see docs/06_security/006_trap.md).
/// Only use this for critical game state like health, ammo, score, currency, etc.
#[proc_macro_attribute]
pub fn auto_protected(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    // Extract the struct fields
    let fields = match &input.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input,
                "#[auto_protected] only supports structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate constructor parameters (original types)
    let constructor_params = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        quote! { #field_name: #field_type }
    });

    // Generate constructor body (wrap in Protected)
    let constructor_body = fields.iter().map(|field| {
        let field_name = &field.ident;
        quote! { #field_name: ::maxion_core::Protected::new(#field_name) }
    });

    // Generate field declarations (wrapped in Protected)
    let field_decls = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        quote! { pub #field_name: ::maxion_core::Protected<#field_type> }
    });

    // Generate the expanded code
    let expanded = quote! {
        #[derive(Debug)]
        pub struct #struct_name {
            #(#field_decls),*
        }

        #[allow(clippy::too_many_arguments)]
        impl #struct_name {
            /// Create a new instance with protected fields.
            ///
            /// All fields are automatically wrapped in `Protected<T>` for anti-cheat protection.
            pub fn new(#(#constructor_params),*) -> Self {
                Self {
                    #(#constructor_body),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
