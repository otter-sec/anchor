// Tests are for whether we are able to call native Solana programs and fetch
// program state through Anchor client and not for whether internal program logic
// works as intended. For the latter case, check out the program's repository.
//
// Tests are for the latest versions of the programs as of 2022-08-20, in order for
// tests to work, make sure the programs are up-to date in your local validator.

import * as SPL from "./spl";
import { mainTest, programTest } from "./utils";

// You can remove `await`s if you want to run tests in parallel(faster).
mainTest(async () => {
  await programTest(SPL.associatedTokenAccountTests);
  await programTest(SPL.binaryOptionTests);
  await programTest(SPL.binaryOraclePairTests);
  await programTest(SPL.featureProposalTests);
  await programTest(SPL.memoTests);
  await programTest(SPL.nameServiceTests);
  await programTest(SPL.recordTests);
  await programTest(SPL.splTokenTests);
  await programTest(SPL.stakePoolTests);
  await programTest(SPL.statelessAsksTests);
  await programTest(SPL.tokenSwapTests);
});
