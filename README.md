# Astroport Bootstrapping

This repo contains contracts which facilitate the ASTRO token airdrop, lockdrop and ASTRO-UST LP bootstrapping.

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

3. Install required Node libraries:

```bash
cd scripts
npm install
```

3. Deploy on testnet:

```bash
export WALLET="<mnemonic seed>"
export LCD_CLIENT_URL="https://bombay-lcd.terra.dev"
export CHAIN_ID="bombay-12"
node --experimental-json-modules --loader ts-node/esm deploy_script.ts
```

### Compile

Make sure the current working directory is set to the root directory of this repository, then run:

```bash
cargo build
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.11.3
```

## Bug Bounty

The contracts in this repo are included in a [bug bounty program](https://www.immunefi.com/bounty/astroport).

## License

[GNU General Public License v3.0](https://github.com/astroport-fi/astroport-periphery/blob/main/LICENSE)
