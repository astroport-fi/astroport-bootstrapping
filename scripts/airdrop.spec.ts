import chalk from "chalk";
import { join } from "path"
import { LocalTerra, Wallet } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployContract, transferCW20Tokens, getCW20Balance } from "./helpers/helpers.js";
  import {updateAirdropConfig, claimAirdropForTerraUser, claimAirdropForEVMUser, transferAstroByAdminFromAirdropContract
    ,getAirdropConfig, isAirdropClaimed, verify_EVM_SignatureForAirdrop, get_EVM_Signature }  from "./helpers/airdrop_helpers.js";
import Web3 from 'web3';
import  {Terra_Merkle_Tree}  from "./helpers/terra_merkle_tree.js";
import  {EVM_Merkle_Tree}  from "./helpers/evm_merkle_tree.js";
import secp256k1 from 'secp256k1';
import { Bytes32, Bytes64 } from '@uniqys/types'
import { stringToU8a, u8aToHex } from '@hoprnet/hopr-utils'

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

const ARTIFACTS_PATH = "../artifacts"
const terra = new LocalTerra();
let web3 = new Web3(Web3.givenProvider || 'ws://some.local-or-remote.node:8546');

const deployer = terra.wallets.test1;

const terra_user_1 = terra.wallets.test2;
const terra_user_2 = terra.wallets.test3;
const terra_user_3 = terra.wallets.test4;
const terra_user_4 = terra.wallets.test5;

let astro_token_address: string;
let airdrop_contract_address: string;

//----------------------------------------------------------------------------------------
// Setup : Test
//----------------------------------------------------------------------------------------

async function setupTest() {

    let astro_token_config = { "name": "ASTRO",
                            "symbol": "ASTRO",
                            "decimals": 6,
                            "initial_balances": [ {"address":deployer.key.accAddress, "amount":"100000000000000"}], 
                            "mint": { "minter":deployer.key.accAddress, "cap":"100000000000000"}
                           }
    astro_token_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'cw20_token.wasm'),  astro_token_config )
    console.log(chalk.green(`$ASTRO deployed successfully, address : ${chalk.cyan(astro_token_address)}`));

    const init_timestamp = parseInt((Date.now()/1000).toFixed(0))
    const till_timestamp = init_timestamp + (86400 * 30)

    let airdrop_config = { "owner":  deployer.key.accAddress,
                         "astro_token_address": astro_token_address,
                         "terra_merkle_roots": [],
                         "evm_merkle_roots": [],
                         "from_timestamp": init_timestamp, 
                         "till_timestamp": till_timestamp, 
                        } 
    
    airdrop_contract_address = await deployContract(terra, deployer, join(ARTIFACTS_PATH, 'astro_airdrop.wasm'),  airdrop_config )    
    const airdropConfigResponse = await getAirdropConfig(terra, airdrop_contract_address);
      expect(airdropConfigResponse).to.deep.equal({
        astro_token_address: astro_token_address,
        owner: deployer.key.accAddress,
        terra_merkle_roots: [],
        evm_merkle_roots: [],
        from_timestamp: init_timestamp,
        till_timestamp: till_timestamp
      });
    console.log(chalk.green(`Airdrop Contract deployed successfully, address : ${chalk.cyan(airdrop_contract_address)}`));

    var contract_astro_balance_before_transfer = await getCW20Balance(terra, astro_token_address, airdrop_contract_address);
    var deployer_astro_balance_before_transfer = await getCW20Balance(terra, astro_token_address, deployer.key.accAddress);

    await transferCW20Tokens(terra, deployer, astro_token_address, airdrop_contract_address, 2500000 * 10**6 );

    var contract_astro_balance_after_transfer = await getCW20Balance(terra, astro_token_address, airdrop_contract_address);
    var deployer_astro_balance_after_transfer = await getCW20Balance(terra, astro_token_address, deployer.key.accAddress);

    expect(Number(contract_astro_balance_after_transfer) - Number(contract_astro_balance_before_transfer)).to.equal(2500000 * 10**6);
    expect(Number(deployer_astro_balance_before_transfer) - Number(deployer_astro_balance_after_transfer)).to.equal(2500000 * 10**6);
}

