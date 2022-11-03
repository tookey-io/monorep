# Tookey monorepository

Tookey is assets and access management protocol for web3. We build secure environment to interact with crypto without risk of disclose the private key.


## Compilation

`cargo build --release`

## Running

### Keygeneration for 3 parties and threshold is 1 (at lease two key to sign)

1. Run `manager` in the terminal: `cargo run --release --bin manager`
2. Open 3 terminals and run: 
    1. `cargo run --release --bin keygen -- -t 1 -n 3 -i 1 --output key1.json`
    1. `cargo run --release --bin keygen -- -t 1 -n 3 -i 2 --output key2.json`
    1. `cargo run --release --bin keygen -- -t 1 -n 3 -i 3 --output key3.json`

### Sign a message 
1. Ensure you passed keygeneration
1. Run `manager` in the terminal: `cargo run --release --bin manager`
2. Open 2 terminals and run: 
    1. `cargo run --release --bin sign -- -p 1,2 -h "0xbd621a5652a421f0b853d2a56609bfd26ae965709070708a34f7607f1ce97a60" -l key1.json`
    1. `cargo run --release --bin sign -- -p 1,2 -h "0xbd621a5652a421f0b853d2a56609bfd26ae965709070708a34f7607f1ce97a60" -l key2.json`
