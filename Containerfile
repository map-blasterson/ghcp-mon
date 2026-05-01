# Multi-stage build:
#   web      -> Vite SPA bundle (web/dist)
#   linux    -> static-ish Linux ELF                       (x86_64-unknown-linux-gnu)
#   windows  -> Windows .exe via cargo-xwin (clang-cl)     (x86_64-pc-windows-msvc)
#   darwin   -> macOS universal2 Mach-O via cargo-zigbuild (universal2-apple-darwin)
#   dist     -> scratch stage that just exposes the binaries for `-o type=local`
#
# Build everything and extract artifacts:
#   podman build --target dist -o type=local,dest=./dist .
#
# Build a single target:
#   podman build --target linux   -t ghcp-mon:linux   .
#   podman build --target windows -t ghcp-mon:windows .
#   podman build --target darwin  -t ghcp-mon:darwin  .
#
# Smoke-test the Windows binary under wine (slow, off by default — uncomment in
# the windows stage to enable, or run interactively):
#   podman run --rm -it ghcp-mon:windows cargo xwin test --release \
#       --target x86_64-pc-windows-msvc

ARG RUST_VERSION=1.86
ARG NODE_VERSION=22

# ---- web ---------------------------------------------------------------------
FROM docker.io/library/node:${NODE_VERSION}-alpine AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build
# -> /web/dist

# ---- linux -------------------------------------------------------------------
FROM docker.io/library/rust:${RUST_VERSION}-bookworm AS linux
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY src/ ./src/
COPY migrations/ ./migrations/
COPY tests/ ./tests/
COPY --from=web /web/dist ./web/dist
RUN cargo build --release --locked
# -> /src/target/release/ghcp-mon

# ---- windows -----------------------------------------------------------------
# messense/cargo-xwin ships clang, llvm, rustup, cargo-xwin, and wine, so the
# MSVC sysroot download + a wine smoke-test path are both available without any
# host setup. License acceptance is required by Microsoft.
FROM docker.io/messense/cargo-xwin AS windows
ENV XWIN_ACCEPT_LICENSE=1
WORKDIR /src
RUN rustup target add x86_64-pc-windows-msvc
# Pre-cache the MSVC CRT + Windows SDK in its own layer so source edits don't
# re-download hundreds of MB.
RUN cargo xwin cache xwin --xwin-arch x86_64
COPY Cargo.toml Cargo.lock build.rs ./
COPY src/ ./src/
COPY migrations/ ./migrations/
COPY tests/ ./tests/
COPY --from=web /web/dist ./web/dist
RUN cargo xwin build --release --locked --target x86_64-pc-windows-msvc
# Optional wine smoke test — uncomment to gate on it (slow, can be flaky on
# tests that touch sockets/filesystem under wine):
# RUN cargo xwin test --release --locked --target x86_64-pc-windows-msvc
# -> /src/target/x86_64-pc-windows-msvc/release/ghcp-mon.exe

# ---- darwin ------------------------------------------------------------------
# Uses cargo-zigbuild's official image, which bundles zig + cargo-zigbuild +
# rustup with the apple-darwin targets + a phracker MacOSX SDK at SDKROOT.
# The SDK is required because rustc's libstd unconditionally emits
# `-framework CoreFoundation` on every *-apple-darwin target, and zig itself
# does not ship Apple framework stubs (only libSystem). See ziglang/zig#1349.
# Note: Apple's SDK redistribution is a license gray area; we inherit the
# upstream image's stance on that. Output binaries are unsigned (ad-hoc only
# for arm64 via zig's auto-codesign); Gatekeeper notarization is out of scope.
FROM ghcr.io/rust-cross/cargo-zigbuild:latest AS darwin
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY src/ ./src/
COPY migrations/ ./migrations/
COPY tests/ ./tests/
COPY --from=web /web/dist ./web/dist
RUN cargo zigbuild --release --locked --target universal2-apple-darwin
# -> /src/target/universal2-apple-darwin/release/ghcp-mon

# ---- dist --------------------------------------------------------------------
FROM scratch AS dist
COPY --from=linux   /src/target/release/ghcp-mon                              /ghcp-mon
COPY --from=windows /src/target/x86_64-pc-windows-msvc/release/ghcp-mon.exe   /ghcp-mon.exe
COPY --from=darwin  /src/target/universal2-apple-darwin/release/ghcp-mon      /ghcp-mon-darwin
