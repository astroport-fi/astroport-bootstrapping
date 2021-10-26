import chalk from "chalk";
import { join } from "path"
import { LocalTerra, Wallet } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployContract, transferCW20Tokens, getCW20Balance } from "./helpers/helpers.js";
