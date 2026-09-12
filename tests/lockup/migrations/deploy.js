// deploy.js is a simple deploy script to initialize a program. This is run
// immediately after a deploy.

const serumCmn = require("@project-serum/common");
const anchor = require("@anchor-lang/core");
const PublicKey = anchor.web3.PublicKey;

module.exports = async function (provider) {
  // Configure client to use the provider.
  anchor.setProvider(provider);

  // Setup genesis state.
  const registrarConfigs = await genesis(provider);

  // Program clients.
  const lockup = anchor.workspace.Lockup;
  const registry = anchor.workspace.Registry;

  // Registry state constructor.
  await registry.state.rpc.new({
    accounts: {
      lockupProgram: lockup.programId,
    },
  });

  // Lockup state constructor.
  await lockup.state.rpc.new({
    accounts: {
      authority: provider.wallet.publicKey,
    },
  });

  // Delete the default whitelist entries.
  const defaultEntry = { programId: new anchor.web3.PublicKey.default() };
  await lockup.state.rpc.whitelistDelete(defaultEntry, {
    accounts: {
      authority: provider.wallet.publicKey,
    },
  });

  // Whitelist the registry.
  await lockup.state.rpc.whitelistAdd(
    { programId: registry.programId },
    {
      accounts: {
        authority: provider.wallet.publicKey,
      },
    }
  );

  // Initialize all registrars.
  const cfgKeys = Object.keys(registrarConfigs);
  for (let k = 0; k < cfgKeys.length; k += 1) {
    let r = registrarConfigs[cfgKeys[k]];
    const registrar = await registrarInit(
      registry,
      r.withdrawalTimelock,
      r.stakeRate,
      r.rewardQLen,
      new anchor.web3.PublicKey(r.mint)
    );
    r["registrar"] = registrar.toString();
  }

  // Generate code for whitelisting on UIs.
  const code = generateCode(registry, lockup, registrarConfigs);
  console.log("Generated whitelisted UI addresses:", code);
};

function generateCode(registry, lockup, registrarConfigs) {
  const registrars = Object.keys(registrarConfigs)
    .map((cfg) => `${cfg}: new PublicKey('${registrarConfigs[cfg].registrar}')`)
    .join(",");

  const mints = Object.keys(registrarConfigs)
    .map((cfg) => `${cfg}: new PublicKey('${registrarConfigs[cfg].mint}')`)
    .join(",");

  return `{
registryProgramId: new PublicKey('${registry.programId}'),
lockupProgramId: new PublicKey('${lockup.programId}'),
registrars: { ${registrars} },
mints: { ${mints} },
 }`;
}

async function genesis(provider) {
  if (
    provider.connection._rpcEndpoint === "https://api.mainnet-beta.solana.com"
  ) {
    return {
      srm: {
        withdrawalTimelock: 60 * 60 * 24 * 7, // 1 week.
        stakeRate: 500 * 10 ** 6, // 500 SRM.
        rewardQLen: 150,
        mint: "SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt",
      },
      msrm: {
        withdrawalTimelock: 60 * 60 * 24 * 7, // 1 week.
        stakeRate: 1,
        rewardQLen: 150,
        mint: "MSRMcoVyrFxnSgo5uXwone5SKcGhT1KEJMFEkMEWf9L",
      },
    };
  } else {
    const [token1Mint, _god1] = await serumCmn.createMintAndVault(
      provider,
      new anchor.BN(10000000000000),
      undefined,
      6
    );
    const [token2Mint, _god2] = await serumCmn.createMintAndVault(
      provider,
      new anchor.BN(10000000000),
      undefined,
      0
    );
    return {
      token1: {
        withdrawalTimelock: 60 * 60 * 24 * 7,
        stakeRate: 1000 * 10 ** 6,
        rewardQLen: 150,
        mint: token1Mint.toString(),
      },
      token2: {
        withdrawalTimelock: 60 * 60 * 24 * 7,
        stakeRate: 1,
        rewardQLen: 150,
        mint: token2Mint.toString(),
      },
    };
  }
}

async function registrarInit(
  registry,
  _withdrawalTimelock,
  _stakeRate,
  rewardQLen,
  mint
) {
  const registrar = anchor.web3.Keypair.generate();
  const rewardQ = anchor.web3.Keypair.generate();
  const withdrawalTimelock = new anchor.BN(_withdrawalTimelock);
  const stakeRate = new anchor.BN(_stakeRate);
  const [registrarSigner, nonce] =
    await anchor.web3.PublicKey.findProgramAddress(
      [registrar.publicKey.toBuffer()],
      registry.programId
    );
  const poolMint = await serumCmn.createMint(
    registry.provider,
    registrarSigner
  );
  await registry.rpc.initialize(
    mint,
    registry.provider.wallet.publicKey,
    nonce,
    withdrawalTimelock,
    stakeRate,
    rewardQLen,
    {
      accounts: {
        registrar: registrar.publicKey,
        poolMint,
        rewardEventQ: rewardQ.publicKey,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
      signers: [registrar, rewardQ],
      instructions: [
        await registry.account.registrar.createInstruction(registrar),
        await registry.account.rewardQueue.createInstruction(rewardQ, 8250),
      ],
    }
  );
  return registrar.publicKey;
}
