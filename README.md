# rusty-raft
A Rust implementation of the [Raft](https://raft.github.io/raft.pdf) algorithm

## Run
Dependencies and builds are handled using Cargo.

To start raft nodes, use the example `run.sh` script. This starts them as 3 separate processes hosting servers on 3 different ports - however they can run on different nodes as well.

So, start these in a separate process `./run.sh &`.

Then to publish messages, use the example `./client.sh` script.

The algorithm run can be verified by looking at the persisted state in `state/` directory.
