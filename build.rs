// Ensures `web/dist/` exists at compile time so the `rust-embed` derive in
// `src/static_assets.rs` always has a folder to point at, even on a fresh
// clone where `npm run build` has not run yet. Real frontend builds (CI,
// containers) overwrite this stub by producing a real `web/dist/index.html`.
use std::path::Path;

fn main() {
    let dist = Path::new("web/dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).expect("create web/dist");
    }
    let index = dist.join("index.html");
    if !index.exists() {
        std::fs::write(
            &index,
            "<!doctype html><title>ghcp-mon</title>\
             <h1>ghcp-mon</h1>\
             <p>Frontend not built. Run <code>cd web &amp;&amp; npm ci &amp;&amp; npm run build</code> \
             and rebuild, or use the container build.</p>",
        )
        .expect("write stub web/dist/index.html");
    }
    println!("cargo:rerun-if-changed=web/dist");
    println!("cargo:rerun-if-changed=build.rs");
}
