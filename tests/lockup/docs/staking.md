# Staking

WARNING: All code related to staking is unaudited. Use at your own risk.

## Introduction

The **Registry** program provides an on-chain mechanism for a group of stakers to

* Share rewards proportionally amongst a staking pool
* Govern on chain protocols with stake weighted voting
* Stake and earn locked tokens

The program makes little assumptions about the form of stake or rewards.
In the same way you can make a new SPL token with its own mint, you can create a new stake
pool. Although the token being staked  must be a predefined mint upon pool initialization,
rewards on a particular pool can be arbitrary SPL tokens, or, in the case of locked rewards,
program controlled accounts.
Rewards can come from an arbitrary
wallet, e.g. automatically from a fee earning program,
or manually from a wallet owned by an individual. The specifics are token and protocol
dependent.

Similarly, the specifics of governance are not assumed by the staking program. However, a
governance system can use this program as a primitive to implement stake weighted voting.

Here staking is covered at somewhat of a low level with the goal of allowing one
to understand, contribute to, or modify the code.

## Accounts

Accounts are the pieces of state owned by a Solana program. For reference while reading, here are all
accounts used by the **Registry** program.

* `Registrar` - Analogous to an SPL token `Mint`, the `Registrar` defines a staking instance. It has its own pool, and its own set of rewards distributed amongst its own set of stakers.
* `Member` - Analogous to an SPL token `Account`, `Member` accounts represent a **beneficiary**'s (i.e. a wallet's) stake state. This account has several vaults, all of which represent the funds belonging to an individual user.
* `PendingWithdrawal` - A transfer out of the staking pool (poorly named since it's not a withdrawal out of the program. But a withdrawal out of the staking pool and into a `Member`'s freely available balances).
* `RewardVendor` - A reward that has been dropped onto stakers and is distributed pro rata to staked `Member` beneficiaries.
* `RewardEventQueue` - A ring buffer of all rewards available to stakers. Each entry is the address of a `RewardVendor`.

## Creating a member account.

Before being able to enter the stake pool, one must create a **Member** account with the
**Registrar**, providing identity to the **Registry** program. By default, each member has
four types of token vaults making up a set of balances owned by the program on behalf of a
**Member**:

* Available balances: a zero-interest earning token account with no restrictions.
* Pending: unstaked funds incurring an unbonding timelock.
* Stake: the total amount of tokens staked.
* Stake pool token: the total amount of pool tokens created from staking (`stake = stake-pool-token * stake-pool-token-price`).

Each of these vaults provide a unit of balance isolation unique to a **Member**.
That is, although the stake program appears to provide a pooling mechanism, funds between
**Member** accounts are not commingled. They do not share SPL token accounts, and the only
way for funds to move is for  a **Member**'s beneficiary to authorize instructions that either exit the
system or move funds between a **Member**'s own vaults.

## Depositing and Withdrawing.

Funds initially enter and exit the program through the `Deposit` and `Withdraw` instructions,
which transfer funds into and out of the **available balances** vault.
As the name suggests, all funds in this vault are freely available, unrestricted, and
earn zero interest. The vault is purely a gateway for funds to enter the program.

## Staking.

Once deposited, a **Member** beneficiary invokes the `Stake` instruction to transfer funds from
their **available-balances-vault** to one's **stake-vault**, creating newly minted
**stake-pool-tokens** as proof of the stake deposit. These new tokens represent
one's proportional right to all rewards distributed to the staking pool and are offered
by the **Registry** program at a fixed price, e.g., of 500 SPL tokens.

## Unstaking

Once staked, funds cannot be immediately withdrawn. Rather, the **Registrar** will enforce
a one week timelock before funds are released. Upon executing the `StartUnstake`
instruction, three operations execute. 1) The given amount of stake pool tokens will be burned.
2) Staked funds proportional to the stake pool tokens burned will be transferred from the
**Member**'s **stake-vault** to the **Member**'s **pending-vault**. 3) A `PendingWithdrawal`
account will be created as proof of the stake withdrawal, stamping the current block's
`unix_timestamp` onto the account. When the timelock period ends, a **Member** can invoke the
`EndUnstake` instruction to complete the transfer out of the `pending-vault` and
into the `available-balances`, providing the previously printed `PendingWithdrawal`
receipt to the program as proof that the timelock has passed. At this point, the exit
from the stake pool is complete, and the funds are ready to be used again.

## Reward Design Motivation

Feel free to skip this section and jump to the **Reward Vendors** section if you want to
just see how rewards work.

One could imagine several ways to drop rewards onto a staking pool, each with their own downsides.
Of course what you want is, for a given reward amount, to atomically snapshot the state
of the staking pool and to distribute it proportionally to all stake holders. Effectively,
an on chain program such as

```python
for account in stake_pool:
  account.token_amount += total_reward * (account.stake_pool_token.amount / stake_pool_token.supply)
 ```

Surprisingly, such a mechanism is not immediately obvious.

