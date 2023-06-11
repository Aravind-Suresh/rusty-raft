rm state/*
cargo run --bin main -- 1 &
cargo run --bin main -- 2 &
cargo run --bin main -- 3 &
wait
