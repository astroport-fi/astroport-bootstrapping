import 'dotenv/config'
import {getMerkleRoots}  from "./helpers/merkle_tree_utils.js";
import {
    deployContract,
    executeContract,
    newClient,
    queryContract,
    readArtifact,
    writeArtifact,
    Client
  } from "./helpers/helpers.js";
import { bombay_testnet } from "./deploy_configs.js";
import { LCDClient } from "@terra-money/terra.js"
import { join } from "path"
import { Coin } from '@terra-money/terra.js';

const ARTIFACTS_PATH = "../artifacts"
const terraswap_factory_address = "terra18qpjm4zkvqnpjpw0zn0tdr8gdzvt8au35v45xf"






async function add_pool(name:String, symbol:String, amount:number, send_to: String ) {

    const {terra, wallet} = newClient()
    console.log(`chainID: ${terra.config.chainID} wallet: ${wallet.key.accAddress}`)

    const deployed_contracts = readArtifact(terra.config.chainID)

    if (!deployed_contracts.lockdropAddress) {
      console.log(`Please deploy the Lockdrop contract and then set this address in the deploy config before running this script...`)
      return
    }
  

    const network =  readArtifact("bombay-12-deployed-pools")
    console.log('network:', network)

    network.symbol = symbol
    // Create dummy token
    network.cw20_token_address = await deployContract(  terra, 
                                                    wallet, 
                                                    join(ARTIFACTS_PATH, 'cw20_token.wasm'),  
                                                    { "name": name,
                                                    "symbol": symbol,
                                                    "decimals": 6,
                                                    "initial_balances": [ {"address":wallet.key.accAddress, "amount": String(amount) }], 
                                                    "mint": { "minter":wallet.key.accAddress, "cap": String(amount) }
                                                    }
                                                )
    console.log(` ${symbol} Contract Address : ${network.cw20_token_address}`)


    // Create terraswap Pair
    await executeContract( terra, wallet, terraswap_factory_address ,  
                                                    { "create_pair": { "asset_infos": [ { "native_token": { "denom": "uusd" }  },
                                                                                        { "token": { "contract_addr": network.cw20_token_address }  }   
                                                                                      ]   
                                                                     }
                                                    }
                          )
    
    // Query Pool addresses
    network.terra_pool_info = await queryContract( terra, terraswap_factory_address , {"pair": { "asset_infos": [ { "native_token": { "denom": "uusd" }  },
                                                                                                                  { "token": { "contract_addr": network.cw20_token_address }  }   
                                                                                                                ]   
                                                                                                }
                                                                                      } 
                                                  )
    // Add liquidity to terraswap Pool (fist increase allowance)
    await executeContract( terra, wallet, network.cw20_token_address ,   { "increase_allowance": { "spender": network.terra_pool_info.contract_addr, amount: String(amount) } } )

    await executeContract( terra, wallet, network.terra_pool_info.contract_addr ,   { "provide_liquidity": { "assets": [ { "info": { "native_token": { "denom": "uusd" }  }, "amount": String(300000000)   },
                                                                                                                          { "info": { "token": { "contract_addr": network.cw20_token_address }  }   , "amount": String(amount/10)   },
                                                                                                                        ],
                                                                                                                "slippage_tolerance": undefined,
                                                                                                                "receiver": undefined
                                                                                                      }
                                                                                      }, 
                                                                                      [new Coin("uusd", 300000000)]
                          )

    // Query LP Token balance
    network.liquidity_token_balance = await queryContract( terra, network.terra_pool_info.liquidity_token , {"balance": { "address":  wallet.key.accAddress } }   )

    // Initialize Pool in Lockdrop 
    await executeContract( terra, wallet, deployed_contracts.lockdropAddress ,   { "initialize_pool": { "terraswap_lp_token":network.terra_pool_info.liquidity_token,
                                                                                                           "incentives_share": 10000000000,                                                                                                                
                                                                                                      }
                                                                                      },                                                                                       
                          )

    // Send LP TOkens if needed                                                                                  
   if (send_to) {
      await executeContract( terra, wallet, network.terra_pool_info.liquidity_token ,   { "transfer": { "recipient":send_to, "amount":String(network.liquidity_token_balance.balance) } })
      network.liquidity_token_balance = await queryContract( terra, network.terra_pool_info.liquidity_token , {"balance": { "address":  wallet.key.accAddress } }   )
    }                                                                            

                  
    console.log(network)


    writeArtifact(network, "bombay-12-deployed-pools")
    console.log('FINISH')
}




add_pool("PSI", "PSI", 100000000_000000, "terra1lv845g7szf9m3082qn3eehv9ewkjjr2kdyz0t6").catch(console.log)

