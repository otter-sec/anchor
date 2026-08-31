import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  Transaction,
  sendAndConfirmTransaction,
} from '@solana/web3.js'
import { Program } from '@anchor-lang/core'
import type { Example } from './idl/example.ts'
import idl from './idl/example.json'

// Set up a connection to the cluster
const connection = new Connection('http://127.0.0.1:8899', 'confirmed')

// Create a Program instance using the IDL and connection
const program = new Program(idl as Example, {
  connection,
})

// Generate new Keypairs for the payer and the counter account
const payer = Keypair.generate()
const counter = Keypair.generate()

// Airdrop SOL to fund the payer's account for transaction fees
const airdropTransactionSignature = await connection.requestAirdrop(
  payer.publicKey,
  LAMPORTS_PER_SOL,
)
await connection.confirmTransaction(airdropTransactionSignature)

// Build the initialize instruction
const initializeInstruction = await program.methods
  .initialize()
  .accounts({
    payer: payer.publicKey,
    counter: counter.publicKey,
  })
  .instruction()

// Build the increment instruction
const incrementInstruction = await program.methods
  .increment()
  .accounts({
    counter: counter.publicKey,
  })
  .instruction()

// Add both instructions to a single transaction
const transaction = new Transaction().add(initializeInstruction, incrementInstruction)

// Send the transaction
const transactionSignature = await sendAndConfirmTransaction(connection, transaction, [
  payer,
  counter,
])
console.log('Transaction Signature', transactionSignature)

// Fetch the counter account
const counterAccount = await program.account.counter.fetch(counter.publicKey)
console.log('Count:', counterAccount.count)
