# set windows-shell := ["C:\\Program Files\\Git\\bin\\sh.exe","-c"]
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]


# list commands also default
list:
    @just --list

sqlx:
    cd sqlx_storage; cargo sqlx database reset -D sqlite:///$PWD/sqlx.sqlite -y
    cd sqlx_storage; cargo sqlx prepare -D sqlite:///$PWD/sqlx.sqlite -- -p openmls_sqlx_storage
    cd sqlx_storage; cargo sqlx database drop -D sqlite:///$PWD/sqlx.sqlite -y
    cargo check -p openmls_sqlx_storage
    cargo check -p svalin_client_store; echo "ignore error"
    cd store_client; cargo sqlx database reset -D sqlite:///$PWD/sqlx.sqlite -y
    cd store_client; cargo sqlx prepare -D sqlite:///$PWD/sqlx.sqlite -- -p svalin_client_store
    cd store_client; cargo sqlx database drop -D sqlite:///$PWD/sqlx.sqlite -y
    cargo check -p svalin_server_store; echo "ignore error"
    cd store_server; cargo sqlx database reset -D sqlite:///$PWD/sqlx.sqlite -y
    cd store_server; cargo sqlx prepare -D sqlite:///$PWD/sqlx.sqlite -- -p svalin_server_store
    cd store_server; cargo sqlx database drop -D sqlite:///$PWD/sqlx.sqlite -y

test $RUST_LOG="svalin=debug":
    cargo test -p svalin_pki -p svalin_rpc -p svalin -- --nocapture

integration $RUST_LOG="svalin=debug":
    cargo test -p svalin test::integration -- --nocapture

server $RUST_LOG="debug":
    cargo run -p svalin server 0.0.0.0:1234

reset:
    rm -r /var/lib/svalin/*

reset-win:
    Remove-Item -Path C:\ProgramData\svalin\* -Recurse -Force

agent $RUST_LOG="debug":
    cargo run -p svalin agent

agent_init:
    cargo run -p svalin agent init localhost:1234

gui $RUST_LOG="svalin=debug":
    cargo run -p svalin_iced

build-debian: build-cross
    cargo deb --package svalin --target x86_64-unknown-linux-gnu --no-build


build-cross:
    cross build --release
