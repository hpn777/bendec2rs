//! Portable source emitter compatible with Bendec 0.52.0's Rust generator.
//!
//! Callback-valued JavaScript options cannot be executed by this library.
//! `typeMapping` values may instead be strings with `{{ size }}` substitutions;
//! `forEachType` may be a string with `{{ generated }}` and `{{ name }}`.
//! Enum conversion templates accept `{{ underlying }}` and `{{ name }}`.
//! Templates are substitutions, not JavaScript expressions. Unsupported options,
//! expressions, and callback representations return errors rather than being ignored.
//!
//! Compatibility is source-level, including upstream formatting quirks; it does
//! not guarantee that every emitted schema is valid Rust. Non-finite numeric
//! values and radix conversions outside the portable `u128` range are rejected.
//! Identifier word splitting covers ASCII and Latin deburring, but is not a full
//! implementation of Lodash's Unicode/emoji regular-expression word grammar.

mod emitter;
mod emitter_union;

/// Generate Rust source without file I/O or project-specific dependencies.
pub fn generate_string(
    types: &[serde_json::Value],
    options: &serde_json::Value,
) -> Result<String, String> {
    emitter::generate(types, options, &options["meta"], &mut (), false)
}

/// Native extension points. Contexts have numeric kinds and normalized enum
/// variants; metadata is supplied separately and is never interpreted as options.
pub trait NativeHooks {
    fn for_each_type(
        &mut self,
        generated: String,
        _context: &serde_json::Value,
        _metadata: &serde_json::Value,
    ) -> Result<String, String> {
        Ok(generated)
    }
    fn union_discriminant(
        &mut self,
        _context: &serde_json::Value,
        _member: &str,
        value: f64,
    ) -> Result<f64, String> {
        Ok(value)
    }
}
impl NativeHooks for () {}

/// Generate with native callbacks, keeping opaque callback metadata out of the
/// validated emitter options. The existing `generate_string` API is unchanged.
pub fn generate_string_with_hooks(
    types: &[serde_json::Value],
    options: &serde_json::Value,
    metadata: &serde_json::Value,
    hooks: &mut dyn NativeHooks,
) -> Result<String, String> {
    emitter::generate(types, options, metadata, hooks, true)
}
