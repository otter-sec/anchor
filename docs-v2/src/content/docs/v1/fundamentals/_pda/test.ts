import * as anchor from '@anchor-lang/core'
import { Program } from '@anchor-lang/core'
import { HelloAnchor } from '../target/types/hello_anchor'

describe('hello_anchor', () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env())

  const program = anchor.workspace.HelloAnchor as Program<HelloAnchor>

  it('Is initialized!', async () => {
    // Account address is automatically resolved using the IDL
    const tx = await program.methods.testInstruction().rpc()
    console.log('Your transaction signature', tx)
  })
})
