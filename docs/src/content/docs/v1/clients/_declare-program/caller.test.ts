import * as anchor from '@anchor-lang/core'
import { Program } from '@anchor-lang/core'
import { Example } from '../target/types/example'
import { ExampleCpi } from '../target/types/example_cpi'
import { Keypair } from '@solana/web3.js'

describe('example', () => {
  anchor.setProvider(anchor.AnchorProvider.env())

  const program = anchor.workspace.example as Program<Example>
  const cpiProgram = anchor.workspace.exampleCpi as Program<ExampleCpi>

  const counterAccount = Keypair.generate()

  it('Is initialized!', async () => {
    const transactionSignature = await cpiProgram.methods
      .initializeCpi()
      .accounts({
        counter: counterAccount.publicKey,
      })
      .signers([counterAccount])
      .rpc({ skipPreflight: true })

    const accountData = await program.account.counter.fetch(counterAccount.publicKey)

    console.log(`Transaction Signature: ${transactionSignature}`)
    console.log(`Count: ${accountData.count}`)
  })

  it('Increment', async () => {
    const transactionSignature = await cpiProgram.methods
      .incrementCpi()
      .accounts({
        counter: counterAccount.publicKey,
      })
      .rpc()

    const accountData = await program.account.counter.fetch(counterAccount.publicKey)

    console.log(`Transaction Signature: ${transactionSignature}`)
    console.log(`Count: ${accountData.count}`)
  })
})
