# Astroport Periphery 

This repo contains the contract which facilitates ASTRO tokens airdrop, lockdrop and LP Bootstrapping via auction during the intital protocol launch. 


## Development

### Dependencies

- Rust v1.44.1+
- `wasm32-unknown-unknown` target
- Docker
- [LocalTerra](https://github.com/terra-project/LocalTerra)
- Node.js v16


### Envrionment Setup

1. Install `rustup` via https://rustup.rs/

2. Add `wasm32-unknown-unknown` target

```sh
rustup default stable
rustup target add wasm32-unknown-unknown
```


3. Install Node libraries required for testing:

```bash
cd scripts
npm install
```


### Compile

Make sure the current working directory is set to the root directory of this repository, then

```bash
cargo build
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.11.3
```


### Build schema and run unit-tests
```
cd contracts/airdrop
cargo schema
cargo test
```


### Integration Tests

Start LocalTerra:

```bash
cd /path/to/LocalTerra
git checkout main  
git pull
docker-compose up
```

Run test scripts: inside `scripts` folder,

```bash
cd scripts
node  --experimental-json-modules  --loader ts-node/esm airdrop.spec.ts
```



## License

TBD























