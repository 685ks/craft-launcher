//! Windows only: embed the app icon and version info (VERSIONINFO) into `arthavan-projects.exe`.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `ARTHAVAN_PROJECTS_REQUIRE_WINRES=1` (set
//! by the release packaging) turns it into an error.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/arthavan-projects.ico");
    println!("cargo:rerun-if-env-changed=ARTHAVAN_PROJECTS_REQUIRE_WINRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/app-icon/arthavan-projects.ico")
        .set("ProductName", "Arthavan Projects")
        .set("FileDescription", "Arthavan Projects")
        .set("CompanyName", "Arthavan Projects")
        .set("LegalCopyright", "Copyright (c) the Arthavan Projects contributors and the authors of storytold/craft-launcher. MIT OR Apache-2.0.")
        .set("OriginalFilename", "arthavan-projects.exe")
        .set("InternalName", "arthavan-projects");
    if let Err(e) = res.compile() {
        if std::env::var_os("ARTHAVAN_PROJECTS_REQUIRE_WINRES").is_some() {
            println!("cargo:warning=embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=arthavan-projects.exe built without icon/version resources: {e}");
    }
}
