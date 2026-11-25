//! Windows app icon: embeds `assets/icon/server-status-display.ico` as icon
//! resource 1, which GPUI loads for the window and taskbar icon.

const ICON_RESOURCE_SCRIPT: &str = "assets/icon/server-status-display.rc";
const ICON_FILE: &str = "assets/icon/server-status-display.ico";

fn main() {
    println!("cargo:rerun-if-changed={ICON_RESOURCE_SCRIPT}");
    println!("cargo:rerun-if-changed={ICON_FILE}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile(ICON_RESOURCE_SCRIPT, embed_resource::NONE)
            .manifest_optional()
            .expect("compile the Windows app icon resource");
    }
}
