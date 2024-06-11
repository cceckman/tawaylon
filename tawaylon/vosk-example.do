

# TODO: This is a hack
export RUSTFLAGS="-L$(pwd)/../venv/lib/python3.11/site-packages/vosk"

cargo build --bins
# TODO: Cargo, WTF?
cp ~/target/debug/vosk-example "$3"

redo-ifchange Cargo.toml $(find src/)

