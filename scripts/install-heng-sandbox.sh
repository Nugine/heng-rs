set -e
set -x
cargo build -p heng-sandbox --release
cp ./target/release/heng-sandbox /usr/local/bin/heng-sandbox
