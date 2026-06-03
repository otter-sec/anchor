const spl = require("@solana/spl-token");
const anchor = require("@anchor-lang/core");
const serumCmn = require("@project-serum/common");
const TokenInstructions = require("@project-serum/serum").TokenInstructions;

// TODO: remove this constant once @project-serum/serum uses the same version
//       of @solana/web3.js as anchor (or switch packages).
const TOKEN_PROGRAM_ID = new anchor.web3.PublicKey(
  TokenInstructions.TOKEN_PROGRAM_ID.toString()
);

// Our own sleep function.
function sleep(ms) {
  console.log("Sleeping for", ms / 1000, "seconds");
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// Read the cluster's current `unix_timestamp` — the same value the on-chain
// `Clock::get()` observes. Pacing against this instead of `Date.now()`
// eliminates client/validator clock-skew flakiness.
async function getClusterTime(connection) {
  for (let attempt = 0; attempt < 20; attempt++) {
    const slot = await connection.getSlot("confirmed");
    const time = await connection.getBlockTime(slot);
    if (time !== null) return time;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("getBlockTime returned null for 20 consecutive slots");
}

// Poll the cluster clock until it is *strictly past* `targetUnixSecs`.
// Matches the semantics of the on-chain phase checks, which all use
// `clock.unix_timestamp <= boundary` — a tx landing in a block whose
// `unix_timestamp` equals the boundary still trips the check. Polling
// to `now > target` ensures the next tx observes a strictly later
// cluster clock.
async function waitUntilClusterTime(connection, targetUnixSecs) {
  let now = await getClusterTime(connection);
  while (now <= targetUnixSecs) {
    await sleep(Math.min(targetUnixSecs - now + 1, 1) * 1000);
    now = await getClusterTime(connection);
  }
}

async function getTokenAccount(provider, addr) {
  return await serumCmn.getTokenAccount(provider, addr);
}

async function createMint(provider, authority) {
  if (authority === undefined) {
    authority = provider.wallet.publicKey;
  }
  const mint = await spl.Token.createMint(
    provider.connection,
    provider.wallet.payer,
    authority,
    null,
    6,
    TOKEN_PROGRAM_ID
  );
  return mint;
}

async function createTokenAccount(provider, mint, owner) {
  const token = new spl.Token(
    provider.connection,
    mint,
    TOKEN_PROGRAM_ID,
    provider.wallet.payer
  );
  let vault = await token.createAccount(owner);
  return vault;
}

module.exports = {
  TOKEN_PROGRAM_ID,
  sleep,
  getClusterTime,
  waitUntilClusterTime,
  getTokenAccount,
  createTokenAccount,
  createMint,
};
