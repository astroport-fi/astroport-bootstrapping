import {executeContract} from "./helpers.js";
import { LCDClient, Wallet, LocalTerra} from "@terra-money/terra.js";
import utils from 'web3-utils';

//-----------------------------------------------------

// ------ ExecuteContract :: Function signatures ------
// - updateAirdropConfig(terra, wallet, airdropContractAdr, new_config_msg) --> UPDATE CONFIG (ADMIN PRIVILEDGES NEEDED)
// - claimAirdropForTerraUser(terra, wallet, airdropContractAdr, claim_amount, merkle_proof, root_index) -->  AIRDROP CLAIM BY TERRA USER
// - claimAirdropForEVMUser(terra, wallet, airdropContractAdr, claim_amount, merkle_proof, root_index, eth_address, signature) --> AIRDROP CLAIM BY EVM USER
// - transferAstroByAdminFromAirdropContract(terra, wallet, airdropContractAdr, recepient ,amount) --> TRANSFER ASTRO (ADMIN PRIVILEDGES NEEDED)
//------------------------------------------------------
//------------------------------------------------------
// ----------- Queries :: Function signatures ----------
// - getAirdropConfig(terra, airdropContractAdr) --> Returns configuration
// - isAirdropClaimed(terra, airdropContractAdr, address) --> Returns true if airdrop already claimed, else false
// - verify_EVM_SignatureForAirdrop(terra, airdropContractAdr, eth_user_address, signature, msg) --> Verifies ethereum signature (true / false)
//------------------------------------------------------


// UPDATE TERRA MERKLE ROOTS : EXECUTE TX
export async function updateAirdropConfig( terra: LocalTerra | LCDClient, wallet:Wallet, airdropContractAdr: string, new_config: any) {
    let resp = await executeContract(terra, wallet, airdropContractAdr, new_config );
}
  

// AIRDROP CLAIM BY TERRA USER : EXECUTE TX
export async function claimAirdropForTerraUser( terra: LocalTerra | LCDClient, wallet:Wallet, airdropContractAdr: string,  claim_amount: number, merkle_proof: any, root_index: number  ) {
    if ( merkle_proof.length > 1 ) {
      let claim_for_terra_msg = { "claim_by_terra_user": {'claim_amount': claim_amount.toString(), 'merkle_proof': merkle_proof, "root_index": root_index }};
        let resp = await executeContract(terra, wallet, airdropContractAdr, claim_for_terra_msg );
        return resp;        
    } else {
        console.log("AIRDROP TERRA CLAIM :: INVALID MERKLE PROOF");
    }
}
  
  
// AIRDROP CLAIM BY EVM USER : EXECUTE TX
export async function claimAirdropForEVMUser( terra: LocalTerra | LCDClient, wallet:Wallet, airdropContractAdr: string, eth_address: string, claim_amount: number, merkle_proof: any, root_index: number, signature: string, msg_hash:string ) {
    if ( merkle_proof.length > 1 ) {
        let claim_for_evm_msg = { "claim_by_evm_user": {'eth_address': eth_address.replace('0x', '').toLowerCase(), 'claim_amount': claim_amount.toString(), 'merkle_proof': merkle_proof, 'root_index': root_index, "signature": signature.substr(2,128)  , "signed_msg_hash": msg_hash.replace('0x', '') }};
        let resp = await executeContract(terra, wallet, airdropContractAdr, claim_for_evm_msg );
        return resp;        
    } else {
        console.log("AIRDROP EVM CLAIM :: INVALID MERKLE PROOF");
    }
}


// TRANSFER ASTRO TOKENS : EXECUTE TX
export async function transferAstroByAdminFromAirdropContract( terra: LocalTerra | LCDClient, wallet:Wallet, airdropContractAdr: string, recepient: string, amount: number) {
    try {
        let transfer_astro_msg = { "transfer_astro_tokens": {'recepient': recepient, 'amount': amount.toString() }};
        let resp = await executeContract(terra, wallet, airdropContractAdr, transfer_astro_msg );
        return resp;        
    }
    catch {
        console.log("ERROR IN transferAstroByAdminFromAirdropContract function")
    }        
}


// GET CONFIG : CONTRACT QUERY
export async function getAirdropConfig(  terra: LocalTerra | LCDClient, airdropContractAdr: string) {
    try {
        let res = await terra.wasm.contractQuery(airdropContractAdr, { "config": {} })
        return res;
    }
    catch {
        console.log("ERROR IN getAirdropConfig QUERY")
    }    
}

// IS CLAIMED : CONTRACT QUERY
export async function isAirdropClaimed(  terra: LocalTerra | LCDClient, airdropContractAdr: string, address: string ) {
    let is_claimed_msg = { "is_claimed": {'address': address }};
    try {
        let res = await terra.wasm.contractQuery(airdropContractAdr, is_claimed_msg)
        return res;
    }
    catch {
        console.log("ERROR IN isAirdropClaimed QUERY")
    }
    
}
  

// EVM SIGNATURE VERIFICATION : CONTRACT QUERY
export async function verify_EVM_SignatureForAirdrop(  terra: LocalTerra | LCDClient, airdropContractAdr: string, user_address: string, signature: string, msg: string ) {
    try {
        let verify_signature_msg = { "is_valid_signature": {'evm_address':user_address.replace('0x', '').toLowerCase(), 'evm_signature': signature.substr(2,128) , 'signed_msg_hash': msg.replace('0x', '') }};
        let res = await terra.wasm.contractQuery(airdropContractAdr, verify_signature_msg)
        return res;
    }
    catch {
        console.log("ERROR IN verify_EVM_SignatureForAirdrop QUERY")
    }        
}
  


// // GET NATIVE TOKEN BALANCE
// export async function getUserNativeAssetBalance(terra, native_asset, wallet_addr) {
//     let res = await terra.bank.balance(  wallet_addr );
//     let balances = JSON.parse(JSON.parse(JSON.stringify( res )));
//     for (let i=0; i<balances.length;i++) {
//         if ( balances[i].denom == native_asset ) {
//             return balances[i].amount;
//         }
//     }    
//     return 0;
// }


// function print_events(response) {
//     if (response.height > 0) {
//       let events_array = JSON.parse(response["raw_log"])[0]["events"];
//       let attributes = events_array[1]["attributes"];
//       for (let i=0; i < attributes.length; i++ ) {
//         console.log(attributes[i]);
//       }
//     }
//   }


// EVM AIRDROP : SIGN THE MESSAGE
export function get_EVM_Signature(evm_account:any, msg:string) {
    var message = utils.isHexStrict(msg) ? utils.hexToUtf8(msg) : msg;
    let signature =  evm_account.sign(message);    
    return signature;
}