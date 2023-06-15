rm state/*
cargo run --bin main -- 1 &> logs/1.out &
cargo run --bin main -- 2 &> logs/2.out &
cargo run --bin main -- 3 &> logs/3.out &
wait
