
set -x
redo-ifchange ../lib/libvosk.so ../models/vosk-small.zip

# TODO: This is a hack
export RUSTFLAGS="-L$(pwd)/../lib/"

EXE=$(cargo build --bin "$2" \
	--message-format=json \
	| jq -r 'select(.reason == "compiler-artifact") | select(.executable != null) | .executable')
set -ue
cp "$EXE" "$3"

# Cargo caches pretty okay, so we always rebuild,
# and then use redo-stamp.
redo-always
sha256sum "$3" | redo-stamp


