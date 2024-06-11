

redo-ifchange ../lib/libvosk.so

# TODO: This is a hack
export RUSTFLAGS="-L$(pwd)/../lib/"

cargo build --bins
# TODO: Cargo, WTF?
cp ./target/debug/"$2" "$3"

redo-ifchange Cargo.toml $(find src/)

