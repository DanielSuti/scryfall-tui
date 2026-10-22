//! Command-line interface of the `scry` binary.

use clap::Parser;

/// Arguments accepted by `scry`.
#[derive(Debug, Parser)]
#[command(name = "scry", version, about)]
pub struct Cli {}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn version_flag_prints_package_version() {
        let err = Cli::try_parse_from(["scry", "--version"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayVersion);
        assert!(err.to_string().contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn unknown_flag_is_rejected() {
        let err = Cli::try_parse_from(["scry", "--bogus"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }
}
