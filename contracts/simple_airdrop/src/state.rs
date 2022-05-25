use astroport_periphery::simple_airdrop::{Config, State, UserInfo};
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

/// Stores the config struct at the given key
pub const CONFIG: Item<Config> = Item::new("config");
/// Stores the state struct at the given key.
pub const STATE: Item<State> = Item::new("state");
/// Stores user information. Key is address, value is the user info struct
pub const USERS: Map<&Addr, UserInfo> = Map::new("users");