//----------------------------------------------------------------------------------------
// (ADMIN FUNCTION) Update Config : Test
//----------------------------------------------------------------------------------------

async function testUpdateConfig(terra_merkle_roots: [string], evm_merkle_roots: [string]) {
    process.stdout.write("Should update config info correctly... ");
    
    const init_timestamp = parseInt((Date.now()/1000).toFixed(0))
    const till_timestamp = init_timestamp + (86400 * 30)

    await updateAirdropConfig(terra, deployer, airdrop_contract_address,{ "update_config" : {  "new_config" : {
                                                                                                  "terra_merkle_roots": terra_merkle_roots,
                                                                                                  "evm_merkle_roots": evm_merkle_roots,
                                                                                                  "from_timestamp": init_timestamp, 
                                                                                                  "till_timestamp": till_timestamp,  
                                                                                                  }
                                                                                             }                                                                                                                      
                                                                        });

    const airdropConfigResponse = await getAirdropConfig(terra, airdrop_contract_address);
    expect(airdropConfigResponse).to.deep.equal({ astro_token_address: astro_token_address,
                                                  owner: deployer.key.accAddress,
                                                  terra_merkle_roots: terra_merkle_roots,
                                                  evm_merkle_roots: evm_merkle_roots,
                                                  from_timestamp: init_timestamp,
                                                  till_timestamp: till_timestamp,
                                                });
    console.log(chalk.green("\nTerra and evm merkle roots updated successfully"));                                
}

//----------------------------------------------------------------------------------------
// Airdrop Claim By Terra User : Test
//----------------------------------------------------------------------------------------

async function testClaimByTerraUser(claimeeWallet:Wallet, amountClaimed:number, merkle_proof: any, root_index: number ) {
    process.stdout.write( `Should process claim by terra user  ${chalk.cyan(claimeeWallet.key.accAddress)} correctly... `);

   let is_claimed_before = await isAirdropClaimed(terra, airdrop_contract_address, claimeeWallet.key.accAddress);
   expect( is_claimed_before ).to.deep.equal( { is_claimed: false } );

    var contract_astro_balance_before_claim = await getCW20Balance(terra, astro_token_address, airdrop_contract_address);
    var user_astro_balance_before_claim = await getCW20Balance(terra, astro_token_address, claimeeWallet.key.accAddress);

    await claimAirdropForTerraUser(terra,claimeeWallet, airdrop_contract_address,amountClaimed,merkle_proof,root_index);

    var contract_astro_balance_after_claim = await getCW20Balance(terra, astro_token_address, airdrop_contract_address);
    var user_astro_balance_after_claim = await getCW20Balance(terra, astro_token_address, claimeeWallet.key.accAddress);

    let is_claimed_after = await isAirdropClaimed(terra, airdrop_contract_address, claimeeWallet.key.accAddress);
    expect( is_claimed_after ).to.deep.equal( { is_claimed: true } );
 
    expect(Number(contract_astro_balance_before_claim) - Number(contract_astro_balance_after_claim)).to.equal(amountClaimed);
    expect(Number(user_astro_balance_after_claim) - Number(user_astro_balance_before_claim)).to.equal(amountClaimed);


    console.log(chalk.green( `\nClaim by terra user ${chalk.cyan(claimeeWallet.key.accAddress)} processed successfully` ));                                
}

//----------------------------------------------------------------------------------------
// Airdrop Claim By EVM User : Test
//----------------------------------------------------------------------------------------

