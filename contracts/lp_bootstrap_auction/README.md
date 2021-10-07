# LP Bootstrap via Auction Contract

The LP Bootstrap via auction contract facilitates ASTRO-UST Astroport pool initialization during the protocol launch. 

**Phase 1 :: Bootstrapping ASTRO and UST Side of the LP Pool**
* Airdrop recepients and lockdrop participants can delegate part / all of their  ASTRO rewards to the auction contract. 
* Any user can deposit UST directly to the auction contract to participate in the LP bootstrap auction.
* Users can deposit 
* Both UST deposited & ASTRO delegated (if any) balances are used to calculate user's LP token shares and additional ASTRO incentives that he will receive for participating in the auction. 

**Phase 2 :: Post ASTRO-UST Pool initialization**
* ASTRO reward withdrawals from lockdrop & airdrop contracts are enabled during the ASTRO-UST Pool initializaiton.
* ASTRO-UST LP tokens are staked with the generator contract, with LP Staking rewards allocated equally among the users based on their % LP share  
* ASTRO incentives claimable by the users  are also vested linearly on a 10 day period
* Users ASTRO-UST LP shares are also vested linearly on a 90 day period



## Contract Design

### Handle Messages

| Message                       | Description                                                                                         |
| ----------------------------- | --------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::Receive`   |  ReceiveCW20 Hook which facilitates ASTRO tokens delegation by lockdrop participants / airdrop recepients               |
| `ExecuteMsg::UpdateConfig`    | Admin function to update any of the configuration parameters.                                      |

| `ExecuteMsg::DepositUst`    | Facilitates UST deposits by users               |
| `ExecuteMsg::WithdrawUst`    | Facilitates UST withdrawals by users. 100% amount can be withdrawn during deposit window, which is then limited to 50% during 1st half of deposit window which then decreases linearly during 2nd half of deposit window. Only 1 withdrawal can be made by a user during the withdrawal window
| `ExecuteMsg::AddLiquidityToAstroportPool` | Admin function which facilitates Liquidity addtion to the Astroport ASTRO-UST Pool. Uses CallbackMsg to update state post liquidity addition to the pool |
| `ExecuteMsg::StakeLpTokens`          | Facilitates ASTRO withdrawal for airdrop recepients once claims are allowed      |
| `ExecuteMsg::ClaimRewards`          | Facilitates ASTRO rewards claim (staking incentives from generator and unvested lockdrop incentives) for users. Uses CallbackMsgs          |
| `ExecuteMsg::WithdrawLpShares`          | Facilitates withdrawal of LP shares which have been unlocked for the user. Uses CallbackMsgs    |



## Contract Design

### Handle Messages

| Message                       | Description                                                                                         |
| ----------------------------- | --------------------------------------------------------------------------------------------------- |
| `ExecuteMsg::ClaimByTerraUser`   |  Executes an airdrop claim for the Terra User                                                           |
| `ExecuteMsg::ClaimByEvmUser`    | Executes an airdrop claim for the EVM User                                         |
| `ExecuteMsg::TransferAstroTokens`          | Admin function. Transfers ASTRO tokens available with the contract to the recepient address.                                       |
| `ExecuteMsg::UpdateConfig`    | Admin function to update any of the configuration parameters.                                      |

### Query Messages

| Message              | Description                                                                        |
| -------------------- | ---------------------------------------------------------------------------------- |
| `QueryMsg::Config`   | Returns the config info                                                            |
| `QueryMsg::IsClaimed`    |Returns a boolean value indicating if the corresponding address have yet claimed the airdrop or not                                                |
| `QueryMsg::IsValidSignature` | Returns the recovered public key, corresponding evm address (lower case without `0x` prefix) and a boolean value indicating if the message was indeed signed by the provided address or not                                           |


## How to Guide :: Get merkle proofs

### Create distribution lists for terra and evm users

terra_claimees_data.json

```
{[ { address: 'terra1k0jntykt7e4g3y88ltc60czgjuqdy4c9ax8tx2',
    amount: '43454523323'
  },
  { address: 'terra1xzlgeyuuyqje79ma6vllregprkmgwgavjx2h6m',
    amount: '1343252443'
  }
]}
```

evm_claimees_data.json

```
{[ { address: '0x4dc06eeb995484aE670D4400238bA6C467A81315',
    amount: '15432'
  },
  { address: '0x0CF2570Ab8F962867e64313f34785E55845EF31C',
    amount: '4365434'
  }
]}
```

### Get proof with user input
```
    import  {Terra_Merkle_Tree}  from "./helpers/terra_merkle_tree.js";
    import  {EVM_Merkle_Tree}  from "./helpers/evm_merkle_tree.js";

    const terra_merkle_tree = new Terra_Merkle_Tree(terra_claimees_data);
    const terra_tree_root = terra_merkle_tree.getMerkleRoot();

    const evm_merkle_tree = new EVM_Merkle_Tree(evm_claimees_data);
    const evm_tree_root = evm_merkle_tree.getMerkleRoot();

    let merkle_proof_for_terra_user_ = terra_merkle_tree.getMerkleProof({  "address":"terra1k0jntykt7e4g3y88ltc60czgjuqdy4c9ax8tx2", 
                                                                            "amount": (43454523323).toString()
                                                                        } );

    let merkle_proof_for_evm_user_ = terra_merkle_tree.getMerkleProof({  "address":"0x4dc06eeb995484aE670D4400238bA6C467A81315", 
                                                                            "amount": (15432).toString()
                                                                        } );
    console.log("Terra Merkle Root ", terra_tree_root)
    console.log("Terra Merkle Proof ", merkle_proof_for_terra_user_)
    console.log("Verify Terra Merkle Proof ", terra_merkle_tree.verify({  "address":"terra1k0jntykt7e4g3y88ltc60czgjuqdy4c9ax8tx2", 
                                                                            "amount": (43454523323).toString()
                                                                        }) )


    console.log("Evm Merkle Root ", evm_tree_root)
    console.log("Evm Merkle Proof ", merkle_proof_for_evm_user_)
    console.log("Verify Evm Merkle Proof ", evm_merkle_tree.verify({  "address":"0x4dc06eeb995484aE670D4400238bA6C467A81315", 
                                                                            "amount": (15432).toString()
                                                                        }) )    
```


## How to Guide :: verify evm signatures

```
import utils from 'web3-utils';
import Web3 from 'web3';

var evm_wallet = web3.eth.accounts.privateKeyToAccount('<PRIVATE KEY>')
var msg_to_sign = <message to sign>
var signature =  evm_wallet.sign(msg_to_sign)

var evm_wallet_address = evm_wallet.replace('0x', '').toLowerCase()
var signed_msg_hash = signature["messageHash"].substr(2,66)
var signature_hash = signature["signature"].substr(2,128) 

var airdrop_contract_address = <Insert Contract Address>
var terra = new LCDClient({ URL: 'https://bombay-lcd.terra.dev', chainID: 'bombay-10'})
verify_signature_msg = { "is_valid_signature": {
                            'evm_address':evm_wallet_address, 
                            'evm_signature': signature_hash, 
                            'signed_msg_hash': signed_msg_hash 
                            }
                        };
var signature_response = terra.wasm.contractQuery(airdrop_contract_address, verify_signature_msg)
console.log(signature_response)
```


## Build schema and run unit-tests
```
cargo schema
cargo test
```


## License

TBD
