const anchor = require("@anchor-lang/core");
const serumCmn = require("@project-serum/common");
const { TOKEN_PROGRAM_ID } = require("@solana/spl-token");
const utils = require("./utils");
const { assert } = require("chai");
const nativeAssert = require("assert");

describe("Lockup and Registry", () => {
  // Read the provider from the configured environments.
  const provider = anchor.AnchorProvider.env();
  // hack so we don't have to update serum-common library
  // to the new AnchorProvider class and Provider interface
  provider.send = provider.sendAndConfirm;

  // Configure the client to use the provider.
  anchor.setProvider(provider);

  const lockup = anchor.workspace.Lockup;
  const registry = anchor.workspace.Registry;

  let lockupAddress = null;
  const WHITELIST_SIZE = 10;

  let mint = null;
  let god = null;

  it("Sets up initial test state", async () => {
    const [_mint, _god] = await serumCmn.createMintAndVault(
      provider,
      new anchor.BN(1000000)
    );
    mint = _mint;
    god = _god;
  });

  it("Is initialized!", async () => {
    await lockup.state.rpc.new({
      accounts: {
        authority: provider.wallet.publicKey,
      },
    });

    lockupAddress = await lockup.state.address();
    const lockupAccount = await lockup.state.fetch();

    assert.isTrue(lockupAccount.authority.equals(provider.wallet.publicKey));
    assert.strictEqual(lockupAccount.whitelist.length, WHITELIST_SIZE);
    lockupAccount.whitelist.forEach((e) => {
      assert.isTrue(e.programId.equals(anchor.web3.PublicKey.default));
    });
  });

  it("Deletes the default whitelisted addresses", async () => {
    const defaultEntry = { programId: anchor.web3.PublicKey.default };
    await lockup.state.rpc.whitelistDelete(defaultEntry, {
      accounts: {
        authority: provider.wallet.publicKey,
      },
    });
  });

  it("Sets a new authority", async () => {
    const newAuthority = anchor.web3.Keypair.generate();
    await lockup.state.rpc.setAuthority(newAuthority.publicKey, {
      accounts: {
        authority: provider.wallet.publicKey,
      },
    });

    let lockupAccount = await lockup.state.fetch();
    assert.isTrue(lockupAccount.authority.equals(newAuthority.publicKey));

    await lockup.state.rpc.setAuthority(provider.wallet.publicKey, {
      accounts: {
        authority: newAuthority.publicKey,
      },
      signers: [newAuthority],
    });

    lockupAccount = await lockup.state.fetch();
    assert.isTrue(lockupAccount.authority.equals(provider.wallet.publicKey));
  });

  const entries = [];

  it("Adds to the whitelist", async () => {
    const generateEntry = async () => {
      let programId = anchor.web3.Keypair.generate().publicKey;
      return {
        programId,
      };
    };

    for (let k = 0; k < WHITELIST_SIZE; k += 1) {
      entries.push(await generateEntry());
    }

    const accounts = {
      authority: provider.wallet.publicKey,
    };

    await lockup.state.rpc.whitelistAdd(entries[0], { accounts });

    let lockupAccount = await lockup.state.fetch();

    assert.lengthOf(lockupAccount.whitelist, 1);
    assert.deepEqual(lockupAccount.whitelist, [entries[0]]);

    for (let k = 1; k < WHITELIST_SIZE; k += 1) {
      await lockup.state.rpc.whitelistAdd(entries[k], { accounts });
    }

    lockupAccount = await lockup.state.fetch();

    assert.deepEqual(lockupAccount.whitelist, entries);

    await nativeAssert.rejects(
      async () => {
        const e = await generateEntry();
        await lockup.state.rpc.whitelistAdd(e, { accounts });
      },
      (err) => {
        assert.strictEqual(err.error.errorCode.number, 6008);
        assert.strictEqual(err.error.errorMessage, "Whitelist is full");
        return true;
      }
    );
  });

  it("Removes from the whitelist", async () => {
    await lockup.state.rpc.whitelistDelete(entries[0], {
      accounts: {
        authority: provider.wallet.publicKey,
      },
    });
    let lockupAccount = await lockup.state.fetch();
    assert.deepStrictEqual(lockupAccount.whitelist, entries.slice(1));
  });

  const vesting = anchor.web3.Keypair.generate();
  let vestingAccount = null;
  let vestingSigner = null;

  it("Creates a vesting account", async () => {
    const startTs = new anchor.BN(Date.now() / 1000);
    const endTs = new anchor.BN(startTs.toNumber() + 5);
    const periodCount = new anchor.BN(2);
    const beneficiary = provider.wallet.publicKey;
    const depositAmount = new anchor.BN(100);

    const vault = anchor.web3.Keypair.generate();
    let [_vestingSigner, nonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [vesting.publicKey.toBuffer()],
        lockup.programId
      );
    vestingSigner = _vestingSigner;

    await lockup.rpc.createVesting(
      beneficiary,
      depositAmount,
      nonce,
      startTs,
      endTs,
      periodCount,
      null, // Lock realizor is None.
      {
        accounts: {
          vesting: vesting.publicKey,
          vault: vault.publicKey,
          depositor: god,
          depositorAuthority: provider.wallet.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        },
        signers: [vesting, vault],
        instructions: [
          await lockup.account.vesting.createInstruction(vesting),
          ...(await serumCmn.createTokenAccountInstrs(
            provider,
            vault.publicKey,
            mint,
            vestingSigner
          )),
        ],
      }
    );

    vestingAccount = await lockup.account.vesting.fetch(vesting.publicKey);

    assert.isTrue(vestingAccount.beneficiary.equals(provider.wallet.publicKey));
    assert.isTrue(vestingAccount.mint.equals(mint));
    assert.isTrue(vestingAccount.grantor.equals(provider.wallet.publicKey));
    assert.isTrue(vestingAccount.outstanding.eq(depositAmount));
    assert.isTrue(vestingAccount.startBalance.eq(depositAmount));
    assert.isTrue(vestingAccount.whitelistOwned.eq(new anchor.BN(0)));
    assert.strictEqual(vestingAccount.nonce, nonce);
    assert.isTrue(vestingAccount.createdTs.gt(new anchor.BN(0)));
    assert.isTrue(vestingAccount.startTs.eq(startTs));
    assert.isTrue(vestingAccount.endTs.eq(endTs));
    assert.isNull(vestingAccount.realizor);
  });

  it("Fails to withdraw from a vesting account before vesting", async () => {
    await nativeAssert.rejects(
      async () => {
        await lockup.rpc.withdraw(new anchor.BN(100), {
          accounts: {
            vesting: vesting.publicKey,
            beneficiary: provider.wallet.publicKey,
            token: god,
            vault: vestingAccount.vault,
            vestingSigner: vestingSigner,
            tokenProgram: TOKEN_PROGRAM_ID,
            clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
          },
        });
      },
      (err) => {
        assert.strictEqual(err.error.errorCode.number, 6007);
        assert.strictEqual(
          err.error.errorMessage,
          "Insufficient withdrawal balance."
        );
        return true;
      }
    );
  });

  it("Waits for a vesting period to pass", async () => {
    await serumCmn.sleep(10 * 1000);
  });

  it("Withdraws from the vesting account", async () => {
    const token = await serumCmn.createTokenAccount(
      provider,
      mint,
      provider.wallet.publicKey
    );

    await lockup.rpc.withdraw(new anchor.BN(100), {
      accounts: {
        vesting: vesting.publicKey,
        beneficiary: provider.wallet.publicKey,
        token,
        vault: vestingAccount.vault,
        vestingSigner,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
      },
    });

    vestingAccount = await lockup.account.vesting.fetch(vesting.publicKey);
    assert.isTrue(vestingAccount.outstanding.eq(new anchor.BN(0)));

    const vaultAccount = await serumCmn.getTokenAccount(
      provider,
      vestingAccount.vault
    );
    assert.isTrue(vaultAccount.amount.eq(new anchor.BN(0)));

    const tokenAccount = await serumCmn.getTokenAccount(provider, token);
    assert.isTrue(tokenAccount.amount.eq(new anchor.BN(100)));
  });

  const registrar = anchor.web3.Keypair.generate();
  const rewardQ = anchor.web3.Keypair.generate();
  const withdrawalTimelock = new anchor.BN(4);
  const stakeRate = new anchor.BN(2);
  const rewardQLen = 170;
  let registrarAccount = null;
  let registrarSigner = null;
  let nonce = null;
  let poolMint = null;

  it("Creates registry genesis", async () => {
    const [_registrarSigner, _nonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [registrar.publicKey.toBuffer()],
        registry.programId
      );
    registrarSigner = _registrarSigner;
    nonce = _nonce;
    poolMint = await serumCmn.createMint(provider, registrarSigner);
  });

  it("Initializes registry's global state", async () => {
    await registry.state.rpc.new({
      accounts: { lockupProgram: lockup.programId },
    });

    const state = await registry.state.fetch();
    assert.isTrue(state.lockupProgram.equals(lockup.programId));

    // Should not allow a second initializatoin.
    await nativeAssert.rejects(
      async () => {
        await registry.state.rpc.new(lockup.programId);
      },
      (err) => {
        return true;
      }
    );
  });

  it("Initializes the registrar", async () => {
    await registry.rpc.initialize(
      mint,
      provider.wallet.publicKey,
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

    registrarAccount = await registry.account.registrar.fetch(
      registrar.publicKey
    );

    assert.isTrue(registrarAccount.authority.equals(provider.wallet.publicKey));
    assert.strictEqual(registrarAccount.nonce, nonce);
    assert.isTrue(registrarAccount.mint.equals(mint));
    assert.isTrue(registrarAccount.poolMint.equals(poolMint));
    assert.isTrue(registrarAccount.stakeRate.eq(stakeRate));
    assert.isTrue(registrarAccount.rewardEventQ.equals(rewardQ.publicKey));
    assert.isTrue(registrarAccount.withdrawalTimelock.eq(withdrawalTimelock));
  });

  const member = anchor.web3.Keypair.generate();
  let memberAccount = null;
  let memberSigner = null;
  let balances = null;
  let balancesLocked = null;

  it("Creates a member", async () => {
    const [_memberSigner, nonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [registrar.publicKey.toBuffer(), member.publicKey.toBuffer()],
        registry.programId
      );
    memberSigner = _memberSigner;

    const [mainTx, _balances] = await utils.createBalanceSandbox(
      provider,
      registrarAccount,
      memberSigner
    );
    const [lockedTx, _balancesLocked] = await utils.createBalanceSandbox(
      provider,
      registrarAccount,
      memberSigner
    );

    balances = _balances;
    balancesLocked = _balancesLocked;

    const tx = registry.transaction.createMember(nonce, {
      accounts: {
        registrar: registrar.publicKey,
        member: member.publicKey,
        beneficiary: provider.wallet.publicKey,
        memberSigner,
        balances,
        balancesLocked,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
      instructions: [await registry.account.member.createInstruction(member)],
    });

    const signers = [member, provider.wallet.payer];

    const allTxs = [mainTx, lockedTx, { tx, signers }];

    let txSigs = await provider.sendAll(allTxs);

    memberAccount = await registry.account.member.fetch(member.publicKey);

    assert.isTrue(memberAccount.registrar.equals(registrar.publicKey));
    assert.isTrue(memberAccount.beneficiary.equals(provider.wallet.publicKey));
    assert.isTrue(memberAccount.metadata.equals(anchor.web3.PublicKey.default));
    assert.strictEqual(
      JSON.stringify(memberAccount.balances),
      JSON.stringify(balances)
    );
    assert.strictEqual(
      JSON.stringify(memberAccount.balancesLocked),
      JSON.stringify(balancesLocked)
    );
    assert.strictEqual(memberAccount.rewardsCursor, 0);
    assert.isTrue(memberAccount.lastStakeTs.eq(new anchor.BN(0)));
  });

  it("Deposits (unlocked) to a member", async () => {
    const depositAmount = new anchor.BN(120);
    await registry.rpc.deposit(depositAmount, {
      accounts: {
        depositor: god,
        depositorAuthority: provider.wallet.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
        vault: memberAccount.balances.vault,
        beneficiary: provider.wallet.publicKey,
        member: member.publicKey,
      },
    });

    const memberVault = await serumCmn.getTokenAccount(
      provider,
      memberAccount.balances.vault
    );
    assert.isTrue(memberVault.amount.eq(depositAmount));
  });

  /*
  it("Stakes to a member (unlocked)", async () => {
    const stakeAmount = new anchor.BN(10);
    await registry.rpc.stake(stakeAmount, false, {
      accounts: {
        // Stake instance.
        registrar: registrar.publicKey,
        rewardEventQ: rewardQ.publicKey,
        poolMint,
        // Member.
        member: member.publicKey,
        beneficiary: provider.wallet.publicKey,
        balances,
        balancesLocked,
        // Program signers.
        memberSigner,
        registrarSigner,
        // Misc.
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
    });

    const vault = await serumCmn.getTokenAccount(
      provider,
      memberAccount.balances.vault
    );
    const vaultStake = await serumCmn.getTokenAccount(
      provider,
      memberAccount.balances.vaultStake
    );
    const spt = await serumCmn.getTokenAccount(
      provider,
      memberAccount.balances.spt
    );

    assert.isTrue(vault.amount.eq(new anchor.BN(100)));
    assert.isTrue(vaultStake.amount.eq(new anchor.BN(20)));
    assert.isTrue(spt.amount.eq(new anchor.BN(10)));
  });

  const unlockedVendor = anchor.web3.Keypair.generate();
  const unlockedVendorVault = anchor.web3.Keypair.generate();
  let unlockedVendorSigner = null;

  it("Drops an unlocked reward", async () => {
    const rewardKind = {
      unlocked: {},
    };
    const rewardAmount = new anchor.BN(200);
    const expiry = new anchor.BN(Date.now() / 1000 + 5);
    const [_vendorSigner, nonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [registrar.publicKey.toBuffer(), unlockedVendor.publicKey.toBuffer()],
        registry.programId
      );
    unlockedVendorSigner = _vendorSigner;

    await registry.rpc.dropReward(
      rewardKind,
      rewardAmount,
      expiry,
      provider.wallet.publicKey,
      nonce,
      {
        accounts: {
          registrar: registrar.publicKey,
          rewardEventQ: rewardQ.publicKey,
          poolMint,

          vendor: unlockedVendor.publicKey,
          vendorVault: unlockedVendorVault.publicKey,

          depositor: god,
          depositorAuthority: provider.wallet.publicKey,

          tokenProgram: TOKEN_PROGRAM_ID,
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [unlockedVendorVault, unlockedVendor],
        instructions: [
          ...(await serumCmn.createTokenAccountInstrs(
            provider,
            unlockedVendorVault.publicKey,
            mint,
            unlockedVendorSigner
          )),
          await registry.account.rewardVendor.createInstruction(unlockedVendor),
        ],
      }
    );

    const vendorAccount = await registry.account.rewardVendor.fetch(
      unlockedVendor.publicKey
    );

    assert.isTrue(vendorAccount.registrar.equals(registrar.publicKey));
    assert.isTrue(vendorAccount.vault.equals(unlockedVendorVault.publicKey));
    assert.strictEqual(vendorAccount.nonce, nonce);
    assert.isTrue(vendorAccount.poolTokenSupply.eq(new anchor.BN(10)));
    assert.isTrue(vendorAccount.expiryTs.eq(expiry));
    assert.isTrue(
      vendorAccount.expiryReceiver.equals(provider.wallet.publicKey)
    );
    assert.isTrue(vendorAccount.total.eq(rewardAmount));
    assert.isFalse(vendorAccount.expired);
    assert.strictEqual(vendorAccount.rewardEventQCursor, 0);
    assert.deepEqual(vendorAccount.kind, rewardKind);

    const rewardQAccount = await registry.account.rewardQueue.fetch(
      rewardQ.publicKey
    );
    assert.strictEqual(rewardQAccount.head, 1);
    assert.strictEqual(rewardQAccount.tail, 0);
    const e = rewardQAccount.events[0];
    assert.isTrue(e.vendor.equals(unlockedVendor.publicKey));
    assert.strictEqual(e.locked, false);
  });

  it("Collects an unlocked reward", async () => {
    const token = await serumCmn.createTokenAccount(
      provider,
      mint,
      provider.wallet.publicKey
    );
    await registry.rpc.claimReward({
      accounts: {
        to: token,
        cmn: {
          registrar: registrar.publicKey,

          member: member.publicKey,
          beneficiary: provider.wallet.publicKey,
          balances,
          balancesLocked,

          vendor: unlockedVendor.publicKey,
          vault: unlockedVendorVault.publicKey,
          vendorSigner: unlockedVendorSigner,

          tokenProgram: TOKEN_PROGRAM_ID,
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        },
      },
    });

    let tokenAccount = await serumCmn.getTokenAccount(provider, token);
    assert.isTrue(tokenAccount.amount.eq(new anchor.BN(200)));

    const memberAccount = await registry.account.member.fetch(member.publicKey);
    assert.strictEqual(memberAccount.rewardsCursor, 1);
  });

  const lockedVendor = anchor.web3.Keypair.generate();
  const lockedVendorVault = anchor.web3.Keypair.generate();
  let lockedVendorSigner = null;
  let lockedRewardAmount = null;
  let lockedRewardKind = null;

  it("Drops a locked reward", async () => {
    lockedRewardKind = {
      locked: {
        startTs: new anchor.BN(Date.now() / 1000),
        endTs: new anchor.BN(Date.now() / 1000 + 6),
        periodCount: new anchor.BN(2),
      },
    };
    lockedRewardAmount = new anchor.BN(200);
    const expiry = new anchor.BN(Date.now() / 1000 + 5);
    const [_vendorSigner, nonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [registrar.publicKey.toBuffer(), lockedVendor.publicKey.toBuffer()],
        registry.programId
      );
    lockedVendorSigner = _vendorSigner;

    await registry.rpc.dropReward(
      lockedRewardKind,
      lockedRewardAmount,
      expiry,
      provider.wallet.publicKey,
      nonce,
      {
        accounts: {
          registrar: registrar.publicKey,
          rewardEventQ: rewardQ.publicKey,
          poolMint,

          vendor: lockedVendor.publicKey,
          vendorVault: lockedVendorVault.publicKey,

          depositor: god,
          depositorAuthority: provider.wallet.publicKey,

          tokenProgram: TOKEN_PROGRAM_ID,
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [lockedVendorVault, lockedVendor],
        instructions: [
          ...(await serumCmn.createTokenAccountInstrs(
            provider,
            lockedVendorVault.publicKey,
            mint,
            lockedVendorSigner
          )),
          await registry.account.rewardVendor.createInstruction(lockedVendor),
        ],
      }
    );

    const vendorAccount = await registry.account.rewardVendor.fetch(
      lockedVendor.publicKey
    );

    assert.isTrue(vendorAccount.registrar.equals(registrar.publicKey));
    assert.isTrue(vendorAccount.vault.equals(lockedVendorVault.publicKey));
    assert.strictEqual(vendorAccount.nonce, nonce);
    assert.isTrue(vendorAccount.poolTokenSupply.eq(new anchor.BN(10)));
    assert.isTrue(vendorAccount.expiryTs.eq(expiry));
    assert.isTrue(
      vendorAccount.expiryReceiver.equals(provider.wallet.publicKey)
    );
    assert.isTrue(vendorAccount.total.eq(lockedRewardAmount));
    assert.isFalse(vendorAccount.expired);
    assert.strictEqual(vendorAccount.rewardEventQCursor, 1);
    assert.strictEqual(
      JSON.stringify(vendorAccount.kind),
      JSON.stringify(lockedRewardKind)
    );

    const rewardQAccount = await registry.account.rewardQueue.fetch(
      rewardQ.publicKey
    );
    assert.strictEqual(rewardQAccount.head, 2);
    assert.strictEqual(rewardQAccount.tail, 0);
    const e = rewardQAccount.events[1];
    assert.isTrue(e.vendor.equals(lockedVendor.publicKey));
    assert.isTrue(e.locked);
  });

  let vendoredVesting = null;
  let vendoredVestingVault = null;
  let vendoredVestingSigner = null;

  it("Claims a locked reward", async () => {
    vendoredVesting = anchor.web3.Keypair.generate();
    vendoredVestingVault = anchor.web3.Keypair.generate();
    let [_vendoredVestingSigner, nonce] =
      await anchor.web3.PublicKey.findProgramAddress(
        [vendoredVesting.publicKey.toBuffer()],
        lockup.programId
      );
    vendoredVestingSigner = _vendoredVestingSigner;
    const remainingAccounts = lockup.instruction.createVesting
      .accounts({
        vesting: vendoredVesting.publicKey,
        vault: vendoredVestingVault.publicKey,
        depositor: lockedVendorVault.publicKey,
        depositorAuthority: lockedVendorSigner,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
      })
      // Change the signer status on the vendor signer since it's signed by the program, not the
      // client.
      .map((meta) =>
        meta.pubkey === lockedVendorSigner ? { ...meta, isSigner: false } : meta
      );

    await registry.rpc.claimRewardLocked(nonce, {
      accounts: {
        registry: await registry.state.address(),
        lockupProgram: lockup.programId,
        cmn: {
          registrar: registrar.publicKey,

          member: member.publicKey,
          beneficiary: provider.wallet.publicKey,
          balances,
          balancesLocked,

          vendor: lockedVendor.publicKey,
          vault: lockedVendorVault.publicKey,
          vendorSigner: lockedVendorSigner,

          tokenProgram: TOKEN_PROGRAM_ID,
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        },
      },
      remainingAccounts,
      signers: [vendoredVesting, vendoredVestingVault],
      instructions: [
        await lockup.account.vesting.createInstruction(vendoredVesting),
        ...(await serumCmn.createTokenAccountInstrs(
          provider,
          vendoredVestingVault.publicKey,
          mint,
          vendoredVestingSigner
        )),
      ],
    });

    const lockupAccount = await lockup.account.vesting.fetch(
      vendoredVesting.publicKey
    );

    assert.isTrue(lockupAccount.beneficiary.equals(provider.wallet.publicKey));
    assert.isTrue(lockupAccount.mint.equals(mint));
    assert.isTrue(lockupAccount.vault.equals(vendoredVestingVault.publicKey));
    assert.isTrue(lockupAccount.outstanding.eq(lockedRewardAmount));
    assert.isTrue(lockupAccount.startBalance.eq(lockedRewardAmount));
    assert.isTrue(lockupAccount.endTs.eq(lockedRewardKind.locked.endTs));
    assert.isTrue(
      lockupAccount.periodCount.eq(lockedRewardKind.locked.periodCount)
    );
    assert.isTrue(lockupAccount.whitelistOwned.eq(new anchor.BN(0)));
    assert.isTrue(lockupAccount.realizor.program.equals(registry.programId));
    assert.isTrue(lockupAccount.realizor.metadata.equals(member.publicKey));
  });

  it("Waits for the lockup period to pass", async () => {
    await serumCmn.sleep(10 * 1000);
  });

  it("Should fail to unlock an unrealized lockup reward", async () => {
    const token = await serumCmn.createTokenAccount(
      provider,
      mint,
      provider.wallet.publicKey
    );
    await nativeAssert.rejects(
      async () => {
        const withdrawAmount = new anchor.BN(10);
        await lockup.rpc.withdraw(withdrawAmount, {
          accounts: {
            vesting: vendoredVesting.publicKey,
            beneficiary: provider.wallet.publicKey,
            token,
            vault: vendoredVestingVault.publicKey,
            vestingSigner: vendoredVestingSigner,
            tokenProgram: TOKEN_PROGRAM_ID,
            clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
          },
          // TODO: trait methods generated on the client. Until then, we need to manually
          //       specify the account metas here.
          remainingAccounts: [
            { pubkey: registry.programId, isWritable: false, isSigner: false },
            { pubkey: member.publicKey, isWritable: false, isSigner: false },
            { pubkey: balances.spt, isWritable: false, isSigner: false },
            { pubkey: balancesLocked.spt, isWritable: false, isSigner: false },
          ],
        });
      },
      (err) => {
        // Solana doesn't propagate errors across CPI. So we receive the registry's error code,
        // not the lockup's.
        assert.strictEqual(err.error.errorCode.number, 6020);
        assert.strictEqual(err.error.errorCode.code, "UnrealizedReward");
        assert.strictEqual(
          err.error.errorMessage,
          "Locked rewards cannot be realized until one unstaked all tokens."
        );
        expect(err.error.origin).to.deep.equal({
          file: "programs/registry/src/lib.rs",
          line: 63,
        });
        assert.strictEqual(
          err.program.toString(),
          "HmbTLCmaGvZhKnn1Zfa1JVnp7vkMV4DYVxPLWBVoN65L"
        );
        expect(err.programErrorStack.map((pk) => pk.toString())).to.deep.equal([
          "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS",
          "HmbTLCmaGvZhKnn1Zfa1JVnp7vkMV4DYVxPLWBVoN65L",
        ]);
        return true;
      }
    );
  });

  const pendingWithdrawal = anchor.web3.Keypair.generate();

  it("Unstakes (unlocked)", async () => {
    const unstakeAmount = new anchor.BN(10);

    await registry.rpc.startUnstake(unstakeAmount, false, {
      accounts: {
        registrar: registrar.publicKey,
        rewardEventQ: rewardQ.publicKey,
        poolMint,

        pendingWithdrawal: pendingWithdrawal.publicKey,
        member: member.publicKey,
        beneficiary: provider.wallet.publicKey,
        balances,
        balancesLocked,

        memberSigner,

        tokenProgram: TOKEN_PROGRAM_ID,
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
      signers: [pendingWithdrawal],
      instructions: [
        await registry.account.pendingWithdrawal.createInstruction(
          pendingWithdrawal
        ),
      ],
    });

    const vaultPw = await serumCmn.getTokenAccount(
      provider,
      memberAccount.balances.vaultPw
    );
    const vaultStake = await serumCmn.getTokenAccount(
      provider,
      memberAccount.balances.vaultStake
    );
    const spt = await serumCmn.getTokenAccount(
      provider,
      memberAccount.balances.spt
    );

    assert.isTrue(vaultPw.amount.eq(new anchor.BN(20)));
    assert.isTrue(vaultStake.amount.eq(new anchor.BN(0)));
    assert.isTrue(spt.amount.eq(new anchor.BN(0)));
  });

  const tryEndUnstake = async () => {
    await registry.rpc.endUnstake({
      accounts: {
        registrar: registrar.publicKey,

        member: member.publicKey,
        beneficiary: provider.wallet.publicKey,
        pendingWithdrawal: pendingWithdrawal.publicKey,

        vault: balances.vault,
        vaultPw: balances.vaultPw,

        memberSigner,

        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
    });
  };

  it("Fails to end unstaking before timelock", async () => {
    await nativeAssert.rejects(
      async () => {
        await tryEndUnstake();
      },
      (err) => {
        assert.strictEqual(err.error.errorCode.number, 6009);
        assert.strictEqual(
          err.error.errorMessage,
          "The unstake timelock has not yet expired."
        );
        return true;
      }
    );
  });

  it("Waits for the unstake period to end", async () => {
    await serumCmn.sleep(5000);
  });

  it("Unstake finalizes (unlocked)", async () => {
    await tryEndUnstake();

    const vault = await serumCmn.getTokenAccount(
      provider,
      memberAccount.balances.vault
    );
    const vaultPw = await serumCmn.getTokenAccount(
      provider,
      memberAccount.balances.vaultPw
    );

    assert.isTrue(vault.amount.eq(new anchor.BN(120)));
    assert.isTrue(vaultPw.amount.eq(new anchor.BN(0)));
  });

  it("Withdraws deposits (unlocked)", async () => {
    const token = await serumCmn.createTokenAccount(
      provider,
      mint,
      provider.wallet.publicKey
    );
    const withdrawAmount = new anchor.BN(100);
    await registry.rpc.withdraw(withdrawAmount, {
      accounts: {
        registrar: registrar.publicKey,
        member: member.publicKey,
        beneficiary: provider.wallet.publicKey,
        vault: memberAccount.balances.vault,
        memberSigner,
        depositor: token,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
    });

    const tokenAccount = await serumCmn.getTokenAccount(provider, token);
    assert.isTrue(tokenAccount.amount.eq(withdrawAmount));
  });

  it("Should successfully unlock a locked reward after unstaking", async () => {
    const token = await serumCmn.createTokenAccount(
      provider,
      mint,
      provider.wallet.publicKey
    );

    const withdrawAmount = new anchor.BN(7);
    await lockup.rpc.withdraw(withdrawAmount, {
      accounts: {
        vesting: vendoredVesting.publicKey,
        beneficiary: provider.wallet.publicKey,
        token,
        vault: vendoredVestingVault.publicKey,
        vestingSigner: vendoredVestingSigner,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
      },
      // TODO: trait methods generated on the client. Until then, we need to manually
      //       specify the account metas here.
      remainingAccounts: [
        { pubkey: registry.programId, isWritable: false, isSigner: false },
        { pubkey: member.publicKey, isWritable: false, isSigner: false },
        { pubkey: balances.spt, isWritable: false, isSigner: false },
        { pubkey: balancesLocked.spt, isWritable: false, isSigner: false },
      ],
    });
    const tokenAccount = await serumCmn.getTokenAccount(provider, token);
    assert.isTrue(tokenAccount.amount.eq(withdrawAmount));
  });
		*/
});
