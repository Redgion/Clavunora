# Building the GPLv3 Rust Linux release

Use the `Clavunora-Rust-VERSION-source.tar.gz` supplied next to the Linux binary
archive in the same GitHub release. It contains the application, resources,
build/test scripts, Cargo.lock, and vendored sources of all locked Cargo crates.
SOURCE_FILES.json identifies the project files used to assemble that archive.
There are no user profiles, private keys, developer settings or model weights.

The reference local build uses Rust 1.99.1 on Debian 12 (glibc 2.36). Rust and
ordinary system build tools are not included. Install Rust with rustup and the
following Debian/Ubuntu prerequisites (package names differ on other systems):

    sudo apt-get install build-essential pkg-config libasound2-dev libxkbcommon-dev
    rustup toolchain install 1.99.1
    cd rust
    cargo +1.99.1 build --locked --offline --release -p clavunora-rust

The source archive configures Cargo to use `../vendor`, without network access.
When building a Git checkout instead, fetch dependencies first with
`cargo fetch --locked`; the vendor directory is supplied in the release archive.

Run and install the result with its external catalog:

    mkdir -p ../local-release/resources
    cp target/release/clavunora-rust ../local-release/
    cp -R ../pianoflow/resources/catalog ../local-release/resources/
    cp ../LICENSE ../COPYRIGHT ../THIRD_PARTY_NOTICES.md ../AI_USAGE.md ../local-release/
    ../local-release/clavunora-rust --self-test
    ../local-release/clavunora-rust

If CARGO_TARGET_DIR is set, use that target directory instead of `target`.
The application can also find the catalog in its source checkout during tests.
For distribution always copy resources/catalog beside the executable: score
contents are deliberately not compiled into the GPL binary. Their independent
licenses remain in the provenance/source files.

Verification:

    cargo +1.99.1 fmt --all -- --check
    cargo +1.99.1 test --locked --offline --workspace
    cargo +1.99.1 clippy --locked --offline --workspace --all-targets -- -D warnings

For real GUI tests install X11/Wayland/OpenGL libraries; on a headless Linux host,
use Xvfb plus software rendering. ALSA and graphics libraries are system
prerequisites. Optional FluidSynth/SoundFonts are not required or bundled.

The reference repository also contains a Podman/Debian-12 compatibility builder
in scripts/build_rust_linux_compat.sh and packaging/rust-linux/Containerfile.compat.
It expects the locally installed toolchain/cache at build/rust-toolchain; normal
source builds above do not depend on that developer-specific cache.

Redistribution duties are in LICENSE (particularly section 6). Publish the
matching source archive next to each binary, with equivalent access and no
additional source download charge; preserve notices and the source availability
for the required duration. A link only to a changing development branch does not
identify the corresponding source of a particular binary. Third-party resources
and dependency source files retain their own licenses.

Native Windows/macOS builds use the same Cargo workspace and toolchain. On Windows
install the MSVC C++ build tools and Windows SDK; build with
RUSTFLAGS="-C target-feature=+crt-static". On macOS use Xcode Command Line Tools and
MACOSX_DEPLOYMENT_TARGET=11.0. Use package_rust_desktop.py on the target OS; it
creates the application bundle/portable ZIP with external resources and notices.
See DESKTOP_RELEASE.md and the included rust-desktop.yml for exact native checks.
Certificates and notarization credentials are optional owner-controlled release
inputs, not source build requirements.
