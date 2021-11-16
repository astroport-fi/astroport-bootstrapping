export const bombay_testnet: Config = {

    airdrop_InitMsg: {
        "config": {
            "owner": undefined,
            "astro_token_address": "",
            "merkle_roots": [],
            "from_timestamp": undefined,
            "to_timestamp": 0,
            "total_airdrop_size": "0",
        }
    },

    lockdrop_InitMsg: {
        "config": {
            "owner": undefined,
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
        "config": {
            "owner": undefined,
            "astro_token_address": "",
            "airdrop_contract_address": "",
            "lockdrop_contract_address": "",
            "lp_tokens_vesting_duration": 86400,
            "init_timestamp": 0,
            "deposit_window": 86400,
            "withdrawal_window": 86400,
        }
    }
}




interface AuctionInitMsg {
    config: {
        owner?: string
        astro_token_address: string
        airdrop_contract_address: string
        lockdrop_contract_address: string
        lp_tokens_vesting_duration: number
        init_timestamp: number
        deposit_window: number
        withdrawal_window: number
    }
}


interface LockdropInitMsg {
    config: {
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

interface AirdropInitMsg {
    config: {
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
    airdrop_InitMsg: AirdropInitMsg
}