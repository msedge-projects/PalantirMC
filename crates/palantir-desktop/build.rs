//! The Windows file properties: the icon Explorer draws and the version strings.
//!
//! Two things a person can see about the executable without opening it, and
//! neither is free:
//!
//! * **The icon.** Without a resource, Explorer, the taskbar, the Alt-Tab list
//!   and the file's own properties sheet all show the generic application icon,
//!   however carefully the window draws its own. `assets/brand/app.ico` is
//!   written by `tools/gen_brand.py` from the brand art at the seven sizes
//!   Windows asks for.
//! * **The version.** The exe carries `FileVersion`/`ProductVersion` so a user
//!   reporting a bug can right-click the file and say which build they have, and
//!   the installer can read the same numbers instead of keeping a second copy.
//!
//! **The version comes from `Cargo.toml`**, through `CARGO_PKG_VERSION`, and that
//! is the point of doing it here: `brand::version()` is the same environment
//! variable read at compile time, so the number on disk and the number the
//! launcher would print cannot disagree. The product name is the one literal this
//! file carries, and `brand`'s tests hold it to `APP_NAME` -- a resource that
//! called the product something else would be exactly the drift the comment on
//! that constant warns about.
//!
//! Nothing here runs off Windows: the resource compiler is Microsoft's, the
//! script is MSVC's, and a Linux build has no file properties to write. The
//! build script still runs (Cargo always runs it) and prints its rerun triggers,
//! so an edit to the icon relinks the exe.

/// The product name, as the file's properties show it.
///
/// `palantir_core::PRODUCT_NAME` cannot be read from here without making the
/// core crate a build-dependency of this one, so the literal is repeated and
/// `brand`'s tests hold the two together (`the_resource_name_is_the_product_name`).
const PRODUCT: &str = "PalantirMC";

/// What the file's properties call the executable.
const ORIGINAL_FILENAME: &str = "PalantirMC.exe";

/// The one-line description shown beside the name in the properties sheet.
const DESCRIPTION: &str = "Minecraft launcher";

/// Who owns the copyright, from `LICENSE` and the workspace's `authors`.
const COPYRIGHT: &str = "Copyright (c) 2026 Palantir Studios. All rights reserved.";

fn main() {
    println!("cargo:rerun-if-changed=assets/brand/app.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    resources();
}

/// Compile the icon and the version block into the binary.
///
/// A build that cannot write these is a build whose exe would ship without an
/// icon or a version, so it fails here rather than being noticed in a release;
/// the message is the resource compiler's own, which names what it could not
/// find.
#[cfg(windows)]
fn resources() {
    let version = env!("CARGO_PKG_VERSION");
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/brand/app.ico");
    resource.set("ProductName", PRODUCT);
    resource.set("FileDescription", DESCRIPTION);
    resource.set("OriginalFilename", ORIGINAL_FILENAME);
    resource.set("ProductVersion", version);
    resource.set("FileVersion", version);
    resource.set("LegalCopyright", COPYRIGHT);
    resource.set("CompanyName", "Palantir Studios");
    if let Err(error) = resource.compile() {
        panic!("the Windows resources could not be written: {error}");
    }
}
