# Lockups

WARNING: All code related to Lockups is unaudited. Use at your own risk.

## Introduction

The **Lockup** program provides a simple mechanism to lockup tokens
of any mint, and release those funds over time as defined by a vesting schedule.
Although these lockups track a target **beneficiary**, who will eventually receive the
funds upon vesting, a proper deployment of the program will ensure this **beneficiary**
can never actually retrieve tokens before vesting. Funds are *never* in an SPL
token wallet owned by a user, and are completely program controlled.

## Accounts

There is a single account type used by the program.

* `Vesting` - An account defining a vesting schedule, realization condition, and vault holding the tokens to be released over time.

## Creating a Vesting Account

Lockup occurs when tokens are transferred into the program creating a **Vesting**
account on behalf of a **beneficiary** via the `CreateVesting` instruction.
There are three parameters to specify:

* Start timestamp - unix timestamp (in seconds) of the time when vesting begins.
* End timestamp - unix timestamp (in seconds) of the time when all tokens will unlock.
* Period count - the amount of times vesting should occur.
* Deposit amount - the total amount to vest.
* Realizer - the program defining if and when vested tokens can be distributed to a beneficiary.

Together these parameters form a linearly unlocked vesting schedule. For example,
if one wanted to lock 100 SPL tokens that unlocked twice, 50 each time, over the next year, one
would use the following parameters (in JavaScript).

```javascript
const startTimestamp = Date.now()/1000;
const endTimestamp = Date.now()/1000 + 60*60*24*365;
const periodCount = 2;
const depositAmount = 100 * 10**6; // 6 decimal places.
const realizer = null; // No realizer in this example.
```

From these five parameters, one can deduce the total amount vested at any given time.

Once created, a **Vesting** account's schedule cannot be mutated.

## Withdrawing from a Vesting Account

Withdrawing is straightforward. Simply invoke the `Withdraw` instruction, specifying an
amount to withdraw from a **Vesting** account. The **beneficiary** of the
**Vesting** account must sign the transaction, but if enough time has passed for an
amount to be vested, and, if the funds are indeed held in the lockup program's vault
(a point mentioned below) then the program will release the funds.

## Realizing Locked Tokens

Optionally, vesting accounts can be created with a `realizer` program, which is
a program implementing the lockup program's `RealizeLock` trait. In
addition to the vesting schedule, a `realizer` program determines if and when a
beneficiary can ever seize control over locked funds. It's effectively a function
returning a boolean: is realized or not.

The uses cases for a realizer are application specific.
For example, in the case of the staking program, when a vesting account is distributed as a reward,
the staking program sets itself as the realizor, ensuring that the only way for the vesting account
to be realized is if the beneficiary completely unstakes and incurs the unbonding timelock alongside
any other consequences of unstaking (e.g., the inability to vote on governance proposals).
This implies that, if one never unstakes, one never receives locked token rewards, adding
an additional consideration when managing one's stake.

If no such `realizer` exists, tokens are realized upon account creation.

## Whitelisted Programs

Although funds cannot be freely withdrawn prior to vesting, they can be sent to/from
other programs that are part of a **Whitelist**. These programs are completely trusted.
Any bug or flaw in the design of a whitelisted program can lead to locked tokens being released
ahead of schedule, so it's important to take great care when whitelisting any program.

This of course begs the question, who approves the whitelist? The **Lockup** program doesn't
care. There simply exists an **authority** key that can, for example, be a democratic multisig,
a single admin, or the zero address--in which case the authority ceases to exist, as the
program will reject transactions signing from that address. Although the **authority** can never
move a **Vesting** account's funds, whoever controls the **authority** key
controls the whitelist. So when using the **Lockup** program, one should always be
cognizant of its whitelist governance, which ultimately anchors one's trust in the program,
if any at all.

## Creating a Whitelisted Program

To create a whitelisted program that receives withdrawals/deposits from/to the Lockup program,
one needs to implement the whitelist transfer interface, which assumes nothing about the
`instruction_data` but requires accounts to be provided in a specific order.

Take staking locked tokens as a working example.

### Staking Locked Tokens

Suppose you have a vesting account with some funds you want to stake.

First, one must add the staking **Registry** as a whitelisted program, so that the Lockup program
allows the movement of funds. This is done by the `WhitelistAdd` instruction.

Once whitelisted, **Vesting** accounts can transfer funds out of the **Lockup** program and
into the **Registry** program by invoking the **Lockup** program's `WhitelistWithdraw`
instruction, which, other than access control, simply relays the instruction from the
**Lockup** program to the **Registry** program along with accounts, signing the
Cross-Program-Invocation (CPI) with the **Lockup**'s program-derived-address to allow
the transfer of funds, which ultimately is done by the **Registry**. *It is the Registry's responsibility
to track where these funds came from, keep them locked, and eventually send them back.*

When creating this instruction on the client, there are two parameters to provide:
the maximum `amount` available for transfer and the opaque CPI `instruction_data`.
In the example, here, it would be the Borsh serialized instruction data for the
**Registry**'s `Deposit` instruction.

The other direction follows, similarly. One invokes the `WhitelistDeposit` instruction
on the **LockupProgram**, relaying the transaction to the **Registry**, which ultimately
transfer funds back into the lockup program on behalf of the **Vesting** account.

## Major version upgrades.

Assuming the `authority` account is set on the **Lockup** program, one can use this Whitelist
mechanism to do major version upgrades of the lockup program. One can whitelist the
new **Lockup** program, and then all **Vesting** accounts would individually perform the migration
by transferring their funds to the new program via the `WhitelistWithdraw` instruction.
