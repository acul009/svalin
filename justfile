
build-debian: build-cross
    cargo deb --package svalin --target x86_64-unknown-linux-gnu --no-build


build-cross:
    cross build --release