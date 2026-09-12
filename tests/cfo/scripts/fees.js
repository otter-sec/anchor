#!/usr/bin/env node

const process = require("process");
const fs = require("fs");
const anchor = require("@anchor-lang/core");
const { Market, OpenOrders } = require("@project-serum/serum");
const Account = anchor.web3.Account;
const Program = anchor.Program;
const provider = anchor.AnchorProvider.local();
const secret = JSON.parse(fs.readFileSync("./scripts/market-maker.json"));
const MARKET_MAKER = new Account(secret);
const PublicKey = anchor.web3.PublicKey;

const DEX_PID = new PublicKey("srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX");

async function main() {
  const market = new PublicKey(process.argv[2]);
  while (true) {
    let marketClient = await Market.load(
      provider.connection,
      market,
      { commitment: "processed" },
      DEX_PID
    );
    console.log("Fees: ", marketClient._decoded.quoteFeesAccrued.toString());
    await sleep(3000);
  }
}

main();

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
