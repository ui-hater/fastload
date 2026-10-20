use embed_manifest::manifest::{ActiveCodePage, DpiAwareness, Setting, SupportedOS::*};
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var("TARGET").unwrap_or_default().contains("windows") {
        let manifest = new_manifest("fastload")
            .supported_os(Windows7..=Windows10)
            .active_code_page(ActiveCodePage::Utf8)
            .dpi_awareness(DpiAwareness::PerMonitorV2)
            .long_path_aware(Setting::Enabled);
        embed_manifest(manifest).expect("failed to embed manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
