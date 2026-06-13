import * as anchor from '@anchor-lang/core'
import { Program } from '@anchor-lang/core'
import { CustomError } from '../target/types/custom_error'
import assert from 'assert'

describe('custom-error', () => {
  anchor.setProvider(anchor.AnchorProvider.env())
  const program = anchor.workspace.customError as Program<CustomError>

  it('Successfully validates amount within range', async () => {
    const tx = await program.methods.validateAmount(new anchor.BN(50)).rpc()

    console.log('Transaction signature:', tx)
  })

  it('Fails with amount too small', async () => {
    try {
      await program.methods.validateAmount(new anchor.BN(5)).rpc()

      assert.fail('Expected an error to be thrown')
    } catch (error) {
      assert.strictEqual(error.error.errorCode.code, 'AmountTooSmall')
      assert.strictEqual(error.error.errorMessage, 'Amount must be greater than or equal to 10')
    }
  })

  it('Fails with amount too large', async () => {
    try {
      await program.methods.validateAmount(new anchor.BN(150)).rpc()

      assert.fail('Expected an error to be thrown')
    } catch (error) {
      assert.strictEqual(error.error.errorCode.code, 'AmountTooLarge')
      assert.strictEqual(error.error.errorMessage, 'Amount must be less than or equal to 100')
    }
  })
})