async function testClaimByEvmUser(recepientWallet:Wallet, evm_address:string, amountClaimed:number, public_key:string, signed_msg_hash:string, signature:string, merkle_proof: any, root_index: number ) {
    process.stdout.write(`Should process claim by evm user ${chalk.cyan(evm_address)} correctly... `);

    var contract_astro_balance_before_claim = await getCW20Balance(terra, astro_token_address, airdrop_contract_address);
    var recepient_astro_balance_before_claim = await getCW20Balance(terra, astro_token_address, recepientWallet.key.accAddress );

    let is_claimed_before = await isAirdropClaimed(terra, airdrop_contract_address, evm_address.replace('0x','').toLowerCase() );
    expect( is_claimed_before ).to.deep.equal( { is_claimed: false } ); 

    let verification_response = await verify_EVM_SignatureForAirdrop(terra, airdrop_contract_address, evm_address, signature, signed_msg_hash);
    expect(verification_response).to.deep.equal({   is_valid: true,
                                                    public_key: "04" + public_key,
                                                    recovered_address: evm_address.replace('0x','').toLowerCase()
                                                });

    await claimAirdropForEVMUser(terra,recepientWallet, airdrop_contract_address, evm_address, amountClaimed, merkle_proof, root_index, signature, signed_msg_hash);

    var contract_astro_balance_after_claim = await getCW20Balance(terra, astro_token_address, airdrop_contract_address);
    var recepientWallet_astro_balance_after_claim = await getCW20Balance(terra, astro_token_address, recepientWallet.key.accAddress);

    let is_claimed_after = await isAirdropClaimed(terra, airdrop_contract_address, evm_address.replace('0x','').toLowerCase() );
    expect( is_claimed_after ).to.deep.equal( { is_claimed: true } ); 

    expect(Number(contract_astro_balance_before_claim) - Number(contract_astro_balance_after_claim)).to.equal(amountClaimed);
    expect(Number(recepientWallet_astro_balance_after_claim) - Number(recepient_astro_balance_before_claim)).to.equal(amountClaimed);

    console.log(chalk.green(`\nClaim by evm user ${chalk.cyan(evm_address)} processed successfully`));                                
}


//----------------------------------------------------------------------------------------
// (ADMIN FUNCTION) Transfer ASTRO Tokens : Test
//----------------------------------------------------------------------------------------

async function testTransferAstroByAdmin(recepient_address:string, amountToTransfer:number) {
    process.stdout.write("Should transfer ASTRO from the Airdrop Contract correctly... ");
    
    var contract_astro_balance_before_claim = await getCW20Balance(terra, astro_token_address, airdrop_contract_address);
    var recepient_astro_balance_before_claim = await getCW20Balance(terra, astro_token_address, recepient_address );

    await transferAstroByAdminFromAirdropContract(terra,deployer, airdrop_contract_address, recepient_address, amountToTransfer);

    var contract_astro_balance_after_claim = await getCW20Balance(terra, astro_token_address, airdrop_contract_address);
    var recepientWallet_astro_balance_after_claim = await getCW20Balance(terra, astro_token_address, recepient_address );

    expect(Number(contract_astro_balance_before_claim) - Number(contract_astro_balance_after_claim)).to.equal(amountToTransfer);
    expect(Number(recepientWallet_astro_balance_after_claim) - Number(recepient_astro_balance_before_claim)).to.equal(amountToTransfer);

    console.log(chalk.green("\nTransfer of ASTRO tokens by the deployer with admin privileges processed successfully"));                                
}





//----------------------------------------------------------------------------------------
// Main
//----------------------------------------------------------------------------------------

