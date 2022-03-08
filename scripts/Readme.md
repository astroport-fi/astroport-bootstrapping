# Astroport Launch: Deployment Guide

- <h2> Astroport Lockdrop + LBA Launch Guide </h2>
  <br>

  - **[ TESTNET ONLY ] Initialize ASTRO and 3rd party tokens with their Terraswap pools and mint LP tokens for Lockdrop**

    Requirements - Need to set Terraswap factory address and LUNA-UST Terraswap pair address in `/artifacts/<Chain_ID>` before executing this script.
    Command to execute on terminal -

    ```
    node --loader ts-node/esm create_lockdrop_env.ts
    ```

    Deployed token addresses, their Terraswap LP pairs and LP pool tokens will be stored in `/artifacts/<Chain_ID>`.

    Optionally, you can send testers some LP tokens so they can execute scripts via command line. You can add the tester terra addresses in the `ADDRESSES` array.

    ```
    node --loader ts-node/esm transfer_tokens.ts
    ```

  - **Initialize Lockdrop, Airdrop, Auction Contracts, send them ASTRO tokens for incentives, and initialize LP pools on Lockdrop for deposits.**
    Requirements - Need to set token addresses in `/artifacts/<Chain_ID>` before executing this script (for testnet, this will be done by the previous script mentioned above).
    Command to execute on terminal -

    ```
    node --experimental-json-modules --loader ts-node/esm deploy_periphery_contracts.ts
    ```

    Function execution flow for this script -

    - Deploys Lockdrop
    - Deploys Airdrop
    - Deploys Auction
    - Lockdrop::UpdateConfig : Set ASTRO, Auction addresses
    - Airdrop::UpdateConfig : Set Auction address
    - ASTRO::Send::Lockdrop::IncreaseAstroIncentives : Set Lockdrop incentives
    - ASTRO::Transfer : Set Airdrop incentives
    - ASTRO::Send::Auction::IncreaseAstroIncentives : Set Auction incentives
    - Lockdrop::InitPool : Initialize LUNA-UST pool in Lockdrop
    - Lockdrop::InitPool : Initialize LUNA-BLUNA pool in Lockdrop
    - Lockdrop::InitPool : Initialize ANC-UST pool in Lockdrop
    - Lockdrop::InitPool : Initialize MIR-UST pool in Lockdrop
    - Lockdrop::InitPool : Initialize ORION-UST pool in Lockdrop
    - Lockdrop::InitPool : Initialize STT-UST pool in Lockdrop
    - Lockdrop::InitPool : Initialize VKR-UST pool in Lockdrop
    - Lockdrop::InitPool : Initialize MINE-UST pool in Lockdrop
    - Lockdrop::InitPool : Initialize PSI-UST pool in Lockdrop
    - Lockdrop::InitPool : Initialize APOLLO-UST pool in Lockdrop

      In case any error occurs while executing the above script, it can be re-executed and it will continue from where it previously faulted.

      Addresses of the deployed periphery contracts and certain variables describing which all functions have been called successfully can be found in `/artifacts/<Chain_ID>.json`

  - **Initialize pools on Astroport to which liquidity is to be migrated**
    Requirements - Need to set Astroport factory address before executing this script
    Command to execute on terminal, -

    ```
    node --loader ts-node/esm create_astroport_pools.ts
    ```

    In case any error occurs while executing the above script, it can be re-executed and it will continue from where it previously left.

    Addresses of the newly initialized pools on Astroport can be found in `/artifacts/<Chain_ID>.json`.

  - **Migrating Liquidity to Astroport pools**
    Requirements - Need to set Astroport pool addresses before executing this script. Will be done by the above script automatically.
    Command to execute on terminal -

    ```
    node --loader ts-node/esm migrate_liquidity_to_astroport.ts
    ```

    In case any error occurs while executing this script, it can be re-executed and it will continue from where it previously left.
    <br>

  - **[ TESTNET ONLY ] Deploy 3rd party staking contracts for eligible tokens getting dual incentives**
    <br>

  - **Deploy proxy contracts for Astroport LP tokens eligible for dual incentives**
    <br>

  - **Initialize ASTRO rewards with the Generator for eligible Astroport LP Tokens**
    <br>

  - **Stake Astroport LP tokens in the Generator contract**
    <br>

- Set environment variables

  For bombay -

  ```bash
  export WALLET="<mnemonic seed>"
  export LCD_CLIENT_URL="https://bombay-lcd.terra.dev"
  export CHAIN_ID="bombay-12"
  ```

  For mainnet -

  ```bash
  export WALLET="<mnemonic seed>"
  export LCD_CLIENT_URL="https://lcd.terra.dev"
  export CHAIN_ID="columbus-5"
  ```
