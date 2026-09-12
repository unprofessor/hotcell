//! Derives hotcell's version from `git describe` via semvertag and exposes it
//! to the crate as `SEMVERTAG_VERSION`. `Cargo.toml`'s version is the hint for
//! the next release. Without usable git history (a tarball, a shallow clone)
//! the version falls back to `0.0.0-unknown` rather than failing the build.

use semvertag_shell::Version;

const FALLBACK: &str = "0.0.0-unknown";

fn main() {
    // Best-effort rebuild triggers; changes that only touch packed refs are missed.
    for p in [".git/HEAD", ".git/index", ".git/refs/tags"] {
        println!("cargo:rerun-if-changed={p}");
    }

    let hint = Version::parse(env!("CARGO_PKG_VERSION")).ok();
    let version = match semvertag_shell::describe_with_hint(hint.as_ref()) {
        Ok(v) => v.to_string(),
        Err(e) => {
            println!("cargo:warning=semvertag: {e}; version falls back to {FALLBACK}");
            FALLBACK.to_string()
        }
    };
    println!("cargo:rustc-env=SEMVERTAG_VERSION={version}");
}
