# Incentives Contracts 

## Overview

This repo contains the following contracts - 

- **Airdrop Contract** : Used for MARS tokens airdrop claim during the intital protocol launch.

- **Lockdrop Contract** : Allows users to lock their UST for selected duration against which they get MARS token rewards pro-rata to their wighted share along with xMars tokens which are accrued per block. Upon expiration of the lockup, users can withdraw their deposits as interest bearing maUST tokens, redeemable against UST via the Red Bank.

- **LP Staking Contract** : Facilitates MARS-UST LP Token staking and reward distribution.


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


### Test

Start LocalTerra:

```bash
cd /path/to/LocalTerra
git checkout main  # main branch for columbus-5 envrionment
git pull
docker-compose up
```

Run test scripts: inside `scripts` folder,

```bash
cd scripts

node  --experimental-json-modules  --loader ts-node/esm airdrop.spec.ts
node --loader ts-node/esm staking.spec.ts
node --loader ts-node/esm lockdrop.spec.ts
```














