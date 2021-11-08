export const bombay_testnet: Config = { 

    airdrop_InitMsg: {
        "config" : { 
            "owner": undefined,
            "astro_token_address": "",
            "merkle_roots": [],
            "from_timestamp": undefined, 
            "to_timestamp": 0, 
            "total_airdrop_size": "0", 
        } 
    },

    lockdrop_InitMsg: {
        "config" : { 
            "owner": "",
            "init_timestamp": 0,
            "deposit_window": 86400,         
            "withdrawal_window": 86400,      
            "min_lock_duration": 1,         
            "max_lock_duration": 52,
            "weekly_multiplier": 1,    
            "weekly_divider": 12,    
        }
    },

    auction_InitMsg: {
        "config" : { 
            "owner": "",
            "astro_token_address": "",
            "airdrop_contract_address": "",
            "lockdrop_contract_address": "",
            "astroport_lp_pool": undefined, 
            "lp_token_address": undefined,
            "generator_contract": undefined,
            "astro_rewards": "100000000",
            "astro_vesting_duration": 86400,
            "lp_tokens_vesting_duration": 86400,
            "init_timestamp":0,
            "deposit_window":86400,
            "withdrawal_window":86400,
        }
    },

    lockdropUpdateMsg: {
        "config" : { 
            "owner": undefined,
            "astro_token_address": undefined,
            "auction_contract_address": undefined,         
            "generator_address": undefined,      
            "lockdrop_incentives": undefined
        }
    }
}




interface AuctionInitMsg {
    config : { 
        owner: string
        astro_token_address: string
        airdrop_contract_address: string
        lockdrop_contract_address: string
        astroport_lp_pool?: string 
        lp_token_address?: string 
        generator_contract?: string 
        astro_rewards: string
        astro_vesting_duration: number
        lp_tokens_vesting_duration: number
        init_timestamp: number
        deposit_window: number
        withdrawal_window: number
    }
}


interface LockdropInitMsg {
    config : { 
        owner?: string
        init_timestamp: number
        deposit_window: number 
        withdrawal_window: number 
        min_lock_duration: number 
        max_lock_duration: number
        weekly_multiplier: number
        weekly_divider: number
    }
}

interface LockdropUpdateMsg {
    config : { 
        owner?: string
        astro_token_address?: string
        auction_contract_address?: string 
        generator_address?: string 
        lockdrop_incentives?: string 
    }
}


interface AirdropInitMsg {
    config : { 
        owner?: string
        astro_token_address: string
        merkle_roots?: string[]
        from_timestamp?: number 
        to_timestamp: number 
        total_airdrop_size: string 

    }
}


interface Config {
    auction_InitMsg: AuctionInitMsg
    lockdrop_InitMsg: LockdropInitMsg
    lockdropUpdateMsg: LockdropUpdateMsg
    airdrop_InitMsg: AirdropInitMsg
}