#!/usr/bin/env node

// Script to infinitely post orders that are immediately filled.

const process = require("process");
const anchor = require("@anchor-lang/core");
const PublicKey = anchor.web3.PublicKey;
const { runTradeBot } = require("../tests/utils");

async function main() {
  const market = new PublicKey(process.argv[2]);
  const provider = anchor.AnchorProvider.local();
  runTradeBot(market, provider);
}

main();