First, the above program is a non starter. Not only does the SPL token
program not have the ability to iterate through all accounts for a given mint within a program,
but, since Solana transactions require the specification of all accounts being accessed
in a transaction (this is how it achieves parallelism), such a transaction's size would be
well over the limit. So modifying global state atomically in a single transaction is out of the
question.

So if you can't do this on chain, one can try doing it off chain. One could write an program to
snapshot the pool state, and just airdrop tokens onto the pool. This works, but
adds an additional layer of trust. Who snapshots the pool state? At what time?
How do you know they calculated the rewards correctly? What happens if my reward was not given?
This is not auditable or verifiable. And if you want to answer these questions, requires
complex off-chain protocols that require either fancy cryptography or effectively
recreating a BFT system off chain.

Another solution considered was to use a uniswap-style AMM pool (without the swapping).
This has a lot of advantages. First it's easy to reason about and implement in a single transaction.
To drop rewards globally onto the pool, one can deposit funds directly into the pool, in which case
the reward is automatically received by owners of the staking pool token upon redemption, a process
known as "gulping"--since dropping rewards increases the total value of the pool
while their proportion of the pool remained constant.

However, there are enough downsides with using an AMM style pool to offset the convenience.
Unfortunately, it loses the nice balance isolation property **Member** accounts have, because
tokens have to be pooled into the same vault, which is an additional security concern that could
easily lead to loss of funds, e.g., if there's a bug in the redemption calculation. Moreover, dropping
arbitrary tokens onto the pool is a challenge. Not only do you have to create new pool vaults for
every new token you drop onto the pool, but you also need to have stakers purchase those tokens to enter
the pool, effectively requiring one to stake other unintended tokens. An additional oddity is that
as rewards are dropped onto the pool, the price to enter the pool monotonically increases. Remember, entering this
type of pool requires "creating" pool tokens, i.e., depositing enough tokens so that you don't dilute
any other member. So if a single pool token represents one SPL token. And if an additional SPL token is dropped onto every
member of the pool, all the existing member's shares are now worth two SPL tokens. So to enter the pool without
dilution, one would have to "create" at a price of 2 SPL tokens per share. This means that rewarding
stakers becomes more expensive over time. One could of course solve this problem by implementing
arbitrary `n:m` pool token splits, which leads right back to the problem of mutating global account
state for an SPL token.

Furthermore, dropping arbitrary program accounts as rewards hasn't even been covered, for example,
locked token rewards, which of course can't be dropped directly onto an AMM style pool, since they are not tokens.
So, if one did go with an AMM style pool, one would need a separate mechanism for handling more general rewards like
locked token accounts. Ideally, there would be a single mechanism for both.

## Reward Vendors

Instead of trying to *push* rewards to users via a direct transfer or airdrop, one can use a *polling* model
where users effectively event source a log on demand, providing a proof one is eligible for the reward.

When a reward is created, the program must do two things:

1) Create a **Reward Vendor** account with an associated token vault holding the reward.
2) Assign the **Reward Vendor** the next available position in a **Reward Event Queue**. Then, to retrieve
a reward, a staker invokes the `ClaimReward` command, providing a proof that the funds were
staked at the time of the reward being dropped, and in response, the program transfers or,
some might say, *vends* the proportion of the dropped reward to the polling **Member**. The
operation completes by incrementing the **Member**'s queue cursor, ensuring that a given
reward can only be processed once.

This allows the program to drop rewards on the stake pool in a way that is
on chain and verifiable. Of course, it requires an external trigger, some account willing to
transfer funds to a new **RewardVendor**, but that is outside of the scope of the staking
program. The reward dropper can be an off chain BFT committee, or it can be an on-chain multisig.
It can be a charitable individual, or funds can flow directly from a fee paying program such as the DEX,
which itself can create a Reward Vendor from fees collected. It doesn't matter to the **Registry** program.

Note that this solution also allows for rewards to be denominated in any token, not just the token being staked.
Since rewards are paid out by the vendor immediately and to a token account of the **Member**'s
choosing, it *just works*. Even more, this extends to arbitrary program accounts, particularly
**Locked** tokens. A **Reward Vendor** needs to additionally know the accounts and instruction data
to relay to the program, but otherwise, the mechanism is the same. The details of **Locked** tokens will
be explained in an additional document.

### Realizing Locked Rewards

In addition to a vesting schedule, locked rewards are subject to a realization condition defined by the
staking program. Specifically, locked tokens are **realized** upon completely unstaking. So if one never
unstakes and incurs the unbonding timelock, one never receives locked token rewards.

## Misc

### Member Accounts

This document describes 4 vault types belonging to **Member** accounts.
However there are two types of balance groups: locked and unlocked.
As a result, there are really 8 vaults for each **Member**, 4 types of vaults in 2 separate sets,
each isolated from the other, so that locked tokens don't get mixed with unlocked tokens.

## Future Work

* Arbitrary program accounts as rewards. With the current design, it should be straightforward to generalize locked token rewards to arbitrary program accounts from arbitrary programs.
