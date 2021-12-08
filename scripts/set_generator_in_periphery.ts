import "dotenv/config";
import {
  LegacyAminoMultisigPublicKey,
  MsgExecuteContract,
  SimplePublicKey,
} from "@terra-money/terra.js";
import {
  deployContract,
  executeContract,
  newClient,
  executeContractJsonForMultiSig,
  readArtifact,
  writeArtifact,
  Client,
} from "./helpers/helpers.js";
import { bombay_testnet, mainnet, Config } from "./deploy_configs.js";
import { join } from "path";

let MULTI_SIG_TO_USE = "";

const ARTIFACTS_PATH = "../artifacts";

async function main() {
  let CONFIGURATION: Config = bombay_testnet;

  // terra, wallet
  const { terra, wallet } = newClient();
  console.log(
    `chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`
  );

  // network : stores contract addresses
  const network = readArtifact(terra.config.chainID);
  console.log("network:", network);

  // Configuration to use based on network instance
  if (terra.config.chainID == "bombay-12") {
    MULTI_SIG_TO_USE = wallet.key.accAddress;
    CONFIGURATION = bombay_testnet;
  } else if (terra.config.chainID == "columbus-5") {
    CONFIGURATION = mainnet;
  }

  // Astroport generator addresss should be set
  if (!network.astroport_generator_address) {
    console.log(
      `Please deploy the generator contract and then set this address in the <chain-id>.json before running this script...`
    );
    return;
  }

  /*************************************** PERIPHERY :: IF NETWORK IS BOMBAY-12  *****************************************/

  if (terra.config.chainID == "bombay-12") {
    //  LOCKDROP::UpdateConfig :: SET astroport generator address in Lockdrop if bombay-12
    if (!network.generator_set_in_lockdrop) {
      console.log(
        `${terra.config.chainID} :: Setting astroport generator address in Lockdrop...`
      );
      let tx = await executeContract(
        terra,
        wallet,
        network.lockdropAddress,
        {
          update_config: {
            new_config: {
              owner: undefined,
              astro_token_address: undefined,
              auction_contract_address: undefined,
              generator_address: network.astroport_generator_address,
            },
          },
        },
        [],
        "Lockdrop : Setting astroport generator"
      );
      console.log(
        `Lockdrop ::: Astroport generator set successfully ::: ${tx.txhash}\n`
      );
      network.generator_set_in_lockdrop = true;
      writeArtifact(network, terra.config.chainID);
    } else {
      console.log(`Lockdrop ::: Astroport generator already set \n`);
    }

    //  Auction::UpdateConfig :: SET astroport generator address in Auction if bombay-12
    if (!network.generator_set_in_auction) {
      console.log(
        `${terra.config.chainID} :: Setting astroport generator address in Auction...`
      );
      let tx = await executeContract(
        terra,
        wallet,
        network.auction_Address,
        {
          update_config: {
            new_config: {
              owner: undefined,
              astro_ust_pair_address: undefined,
              generator_contract: network.astroport_generator_address,
            },
          },
        },
        [],
        "Auction : Setting astroport generator"
      );
      console.log(
        `Auction ::: Astroport generator set successfully ::: ${tx.txhash}\n`
      );
      network.generator_set_in_auction = true;
      writeArtifact(network, terra.config.chainID);
    } else {
      console.log(`Auction ::: Astroport generator already set \n`);
    }

    /*************************************** PERIPHERY :: IF NETWORK IS COLUMBUS-5  *****************************************/

    //   if (terra.config.chainID == "columbus-5") {
    //     // Multisig details:
    //     // Multisig details:
    //     // Multisig details:
    //     const MULTISIG_PUBLIC_KEYS = process.env
    //       .MULTISIG_PUBLIC_KEYS!.split(",")
    //       // terrad sorts keys of multisigs by comparing bytes of their address
    //       .sort((a, b) => {
    //         return Buffer.from(new SimplePublicKey(a).rawAddress()).compare(
    //           Buffer.from(new SimplePublicKey(b).rawAddress())
    //         );
    //       })
    //       .map((x) => new SimplePublicKey(x));

    //     const MULTISIG_THRESHOLD = parseInt(process.env.MULTISIG_THRESHOLD!);

    //     // PubKey
    //     const multisigPubKey = new LegacyAminoMultisigPublicKey(
    //       MULTISIG_THRESHOLD,
    //       MULTISIG_PUBLIC_KEYS
    //     );
    //     const multisigAddress = multisigPubKey.address();
    //     console.log("Astroport Multi-Sig:", multisigAddress);

    //     const accInfo = await terra.auth.accountInfo(multisigAddress);
    //     let sequence_number = accInfo.getSequenceNumber();

    //     // Purpose:  SET ASTRO Token and Auction Contract in Lockdrop
    //     // Contract Address: "Lockdrop Contract"
    //     if (!network.lockdrop_astro_token_set && !network.auction_set_in_lockdrop) {
    //       console.log(
    //         `${terra.config.chainID} :: Need to make Multi-sig tx to set ASTRO token address & Auction contract address in Lockdrop contract`
    //       );

    //       let unsigned_lockdrop_set_astro_and_auction =
    //         await executeContractJsonForMultiSig(
    //           terra,
    //           multisigAddress,
    //           sequence_number,
    //           accInfo.getPublicKey(),
    //           network.lockdropAddress,
    //           {
    //             update_config: {
    //               new_config: {
    //                 owner: undefined,
    //                 astro_token_address: network.astro_token_address,
    //                 auction_contract_address: network.auction_Address,
    //                 generator_address: undefined,
    //               },
    //             },
    //           },
    //           CONFIGURATION.memos.lockdrop_set_astro
    //         );
    //       writeArtifact(
    //         unsigned_lockdrop_set_astro_and_auction,
    //         `${sequence_number}-unsigned_lockdrop_set_astro_and_auction`
    //       );
    //       console.log(
    //         `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_set_astro_and_auction.json successfully created.\n`
    //       );
    //       network.lockdrop_astro_token_set = true;
    //       network.auction_set_in_lockdrop = true;
    //       writeArtifact(network, terra.config.chainID);
    //       sequence_number += 1;
    //     }

    //     // Purpose:  SET Auction Contract in Airdrop
    //     // Contract Address: "Airdrop Contract"
    //     if (!network.auction_set_in_airdrop) {
    //       console.log("Set auction_address in Airdrop Contract ...");
    //       // update Config Tx
    //       let out = await executeContract(
    //         terra,
    //         wallet,
    //         network.airdrop_Address,
    //         {
    //           update_config: {
    //             owner: undefined,
    //             auction_contract_address: network.auction_Address,
    //             merkle_roots: undefined,
    //             from_timestamp: undefined,
    //             to_timestamp: undefined,
    //           },
    //         },
    //         [],
    //         " ASTRO Airdrop : Set Auction address "
    //       );
    //       console.log(
    //         `${terra.config.chainID} :: Setting auction contract address in ASTRO Airdrop contract,  ${out.txhash}`
    //       );
    //       network.auction_set_in_airdrop = true;
    //       writeArtifact(network, terra.config.chainID);
    //       sequence_number += 1;
    //     }

    //     // Purpose:  Transfer ASTRO to Lockdrop and set total incentives
    //     // Contract Address: "Lockdrop Contract"
    //     if (!network.lockdrop_astro_token_transferred) {
    //       console.log(
    //         `${terra.config.chainID} :: Need to make Multi-sig tx to transfer ASTRO and set incentives in Lockdrop contract`
    //       );

    //       let unsigned_lockdrop_increase_astro_incentives =
    //         await executeContractJsonForMultiSig(
    //           terra,
    //           multisigAddress,
    //           sequence_number,
    //           accInfo.getPublicKey(),
    //           network.lockdropAddress,
    //           {
    //             send: {
    //               contract: network.lockdropAddress,
    //               amount: String(LOCKDROP_INCENTIVES),
    //               msg: Buffer.from(
    //                 JSON.stringify({ increase_astro_incentives: {} })
    //               ).toString("base64"),
    //             },
    //           },
    //           "Transfer ASTRO and set Lockdrop incentives"
    //         );
    //       writeArtifact(
    //         unsigned_lockdrop_increase_astro_incentives,
    //         `${sequence_number}-unsigned_lockdrop_increase_astro_incentives`
    //       );
    //       console.log(
    //         `${terra.config.chainID} :: ${sequence_number}-unsigned_lockdrop_increase_astro_incentives.json successfully created.\n`
    //       );
    //       network.lockdrop_astro_token_transferred = true;
    //       writeArtifact(network, terra.config.chainID);
    //       sequence_number += 1;
    //     }

    //     // Purpose:  Transfer ASTRO to Airdrop
    //     // Contract Address: "ASTRO Token Contract"
    //     if (!network.airdrop_astro_token_transferred) {
    //       let unsigned_transfer_astro_to_airdrop =
    //         await executeContractJsonForMultiSig(
    //           terra,
    //           multisigAddress,
    //           sequence_number,
    //           accInfo.getPublicKey(),
    //           network.lockdropAddress,
    //           {
    //             transfer: {
    //               recipient: network.airdrop_Address,
    //               amount: String(AIRDROP_INCENTIVES),
    //             },
    //           },
    //           "Transfer ASTRO to Airdrop Contract"
    //         );
    //       writeArtifact(
    //         unsigned_transfer_astro_to_airdrop,
    //         `${sequence_number}-unsigned_transfer_astro_to_airdrop`
    //       );
    //       network.airdrop_astro_token_transferred = true;
    //       console.log(
    //         `${terra.config.chainID} :: ${sequence_number}-unsigned_transfer_astro_to_airdrop.json successfully created.\n`
    //       );
    //       sequence_number += 1;
    //     }

    //     // Purpose:  Transfer ASTRO to Auction to set incentives
    //     // Contract Address: "ASTRO Token Contract"
    //     if (!network.auction_astro_token_transferred) {
    //       let unsigned_transfer_astro_to_auction =
    //         await executeContractJsonForMultiSig(
    //           terra,
    //           multisigAddress,
    //           sequence_number,
    //           accInfo.getPublicKey(),
    //           network.auction_Address,
    //           {
    //             send: {
    //               contract: network.auction_Address,
    //               amount: String(AUCTION_INCENTIVES),
    //               msg: Buffer.from(
    //                 JSON.stringify({ increase_astro_incentives: {} })
    //               ).toString("base64"),
    //             },
    //           },
    //           "Transfer ASTRO to Auction Contract for participation incentives"
    //         );
    //       writeArtifact(
    //         unsigned_transfer_astro_to_auction,
    //         `${sequence_number}-unsigned_transfer_astro_to_auction`
    //       );
    //       console.log(
    //         `${terra.config.chainID} :: ${sequence_number}-unsigned_transfer_astro_to_auction.json successfully created.\n`
    //       );
    //       sequence_number += 1;
    //     }
    //   }

    writeArtifact(network, terra.config.chainID);
    console.log("FINISH");
  }
}

main().catch(console.log);
