set -x
cargo build --release
cp ../target/release/heng-sandbox /usr/local/bin/heng-sandbox
