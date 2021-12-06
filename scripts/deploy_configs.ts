export const mainnet: Config = {
  // timestamp 1638835200 :: Date and time (GMT): Tuesday, 7 December 2021 00:00:00
  // 24 hrs = 86400 :: 86400*5 = 5 days of deposit window
  // 24 hrs = 86400 :: 86400*2 = 2 days of deposit window
  // Lockup duration :: Min 2 weeks and max 52 weeks
  // multiplier / divider values for longer lockup multiple for more ASTRO = 3 / 51
  lockdrop_InitMsg: {
    config: {
      owner: undefined,
      init_timestamp: 1638835200,
      deposit_window: 86400 * 5,
      withdrawal_window: 86400 * 2,
      min_lock_duration: 2,
      max_lock_duration: 52,
      weekly_multiplier: 3,
      weekly_divider: 51,
      max_positions_per_user: 14,
    },
  },

  airdrop_InitMsg: {
    config: {
      owner: undefined,
      astro_token_address: "",
      merkle_roots: [],
      from_timestamp: 1638133200,
      to_timestamp: 1638133200 + 7 * 24 * 60 * 60,
    },
  },

  auction_InitMsg: {
    config: {
      owner: undefined,
      astro_token_address: "",
      airdrop_contract_address: "",
      lockdrop_contract_address: "",
      lp_tokens_vesting_duration: 3 * 30 * 24 * 60 * 60,
      init_timestamp: 1638133200 + 7 * 24 * 60 * 60,
      deposit_window: 5 * 24 * 60 * 60,
      withdrawal_window: 2 * 24 * 60 * 60,
    },
  },

  memos: {
    lockdrop:
      "ASTROPORT Launch : Phase 1  Lockdrop for liquidity migration to Astroport",
    airdrop: "ASTROPORT Launch : ASTRO Airdrop",
    auction:
      "ASTROPORT Launch : Auction for Bootstrapping ASTRO-UST LP Pool on Astroport",
    lockdrop_set_astro:
      "ASTROPORT Launch : Phase 1  Lockdrop :: Update ASTRO Token address",
  },
};

export const bombay_testnet: Config = {
  // 1 hr = 3600 :: 3600*5 = 5 hours of deposit window
  // 1 hr = 3600 :: 3600*2 = 2 hours of withdrawal window
  // Lockup duration :: Min 2 weeks and max 52 weeks [week means 1 hr when testing]
  // multiplier / divider values for longer lockup multiple for more ASTRO = 3 / 51
  lockdrop_InitMsg: {
    config: {
      owner: undefined,
      init_timestamp: 0,
      deposit_window: 0,
      withdrawal_window: 0,
      min_lock_duration: 2,
      max_lock_duration: 52,
      weekly_multiplier: 3,
      weekly_divider: 51,
      max_positions_per_user: 44,
    },
  },

  auction_InitMsg: {
    config: {
      owner: undefined,
      astro_token_address: "",
      airdrop_contract_address: "",
      lockdrop_contract_address: "",
      lp_tokens_vesting_duration: 0,
      init_timestamp: 0,
      deposit_window: 0,
      withdrawal_window: 0,
    },
  },

  airdrop_InitMsg: {
    config: {
      owner: undefined,
      astro_token_address: "",
      merkle_roots: [],
      from_timestamp: 0,
      to_timestamp: 0 + 0,
    },
  },

  memos: {
    lockdrop:
      "ASTROPORT Launch : Phase 1  Lockdrop for liquidity migration to Astroport",
    airdrop: "ASTROPORT Launch : ASTRO Airdrop",
    auction:
      "ASTROPORT Launch : Auction for Bootstrapping ASTRO-UST LP Pool on Astroport",
    lockdrop_set_astro:
      "ASTROPORT Launch : Phase 1  Lockdrop :: Update ASTRO Token address",
  },
};

interface AuctionInitMsg {
  config: {
    owner?: string;
    astro_token_address: string;
    airdrop_contract_address: string;
    lockdrop_contract_address: string;
    lp_tokens_vesting_duration: number;
    init_timestamp: number;
    deposit_window: number;
    withdrawal_window: number;
  };
}

interface LockdropInitMsg {
  config: {
    owner?: string;
    init_timestamp: number;
    deposit_window: number;
    withdrawal_window: number;
    min_lock_duration: number;
    max_lock_duration: number;
    weekly_multiplier: number;
    weekly_divider: number;
    max_positions_per_user: number;
  };
}

interface AirdropInitMsg {
  config: {
    owner?: string;
    astro_token_address: string;
    merkle_roots?: string[];
    from_timestamp?: number;
    to_timestamp: number;
  };
}

interface Memos {
  lockdrop: string;
  airdrop: string;
  auction: string;
  lockdrop_set_astro: string;
}

export interface Config {
  auction_InitMsg: AuctionInitMsg;
  lockdrop_InitMsg: LockdropInitMsg;
  airdrop_InitMsg: AirdropInitMsg;
  memos: Memos;
}
