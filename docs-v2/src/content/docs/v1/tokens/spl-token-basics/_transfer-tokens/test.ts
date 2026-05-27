import * as anchor from '@anchor-lang/core'
import { Program } from '@anchor-lang/core'
import { TokenExample } from '../target/types/token_example'
import {
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddress,
  getMint,
  getAccount,
} from '@solana/spl-token'

describe('token-example', () => {
  anchor.setProvider(anchor.AnchorProvider.env())

  const program = anchor.workspace.TokenExample as Program<TokenExample>
  const [mint, mintBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from('mint')],
    program.programId,
  )

  const [token, tokenBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from('token')],
    program.programId,
  )

  it('Is initialized!', async () => {
    const tx = await program.methods
      .createAndMintTokens(new anchor.BN(100))
      .accounts({
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .rpc({ commitment: 'confirmed' })
    console.log('Your transaction signature', tx)

    const mintAccount = await getMint(
      program.provider.connection,
      mint,
      'confirmed',
      TOKEN_2022_PROGRAM_ID,
    )

    console.log('Mint Account', mintAccount)
  })

  it('Transfer Tokens', async () => {
    const tx = await program.methods
      .transferTokens()
      .accounts({
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .rpc({ commitment: 'confirmed' })

    console.log('Your transaction signature', tx)

    const associatedTokenAccount = await getAssociatedTokenAddress(
      mint,
      program.provider.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID,
    )

    const recipientTokenAccount = await getAccount(
      program.provider.connection,
      associatedTokenAccount,
      'confirmed',
      TOKEN_2022_PROGRAM_ID,
    )

    const senderTokenAccount = await getAccount(
      program.provider.connection,
      token,
      'confirmed',
      TOKEN_2022_PROGRAM_ID,
    )

    console.log('Recipient Token Account', recipientTokenAccount)
    console.log('Sender Token Account', senderTokenAccount)
  })
})
