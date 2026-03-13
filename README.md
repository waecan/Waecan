# Wæcnan — $WAEC Protocol

> Built from zero. No pre-mine. No compromise.

## Parameters

| Parameter        | Value                              |
|------------------|------------------------------------|
| Ticker           | $WAEC                              |
| Consensus        | Proof of Work — RandomX (CPU-only) |
| Block time       | 2 minutes                          |
| Block reward     | 50 WAEC (genesis)                  |
| Halving interval | Every 525,600 blocks (~2 years)    |
| Max supply       | ~105,000,000 WAEC                  |
| Fee              | 0.001 WAEC — burned permanently    |
| Privacy          | Ring Signatures + Stealth Addresses|
| Pre-mine         | Zero                               |

## Dev Environment

Open in GitHub Codespaces — no local installation needed.
Click: Code → Codespaces → Create codespace on main

## Build

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```
