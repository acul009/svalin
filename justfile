# set windows-shell := ["C:\\Program Files\\Git\\bin\\sh.exe","-c"]
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]


# list commands also default
list:
    @just --list

# Setup the database
setup: create-db migrate

# Create the database
create-db:
    cargo sqlx database create

# Run database migrations
migrate:
    cargo sqlx migrate run --source svalin/migrations/server

# remove database
clean:
    cargo sqlx database drop -y

# Restart the database by recreating it
restart: clean setup

test $RUST_LOG="1":
    RUST_LOG=debug cargo test -p svalin_pki -p svalin_rpc -p svalin -- --nocapture

build-debian: build-cross
    cargo deb --package svalin --target x86_64-unknown-linux-gnu --no-build


build-cross:
    cross build --release
