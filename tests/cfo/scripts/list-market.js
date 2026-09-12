#!/usr/bin/env node

// Script to list a market, logging the address to stdout.

const utils = require("../tests/utils");
const fs = require("fs");
const anchor = require("@anchor-lang/core");
const provider = anchor.AnchorProvider.local();
// hack so we don't have to update serum-common library
// to the new AnchorProvider class and Provider interface
provider.send = provider.sendAndConfirm;

async function main() {
  ORDERBOOK_ENV = await utils.initMarket({
    provider,
  });
  const out = {
    market: ORDERBOOK_ENV.marketA._decoded.ownAddress.toString(),
  };
  console.log(JSON.stringify(out));
}

main();
