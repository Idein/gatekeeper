# Reproduce thread leak by connection reset by server

https://www.notion.so/gatekeeper-38e0b510b36f4f4da0a082ae00baf998

## Usage

Start the gatekeeper with the command below.

```
RUST_LOG=trace cargo run -- --rule tests/thread-leak-by-server-disconnection/example.yml --port=18080
```

In another terminal, run the command below in this directory.

```
node server.js
```

This script starts a server and invokes `curl` to get some content from the server.

It is expected that `curl` does not stop. It is a sign of thread leak.
