import "dotenv/config";
import { Coin } from "@terra-money/terra.js";
import {
  uploadContract,
  instantiateContract,
  executeContract,
  newClient,
  queryContract,
  readArtifact,
  writeArtifact,
  extract_terraswap_pool_info,
  Client,
} from "./helpers/helpers.js";
import { join } from "path";

const ARTIFACTS_PATH = "../artifacts";

// ########### LOCKDROP :: LP TOKENS ELIGIBLE ###########

// - LUNA/UST - dual incentives : ASTRO only
// - LUNA/BLUNA - dual incentives : ASTRO only
// - ANC/UST - dual incentives : ASTRO and ANC        [.wasm available]
// - MIR/UST - dual incentives : ASTRO and MIR        [.wasm available]
// - ORION/UST - dual incentives : ASTRO only         [.wasm available]
// - STT/UST - dual incentives : ASTRO and STT        [.wasm available]
// - VKR/UST - dual incentives : ASTRO and VKR        [.wasm available]
// - MINE/UST - dual incentives : ASTRO and MINE      [.wasm available]
// - PSI/UST - dual incentives : ASTRO and PSI        [.wasm available]
// - APOLLO/UST - dual incentives : ASTRO and APOLLO  [.wasm available]

async function main() {
  const { terra, wallet } = newClient();

  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  if (terra.config.chainID != "bombay-12") {
    console.log(`network must be bombay-12`);
    return;
  }

  const network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  let inital_ust_liquidity_for_each = 1000_000000;
  let inital_token_liquidity_for_each = 1000000_000000;

  // CW20 TOKEN CODE ID
  if (!network.cw20_token_code_id) {
    network.cw20_token_code_id = await uploadContract(
      terra,
      wallet,
      join(ARTIFACTS_PATH, "cw20_token.wasm")
    );
    console.log(`Cw20 Code id = ${network.cw20_token_code_id}`);
    writeArtifact(network, terra.config.chainID);
  }
  /*************************************** DEPLOYMENT :: ASTRO TOKEN ON BOMBAY TESTNET  *****************************************/

  // Deploy ASTRO (dummy) Token
  if (!network.astro_token_address) {
    network.astro_token_address = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "Astroport",
        symbol: "ASTRO",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(10_000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(10_000_000_000_000000),
        },
      },
      "ASTRO Token for testing"
    );
    console.log(
      `ASTRO Token deployed successfully, address : ${network.astro_token_address}`
    );
    writeArtifact(network, terra.config.chainID);
  } else {
    console.log(`ASTRO Token already deployed on bombay-12`);
  }

  // // Mint luna-ust LP Tokens
  await executeContract(
    terra,
    wallet,
    network.luna_ust_terraswap_pool,
    {
      provide_liquidity: {
        assets: [
          {
            info: { native_token: { denom: "uluna" } },
            amount: String(inital_ust_liquidity_for_each / 10),
          },
          {
            info: { native_token: { denom: "uusd" } },
            amount: String(inital_ust_liquidity_for_each),
          },
        ],
        slippage_tolerance: undefined,
      },
    },
    [
      new Coin("uluna", inital_ust_liquidity_for_each / 10),
      new Coin("uusd", inital_ust_liquidity_for_each),
    ]
  );
  console.log(`LUNA-UST LP Tokens successfully minted on bombay-12`);

  // Mint luna-Bluna LP Tokens : Deploy BLuna, create Luna-BLuna Pair on terraswap : Mint LP Tokens
  if (!network.bluna_token_address) {
    // deploy token
    network.bluna_token_address = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "Bonded Luna",
        symbol: "bluna",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(1000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(1000_000_000_000000),
        },
      },
      "BLUNA Token for testing"
    );
    console.log(
      `BLuna Token deployed successfully, address : ${network.bluna_token_address}`
    );
    // Initialize terraswap pool
    let bluna_init_pool_tx = await executeContract(
      terra,
      wallet,
      network.terraswap_factory_address,
      {
        create_pair: {
          asset_infos: [
            { token: { contract_addr: network.bluna_token_address } },
            { native_token: { denom: "uluna" } },
          ],
        },
      }
    );
    let bluna_init_pool_tx_resp =
      extract_terraswap_pool_info(bluna_init_pool_tx);
    network.bluna_luna_terraswap_pool = bluna_init_pool_tx_resp.pool_address;
    network.bluna_luna_terraswap_lp_token_address =
      bluna_init_pool_tx_resp.lp_token_address;
    // Mint LP Tokens : increase_allowance and provide_liquidity
    await executeContract(terra, wallet, network.bluna_token_address, {
      increase_allowance: {
        spender: network.bluna_luna_terraswap_pool,
        amount: String(1000_000_000_000000),
      },
    });
    await delay(300);
    await executeContract(
      terra,
      wallet,
      network.bluna_luna_terraswap_pool,
      {
        provide_liquidity: {
          assets: [
            {
              info: { native_token: { denom: "uluna" } },
              amount: String(inital_ust_liquidity_for_each),
            },
            {
              info: { token: { contract_addr: network.bluna_token_address } },
              amount: String(inital_token_liquidity_for_each),
            },
          ],
          slippage_tolerance: undefined,
        },
      },
      [new Coin("uluna", inital_ust_liquidity_for_each)]
    );
    console.log(`LUNA-BLUNA LP Tokens successfully minted on bombay-12 \n`);
    writeArtifact(network, terra.config.chainID);
  }

  // Mint ANC-UST LP Tokens : Deploy ANC, create ANC-UST Pair on terraswap : Mint LP Tokens
  // Mint ANC-UST LP Tokens : Deploy ANC, create ANC-UST Pair on terraswap : Mint LP Tokens
  // Mint ANC-UST LP Tokens : Deploy ANC, create ANC-UST Pair on terraswap : Mint LP Tokens
  if (!network.anc_token) {
    // deploy token
    network.anc_token = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "Anchor Token",
        symbol: "ANC",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(1000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(1000_000_000_000000),
        },
      },
      "ANC Token for testing"
    );
    console.log(
      `ANC Token deployed successfully, address : ${network.anc_token}`
    );
    // Initialize terraswap pool
    let anc_init_pool_tx = await executeContract(
      terra,
      wallet,
      network.terraswap_factory_address,
      {
        create_pair: {
          asset_infos: [
            { token: { contract_addr: network.anc_token } },
            { native_token: { denom: "uusd" } },
          ],
        },
      }
    );
    let anc_init_pool_tx_resp = extract_terraswap_pool_info(anc_init_pool_tx);
    network.anc_ust_terraswap_pool = anc_init_pool_tx_resp.pool_address;
    network.anc_ust_terraswap_lp_token_address =
      anc_init_pool_tx_resp.lp_token_address;
    // Mint LP Tokens : increase_allowance and provide_liquidity
    await executeContract(terra, wallet, network.anc_token, {
      increase_allowance: {
        spender: network.anc_ust_terraswap_pool,
        amount: String(1000_000_000_000000),
      },
    });
    await delay(300);
    await executeContract(
      terra,
      wallet,
      network.anc_ust_terraswap_pool,
      {
        provide_liquidity: {
          assets: [
            {
              info: { native_token: { denom: "uusd" } },
              amount: String(inital_ust_liquidity_for_each),
            },
            {
              info: { token: { contract_addr: network.anc_token } },
              amount: String(inital_token_liquidity_for_each),
            },
          ],
          slippage_tolerance: undefined,
        },
      },
      [new Coin("uusd", inital_ust_liquidity_for_each)]
    );
    await delay(300);
    console.log(`ANC-UST LP Tokens successfully minted on bombay-12 \n`);
    writeArtifact(network, terra.config.chainID);
  }

  // Mint MIR-UST LP Tokens : Deploy MIR, create MIR-UST Pair on terraswap : Mint LP Tokens
  // Mint MIR-UST LP Tokens : Deploy MIR, create MIR-UST Pair on terraswap : Mint LP Tokens
  // Mint MIR-UST LP Tokens : Deploy MIR, create MIR-UST Pair on terraswap : Mint LP Tokens
  // Mint MIR-UST LP Tokens : Deploy MIR, create MIR-UST Pair on terraswap : Mint LP Tokens
  if (!network.mir_token) {
    // deploy token
    network.mir_token = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "Mirror Token",
        symbol: "MIR",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(1000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(1000_000_000_000000),
        },
      },
      "MIR Token for testing"
    );
    console.log(
      `MIR Token deployed successfully, address : ${network.mir_token}`
    );
    // Initialize terraswap pool
    let mir_init_pool_tx = await executeContract(
      terra,
      wallet,
      network.terraswap_factory_address,
      {
        create_pair: {
          asset_infos: [
            { token: { contract_addr: network.mir_token } },
            { native_token: { denom: "uusd" } },
          ],
        },
      }
    );
    let mir_init_pool_tx_resp = extract_terraswap_pool_info(mir_init_pool_tx);
    network.mir_ust_terraswap_pool = mir_init_pool_tx_resp.pool_address;
    network.mir_ust_terraswap_lp_token_address =
      mir_init_pool_tx_resp.lp_token_address;
    // Mint LP Tokens : increase_allowance and provide_liquidity
    await executeContract(terra, wallet, network.mir_token, {
      increase_allowance: {
        spender: network.mir_ust_terraswap_pool,
        amount: String(1000_000_000_000000),
      },
    });
    await delay(300);
    await executeContract(
      terra,
      wallet,
      network.mir_ust_terraswap_pool,
      {
        provide_liquidity: {
          assets: [
            {
              info: { native_token: { denom: "uusd" } },
              amount: String(inital_ust_liquidity_for_each),
            },
            {
              info: { token: { contract_addr: network.mir_token } },
              amount: String(inital_token_liquidity_for_each),
            },
          ],
          slippage_tolerance: undefined,
        },
      },
      [new Coin("uusd", inital_ust_liquidity_for_each)]
    );
    await delay(300);
    console.log(`MIR-UST LP Tokens successfully minted on bombay-12 \n`);
    writeArtifact(network, terra.config.chainID);
  }

  // Mint ORION-UST LP Tokens : Deploy ORION, create ORION-UST Pair on terraswap : Mint LP Tokens
  // Mint ORION-UST LP Tokens : Deploy ORION, create ORION-UST Pair on terraswap : Mint LP Tokens
  // Mint ORION-UST LP Tokens : Deploy ORION, create ORION-UST Pair on terraswap : Mint LP Tokens
  // Mint ORION-UST LP Tokens : Deploy ORION, create ORION-UST Pair on terraswap : Mint LP Tokens
  if (!network.orion_token) {
    // deploy token
    network.orion_token = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "ORION Token",
        symbol: "ORION",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(1000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(1000_000_000_000000),
        },
      },
      "ORION Token for testing"
    );
    console.log(
      `ORION Token deployed successfully, address : ${network.orion_token}`
    );
    // Initialize terraswap pool
    let orion_init_pool_tx = await executeContract(
      terra,
      wallet,
      network.terraswap_factory_address,
      {
        create_pair: {
          asset_infos: [
            { token: { contract_addr: network.orion_token } },
            { native_token: { denom: "uusd" } },
          ],
        },
      }
    );
    let orion_init_pool_tx_resp =
      extract_terraswap_pool_info(orion_init_pool_tx);
    network.orion_ust_terraswap_pool = orion_init_pool_tx_resp.pool_address;
    network.orion_ust_terraswap_lp_token_address =
      orion_init_pool_tx_resp.lp_token_address;
    // Mint LP Tokens : increase_allowance and provide_liquidity
    await executeContract(terra, wallet, network.orion_token, {
      increase_allowance: {
        spender: network.orion_ust_terraswap_pool,
        amount: String(1000_000_000_000000),
      },
    });
    await delay(300);
    await executeContract(
      terra,
      wallet,
      network.orion_ust_terraswap_pool,
      {
        provide_liquidity: {
          assets: [
            {
              info: { native_token: { denom: "uusd" } },
              amount: String(inital_ust_liquidity_for_each),
            },
            {
              info: { token: { contract_addr: network.orion_token } },
              amount: String(inital_token_liquidity_for_each),
            },
          ],
          slippage_tolerance: undefined,
        },
      },
      [new Coin("uusd", inital_ust_liquidity_for_each)]
    );
    await delay(300);
    console.log(`ORION-UST LP Tokens successfully minted on bombay-12 \n`);
    writeArtifact(network, terra.config.chainID);
  }

  // Mint STT-UST LP Tokens : Deploy STT, create STT-UST Pair on terraswap : Mint LP Tokens
  // Mint STT-UST LP Tokens : Deploy STT, create STT-UST Pair on terraswap : Mint LP Tokens
  // Mint STT-UST LP Tokens : Deploy STT, create STT-UST Pair on terraswap : Mint LP Tokens
  // Mint STT-UST LP Tokens : Deploy STT, create STT-UST Pair on terraswap : Mint LP Tokens
  if (!network.stt_token) {
    // deploy token
    network.stt_token = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "STT Token",
        symbol: "STT",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(1000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(1000_000_000_000000),
        },
      },
      "STT Token for testing"
    );
    console.log(
      `STT Token deployed successfully, address : ${network.stt_token}`
    );
    // Initialize terraswap pool
    let stt_init_pool_tx = await executeContract(
      terra,
      wallet,
      network.terraswap_factory_address,
      {
        create_pair: {
          asset_infos: [
            { token: { contract_addr: network.stt_token } },
            { native_token: { denom: "uusd" } },
          ],
        },
      }
    );
    let stt_init_pool_tx_resp = extract_terraswap_pool_info(stt_init_pool_tx);
    network.stt_ust_terraswap_pool = stt_init_pool_tx_resp.pool_address;
    network.stt_ust_terraswap_lp_token_address =
      stt_init_pool_tx_resp.lp_token_address;
    // Mint LP Tokens : increase_allowance and provide_liquidity
    await executeContract(terra, wallet, network.stt_token, {
      increase_allowance: {
        spender: network.stt_ust_terraswap_pool,
        amount: String(1000_000_000_000000),
      },
    });
    await delay(300);
    await executeContract(
      terra,
      wallet,
      network.stt_ust_terraswap_pool,
      {
        provide_liquidity: {
          assets: [
            {
              info: { native_token: { denom: "uusd" } },
              amount: String(inital_ust_liquidity_for_each),
            },
            {
              info: { token: { contract_addr: network.stt_token } },
              amount: String(inital_token_liquidity_for_each),
            },
          ],
          slippage_tolerance: undefined,
        },
      },
      [new Coin("uusd", inital_ust_liquidity_for_each)]
    );
    await delay(300);
    console.log(`STT-UST LP Tokens successfully minted on bombay-12 \n`);
    writeArtifact(network, terra.config.chainID);
  }

  // Mint VKR-UST LP Tokens : Deploy VKR, create VKR-UST Pair on terraswap : Mint LP Tokens
  // Mint VKR-UST LP Tokens : Deploy VKR, create VKR-UST Pair on terraswap : Mint LP Tokens
  // Mint VKR-UST LP Tokens : Deploy VKR, create VKR-UST Pair on terraswap : Mint LP Tokens
  // Mint VKR-UST LP Tokens : Deploy VKR, create VKR-UST Pair on terraswap : Mint LP Tokens
  if (!network.vkr_token) {
    // deploy token
    network.vkr_token = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "VKR Token",
        symbol: "VKR",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(1000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(1000_000_000_000000),
        },
      },
      "VKR Token for testing"
    );
    console.log(
      `VKR Token deployed successfully, address : ${network.vkr_token}`
    );
    // Initialize terraswap pool
    let vkr_init_pool_tx = await executeContract(
      terra,
      wallet,
      network.terraswap_factory_address,
      {
        create_pair: {
          asset_infos: [
            { token: { contract_addr: network.vkr_token } },
            { native_token: { denom: "uusd" } },
          ],
        },
      }
    );
    let vkr_init_pool_tx_resp = extract_terraswap_pool_info(vkr_init_pool_tx);
    network.vkr_ust_terraswap_pool = vkr_init_pool_tx_resp.pool_address;
    network.vkr_ust_terraswap_lp_token_address =
      vkr_init_pool_tx_resp.lp_token_address;
    // Mint LP Tokens : increase_allowance and provide_liquidity
    await executeContract(terra, wallet, network.vkr_token, {
      increase_allowance: {
        spender: network.vkr_ust_terraswap_pool,
        amount: String(1000_000_000_000000),
      },
    });
    await delay(300);
    await executeContract(
      terra,
      wallet,
      network.vkr_ust_terraswap_pool,
      {
        provide_liquidity: {
          assets: [
            {
              info: { native_token: { denom: "uusd" } },
              amount: String(inital_ust_liquidity_for_each),
            },
            {
              info: { token: { contract_addr: network.vkr_token } },
              amount: String(inital_token_liquidity_for_each),
            },
          ],
          slippage_tolerance: undefined,
        },
      },
      [new Coin("uusd", inital_ust_liquidity_for_each)]
    );
    console.log(`VKR-UST LP Tokens successfully minted on bombay-12 \n`);
    writeArtifact(network, terra.config.chainID);
  }

  // Mint MINE-UST LP Tokens : Deploy MINE, create MINE-UST Pair on terraswap : Mint LP Tokens
  // Mint MINE-UST LP Tokens : Deploy MINE, create MINE-UST Pair on terraswap : Mint LP Tokens
  // Mint MINE-UST LP Tokens : Deploy MINE, create MINE-UST Pair on terraswap : Mint LP Tokens
  // Mint MINE-UST LP Tokens : Deploy MINE, create MINE-UST Pair on terraswap : Mint LP Tokens
  if (!network.mine_token) {
    // deploy token
    network.mine_token = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "MINE Token",
        symbol: "MINE",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(1000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(1000_000_000_000000),
        },
      },
      "MINE Token for testing"
    );
    console.log(
      `MINE Token deployed successfully, address : ${network.mine_token}`
    );
    // Initialize terraswap pool
    let mine_init_pool_tx = await executeContract(
      terra,
      wallet,
      network.terraswap_factory_address,
      {
        create_pair: {
          asset_infos: [
            { token: { contract_addr: network.mine_token } },
            { native_token: { denom: "uusd" } },
          ],
        },
      }
    );
    await delay(300);
    let mine_init_pool_tx_resp = extract_terraswap_pool_info(mine_init_pool_tx);
    network.mine_ust_terraswap_pool = mine_init_pool_tx_resp.pool_address;
    network.mine_ust_terraswap_lp_token_address =
      mine_init_pool_tx_resp.lp_token_address;
    // Mint LP Tokens : increase_allowance and provide_liquidity
    await executeContract(terra, wallet, network.mine_token, {
      increase_allowance: {
        spender: network.mine_ust_terraswap_pool,
        amount: String(1000_000_000_000000),
      },
    });
    await delay(300);
    await executeContract(
      terra,
      wallet,
      network.mine_ust_terraswap_pool,
      {
        provide_liquidity: {
          assets: [
            {
              info: { native_token: { denom: "uusd" } },
              amount: String(inital_ust_liquidity_for_each),
            },
            {
              info: { token: { contract_addr: network.mine_token } },
              amount: String(inital_token_liquidity_for_each),
            },
          ],
          slippage_tolerance: undefined,
        },
      },
      [new Coin("uusd", inital_ust_liquidity_for_each)]
    );
    await delay(300);
    console.log(`MINE-UST LP Tokens successfully minted on bombay-12 \n`);
    writeArtifact(network, terra.config.chainID);
  }

  // Mint PSI-UST LP Tokens : Deploy PSI, create PSI-UST Pair on terraswap : Mint LP Tokens
  // Mint PSI-UST LP Tokens : Deploy PSI, create PSI-UST Pair on terraswap : Mint LP Tokens
  // Mint PSI-UST LP Tokens : Deploy PSI, create PSI-UST Pair on terraswap : Mint LP Tokens
  // Mint PSI-UST LP Tokens : Deploy PSI, create PSI-UST Pair on terraswap : Mint LP Tokens
  if (!network.psi_token) {
    // deploy token
    network.psi_token = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "PSI Token",
        symbol: "PSI",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(1000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(1000_000_000_000000),
        },
      },
      "PSI Token for testing"
    );
    console.log(
      `PSI Token deployed successfully, address : ${network.psi_token}`
    );
    // Initialize terraswap pool
    let psi_init_pool_tx = await executeContract(
      terra,
      wallet,
      network.terraswap_factory_address,
      {
        create_pair: {
          asset_infos: [
            { token: { contract_addr: network.psi_token } },
            { native_token: { denom: "uusd" } },
          ],
        },
      }
    );
    let psi_init_pool_tx_resp = extract_terraswap_pool_info(psi_init_pool_tx);
    network.psi_ust_terraswap_pool = psi_init_pool_tx_resp.pool_address;
    network.psi_ust_terraswap_lp_token_address =
      psi_init_pool_tx_resp.lp_token_address;
    // Mint LP Tokens : increase_allowance and provide_liquidity
    await executeContract(terra, wallet, network.psi_token, {
      increase_allowance: {
        spender: network.psi_ust_terraswap_pool,
        amount: String(1000_000_000_000000),
      },
    });
    await delay(300);
    await executeContract(
      terra,
      wallet,
      network.psi_ust_terraswap_pool,
      {
        provide_liquidity: {
          assets: [
            {
              info: { native_token: { denom: "uusd" } },
              amount: String(inital_ust_liquidity_for_each),
            },
            {
              info: { token: { contract_addr: network.psi_token } },
              amount: String(inital_token_liquidity_for_each),
            },
          ],
          slippage_tolerance: undefined,
        },
      },
      [new Coin("uusd", inital_ust_liquidity_for_each)]
    );
    await delay(300);
    console.log(`PSI-UST LP Tokens successfully minted on bombay-12 \n`);
    writeArtifact(network, terra.config.chainID);
  }

  // Mint APOLLO-UST LP Tokens : Deploy APOLLO, create APOLLO-UST Pair on terraswap : Mint LP Tokens
  // Mint APOLLO-UST LP Tokens : Deploy APOLLO, create APOLLO-UST Pair on terraswap : Mint LP Tokens
  // Mint APOLLO-UST LP Tokens : Deploy APOLLO, create APOLLO-UST Pair on terraswap : Mint LP Tokens
  // Mint APOLLO-UST LP Tokens : Deploy APOLLO, create APOLLO-UST Pair on terraswap : Mint LP Tokens
  if (!network.apollo_token) {
    // deploy token
    network.apollo_token = await instantiateContract(
      terra,
      wallet,
      network.cw20_token_code_id,
      {
        name: "APOLLO Token",
        symbol: "APOLLO",
        decimals: 6,
        initial_balances: [
          {
            address: wallet.key.accAddress,
            amount: String(1000_000_000_000000),
          },
        ],
        mint: {
          minter: wallet.key.accAddress,
          cap: String(1000_000_000_000000),
        },
      },
      "APOLLO Token for testing"
    );
    console.log(
      `APOLLO Token deployed successfully, address : ${network.apollo_token}`
    );
    // Initialize terraswap pool
    let apollo_init_pool_tx = await executeContract(
      terra,
      wallet,
      network.terraswap_factory_address,
      {
        create_pair: {
          asset_infos: [
            { token: { contract_addr: network.apollo_token } },
            { native_token: { denom: "uusd" } },
          ],
        },
      }
    );
    await delay(300);
    let apollo_init_pool_tx_resp =
      extract_terraswap_pool_info(apollo_init_pool_tx);
    network.apollo_ust_terraswap_pool = apollo_init_pool_tx_resp.pool_address;
    network.apollo_ust_terraswap_lp_token_address =
      apollo_init_pool_tx_resp.lp_token_address;
    // Mint LP Tokens : increase_allowance and provide_liquidity
    await executeContract(terra, wallet, network.apollo_token, {
      increase_allowance: {
        spender: network.apollo_ust_terraswap_pool,
        amount: String(1000_000_000_000000),
      },
    });
    await delay(300);
    await executeContract(
      terra,
      wallet,
      network.apollo_ust_terraswap_pool,
      {
        provide_liquidity: {
          assets: [
            {
              info: { native_token: { denom: "uusd" } },
              amount: String(inital_ust_liquidity_for_each),
            },
            {
              info: { token: { contract_addr: network.apollo_token } },
              amount: String(inital_token_liquidity_for_each),
            },
          ],
          slippage_tolerance: undefined,
        },
      },
      [new Coin("uusd", inital_ust_liquidity_for_each)]
    );

    console.log(`APOLLO-UST LP Tokens successfully minted on bombay-12 \n`);
    writeArtifact(network, terra.config.chainID);
  }

  writeArtifact(network, terra.config.chainID);
  console.log("FINISH");
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch(console.log);
