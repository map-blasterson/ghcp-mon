# Building & packaging

`ghcp-mon` ships as one binary per OS with the Vite SPA embedded via
[`rust-embed`].

## TL;DR

```bash
podman build --target dist -o type=local,dest=./dist -f Containerfile .

# -> dist/ghcp-mon         (Linux x86_64, glibc, ELF)
# -> dist/ghcp-mon.exe     (Windows x86_64, MSVC ABI, PE32+)
# -> dist/ghcp-mon-darwin  (macOS universal2, x86_64 + arm64, Mach-O)
```

`docker` works in place of `podman`.

## Containerfile stages

```
web      node:22-alpine                     -> /web/dist           (Vite SPA bundle)
linux    rust:1.86-bookworm                 -> ghcp-mon            (x86_64-unknown-linux-gnu)
windows  messense/cargo-xwin                -> ghcp-mon.exe        (x86_64-pc-windows-msvc)
darwin   ghcr.io/rust-cross/cargo-zigbuild  -> ghcp-mon-darwin     (universal2-apple-darwin)
dist     scratch                            -> /ghcp-mon{,.exe,-darwin}
```

Build a single target:

```bash
podman build --target linux   -t ghcp-mon:linux   -f Containerfile .
podman build --target windows -t ghcp-mon:windows -f Containerfile .
podman build --target darwin  -t ghcp-mon:darwin  -f Containerfile .
```

## Embedded SPA

- `Cargo.toml`: `rust-embed = "8"`, `mime_guess = "2"`.
- `build.rs`: writes a stub `web/dist/index.html` when missing so plain
  `cargo build` works without npm. Real builds overwrite this with the
  Vite output.
- `src/static_assets.rs`: derives `RustEmbed` over `web/dist/`; falls back
  to `index.html` for unknown paths.
- `src/server.rs`: `.fallback(static_handler)` is attached after `/api/*`
  and `/ws/*` routes.

In a release build the SPA is baked into the binary. In `cargo build`
(debug), `rust-embed` reads from disk.

## Local dev (no container)

```bash
# Backend (terminal 1)
cargo run -- serve

# Frontend (terminal 2) — Vite dev server with HMR
cd web && npm install && npm run dev
```

Open <http://127.0.0.1:5173>.

To exercise the embedded SPA path locally:

```bash
cd web && npm ci && npm run build       # produces web/dist/
cd .. && cargo build --release          # bakes web/dist/ into the binary
./target/release/ghcp-mon serve         # open http://127.0.0.1:4319
```

## Verifying a packaged build

```bash
podman build --target dist -o type=local,dest=./dist -f Containerfile .

./dist/ghcp-mon serve \
  --db /tmp/ghcp-test.db \
  --otlp-addr 127.0.0.1:14318 \
  --api-addr  127.0.0.1:14319 &

curl -sS http://127.0.0.1:14319/api/healthz                    # {"ok":true}
curl -sS http://127.0.0.1:14319/ | head -1                     # <!doctype html>
curl -sS http://127.0.0.1:14319/some/spa/route | head -1       # <!doctype html>  (SPA fallback)
```

Wine smoke-test the Windows binary:

```bash
podman run --rm -it -v "$PWD:/io" -w /io docker.io/messense/cargo-xwin \
  cargo xwin test --release --target x86_64-pc-windows-msvc
```

## Distribution

The artifacts are plain executables.

- Tarball / zip: `tar czf ghcp-mon-<ver>-linux-x86_64.tar.gz -C dist ghcp-mon`,
  `zip -j ghcp-mon-<ver>-windows-x86_64.zip dist/ghcp-mon.exe`.
- Upload to a file share / blob store / internal release page.
- For a `curl | sh` installer, pick the artifact by `uname -s`/`uname -m`.

