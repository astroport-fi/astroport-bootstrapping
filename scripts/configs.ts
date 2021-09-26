export const bombay_testnet: Config = { 
    airdrop_InitMsg: {
        "config" : { 
            "owner": undefined,
            "astro_token_address": undefined,
            "terra_merkle_roots": [],
            "evm_merkle_roots": [],
            "from_timestamp": undefined, 
            "till_timestamp": undefined, 
        } 
    }
}




interface AirdropInitMsg {
    config : { 
        owner?: string
        astro_token_address?: string
        terra_merkle_roots?: string[]
        evm_merkle_roots?: string[]
        from_timestamp?: number 
        till_timestamp?: number 
    }
}


interface Config {
    airdrop_InitMsg: AirdropInitMsg
}
