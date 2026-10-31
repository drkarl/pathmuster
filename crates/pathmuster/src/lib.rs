//! Check whether a project can move to another computer, operating system or
//! render node without losing the files it depends on.
//!
//! This crate is the scaffolding for the approved design in
//! `docs/architecture.md`. No audit functionality is implemented yet: the only
//! public item is [`version`].

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)
)]

/// Returns the version of this crate, as declared in `Cargo.toml`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn version_has_three_numeric_components() {
        let version = version();
        let components: Vec<&str> = version.split('.').collect();
        assert_eq!(
            components.len(),
            3,
            "expected MAJOR.MINOR.PATCH, got `{version}`"
        );
        for component in components {
            assert!(
                component.parse::<u32>().is_ok(),
                "non-numeric component in `{version}`"
            );
        }
    }
}