(async () => {
    console.log(chalk.yellow("\n Airdrop Test: Info"));
  
    const toHexString = (bytes: Uint8Array) => bytes.reduce((str:string, byte:any) => str + byte.toString(16).padStart(2, '0'), '');

    console.log(`Deployer ::  ${chalk.cyan(deployer.key.accAddress)}`);

    console.log(`${chalk.cyan(terra_user_1.key.accAddress)} as Airdrop clamiee (terra) #1`);
    console.log(`${chalk.cyan(terra_user_2.key.accAddress)} as Airdrop clamiee (terra) #2`);
    console.log(`${chalk.cyan(terra_user_3.key.accAddress)} as Airdrop clamiee (terra) #3`);
    console.log(`${chalk.cyan(terra_user_4.key.accAddress)} as Airdrop clamiee (terra) #4`);

    const PRIVATE_KEY_1 = '89fa5355adfd0879b7dc568ac8b5d543d7609a96b0d8aa0486305403b7429c50';
    const PRIVATE_KEY_2 = 'd356a63b346fc50f9ba7072eac5d4c48939b7f05089a7e830462191ef78dfde4';
    const PRIVATE_KEY_3 = '43d63eecb4814fdf367f3da2022098b0336f71b5e2d31db050f4cd8e5552ba9a';
    const PRIVATE_KEY_4 = '0ee751375e50fa91413edf23501a965c7184db94e2f29d9c552796b94dbb004e';

    const evm_user_1 = web3.eth.accounts.privateKeyToAccount(PRIVATE_KEY_1);
    const evm_user_2 = web3.eth.accounts.privateKeyToAccount(PRIVATE_KEY_2);
    const evm_user_3 = web3.eth.accounts.privateKeyToAccount(PRIVATE_KEY_3);
    const evm_user_4 = web3.eth.accounts.privateKeyToAccount(PRIVATE_KEY_4);
    
    console.log(`${chalk.cyan(evm_user_1.address)} as Airdrop clamiee (evm) #1`);
    console.log(`${chalk.cyan(evm_user_2.address)} as Airdrop clamiee (evm) #2`);
    console.log(`${chalk.cyan(evm_user_4.address)} as Airdrop clamiee (evm) #4`);

    // Deploy the contracts
    console.log(chalk.yellow("\nAirdrop Test: Setup"));
    await setupTest();

    // UpdateConfig :: Test
    console.log(chalk.yellow("\nTest: Update Configuration"));
    let terra_claimees_data = [ {"address":terra_user_1.key.accAddress, "amount": (250 * 10**6).toString()  },
                                {"address":terra_user_2.key.accAddress, "amount": (1).toString()  },
                                {"address":terra_user_3.key.accAddress, "amount": (71000 * 10**6).toString()  },
                                {"address":terra_user_4.key.accAddress, "amount": ( 10**6).toString()  },
                              ]
    let evm_claimees_data = [   {"address":evm_user_1.address, "amount": (50 * 10**6).toString()  },
                                {"address":evm_user_2.address, "amount": (1).toString()  },
                                {"address":evm_user_3.address, "amount": (71 * 10**6).toString()  },
                                {"address":evm_user_4.address, "amount": (1 * 10**6).toString()  },
                            ]
    let merkle_tree_terra = new Terra_Merkle_Tree(terra_claimees_data);
    let terra_tree_root = merkle_tree_terra.getMerkleRoot();
    let merkle_tree_evm = new EVM_Merkle_Tree(evm_claimees_data);
    let evm_tree_root = merkle_tree_evm.getMerkleRoot();

    await testUpdateConfig( [terra_tree_root], [evm_tree_root] );

    // TransferAstroTokens :: Test 
    console.log(chalk.yellow("\nTest: Transfer ASTRO Tokens by Admin : "));
    await testTransferAstroByAdmin(terra.wallets.test5.key.accAddress, 41000 * 10**6);

    // ClaimByTerraUser :: Test #1
    console.log(chalk.yellow("\nTest #1: Airdrop Claim By Terra user : " +  chalk.cyan(terra_user_1.key.accAddress)  ));
    let merkle_proof_for_terra_user_1 = merkle_tree_terra.getMerkleProof( {"address":terra_user_1.key.accAddress, "amount": (250 * 10**6).toString()  } );
    await testClaimByTerraUser(terra_user_1, Number(terra_claimees_data[0]["amount"]), merkle_proof_for_terra_user_1, 0 )

    // ClaimByTerraUser :: Test #2
    console.log(chalk.yellow("\nTest #2: Airdrop Claim By Terra user : " + chalk.cyan(terra_user_2.key.accAddress) ));
    let merkle_proof_for_terra_user_2 = merkle_tree_terra.getMerkleProof( {"address":terra_user_2.key.accAddress, "amount": (1).toString()} );
    await testClaimByTerraUser(terra_user_2, Number(terra_claimees_data[1]["amount"]), merkle_proof_for_terra_user_2, 0 )
    
    // ClaimByTerraUser :: Test #3
    console.log(chalk.yellow("\nTest #3: Airdrop Claim By Terra user : " + chalk.cyan(terra_user_3.key.accAddress) ));
    let merkle_proof_for_terra_user_3 = merkle_tree_terra.getMerkleProof( {"address":terra_user_3.key.accAddress, "amount": (71000 * 10**6).toString()} );
    await testClaimByTerraUser(terra_user_3, Number(terra_claimees_data[2]["amount"]), merkle_proof_for_terra_user_3, 0 )

    // ClaimByTerraUser :: Test #4
    console.log(chalk.yellow("\nTest #4: Airdrop Claim By Terra user : " + chalk.cyan(terra_user_4.key.accAddress) ));
    let merkle_proof_for_terra_user_4 = merkle_tree_terra.getMerkleProof( {"address":terra_user_4.key.accAddress, "amount": ( 10**6).toString()} );
    await testClaimByTerraUser(terra_user_4, Number(terra_claimees_data[3]["amount"]), merkle_proof_for_terra_user_4, 0 )

    // ClaimByEVMUser :: Test #1
    console.log(chalk.yellow("\nTest #1: Airdrop Claim By EVM user : " + chalk.cyan(evm_user_1.address ) ));
    let publicKey_1 = secp256k1.publicKeyCreate(stringToU8a(PRIVATE_KEY_1), false).slice(1);
    let merkle_proof_for_evm_user_1 = merkle_tree_evm.getMerkleProof( {"address":evm_user_1.address, "amount": (50 * 10**6).toString()} );
    let signature_1 = get_EVM_Signature(evm_user_1, evm_user_1.address + terra.wallets.test7.key.accAddress  );
    await testClaimByEvmUser( terra.wallets.test7, evm_user_1.address, Number(evm_claimees_data[0]["amount"]), toHexString(publicKey_1), signature_1["messageHash"], signature_1["signature"], merkle_proof_for_evm_user_1, 0 )

    // ClaimByEVMUser :: Test #2
    console.log(chalk.yellow("\nTest #2: Airdrop Claim By EVM user : " + chalk.cyan( evm_user_2.address) ));
    let publicKey_2 = secp256k1.publicKeyCreate(stringToU8a(PRIVATE_KEY_2), false).slice(1);
    let merkle_proof_for_evm_user_2 = merkle_tree_evm.getMerkleProof( {"address":evm_user_2.address, "amount": (1).toString()  } );
    let signature_2 = get_EVM_Signature(evm_user_2, evm_user_2.address + terra.wallets.test8.key.accAddress  );
    await testClaimByEvmUser( terra.wallets.test8, evm_user_2.address, Number(evm_claimees_data[1]["amount"]), toHexString(publicKey_2), signature_2["messageHash"], signature_2["signature"], merkle_proof_for_evm_user_2, 0 )
    
    // ClaimByEVMUser :: Test #4
    console.log(chalk.yellow("\nTest #4: Airdrop Claim By EVM user : " + chalk.cyan(evm_user_4.address)  ));
    let privateKey_4 = Buffer.from(PRIVATE_KEY_4, 'hex');
    let publicKey_4 = secp256k1.publicKeyCreate(stringToU8a(PRIVATE_KEY_4), false).slice(1);
    let merkle_proof_for_evm_user_4 = merkle_tree_evm.getMerkleProof( {"address":evm_user_4.address, "amount":(1 * 10**6).toString()} );
    let signature_4 = get_EVM_Signature(evm_user_4, evm_user_4.address + terra.wallets.test10.key.accAddress  );
    await testClaimByEvmUser( terra.wallets.test10, evm_user_4.address, Number(evm_claimees_data[3]["amount"]), toHexString(publicKey_4), signature_4["messageHash"], signature_4["signature"], merkle_proof_for_evm_user_4, 0 )
    
    

    console.log("");
  })();

