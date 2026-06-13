extern crate proc_macro;

use {proc_macro::TokenStream, quote::ToTokens, syn::parse_macro_input};

/// Implements an [`Accounts`](./trait.Accounts.html) deserializer on the given
/// struct. Can provide further functionality through the use of attributes.
///
/// # Table of Contents
/// - [Instruction Attribute](#instruction-attribute)
/// - [Constraints](#constraints)
///
/// # Instruction Attribute
///
/// You can access the instruction's arguments with the
/// `#[instruction(..)]` attribute. You have to list them
/// in the same order as in the instruction but you can
/// omit all arguments after the last one you need.
///
/// # Example
///
/// ```ignore
/// ...
/// pub fn initialize(ctx: Context<Create>, bump: u8, authority: Pubkey, data: u64) -> anchor_lang::Result<()> {
///     ...
///     Ok(())
/// }
/// ...
/// #[derive(Accounts)]
/// #[instruction(bump: u8)]
/// pub struct Initialize<'info> {
///     ...
/// }
/// ```
///
/// # Constraints
///
/// There are different types of constraints that can be applied with the `#[account(..)]` attribute.
///
/// Attributes may reference other data structures. When `<expr>` is used in the tables below, an arbitrary expression
/// may be passed in as long as it evaluates to a value of the expected type, e.g. `owner = token_program.key()`. If `target_account`
/// used, the `target_account` must exist in the struct and the `.key()` is implicit, e.g. `payer = authority`.
///
/// - [Normal Constraints](#normal-constraints)
/// - [SPL Constraints](#spl-constraints)
///
/// # Normal Constraints
/// <table>
///     <thead>
///         <tr>
///             <th>Attribute</th>
///             <th>Description</th>
///         </tr>
///     </thead>
///     <tbody>
///         <tr>
///             <td>
///                 <code>#[account(signer)]</code> <br><br><code>#[account(signer @ &lt;custom_error&gt;)]</code>
///             </td>
///             <td>
///                 Checks the given account signed the transaction.<br>
///                 Custom errors are supported via <code>@</code>.<br>
///                 Consider using the <code>Signer</code> type if you would only have this constraint on the account.<br><br>
///                 Example:
///                 <pre><code>
/// #[account(signer)]
/// pub authority: UncheckedAccount&lt;'info&gt;,
/// #[account(signer @ MyError::MyErrorCode)]
/// pub payer: UncheckedAccount&lt;'info&gt;
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(mut)]</code> <br><br><code>#[account(mut @ &lt;custom_error&gt;)]</code>
///             </td>
///             <td>
///                 Checks the given account is mutable.<br>
///                 Makes anchor persist any state changes.<br>
///                 Custom errors are supported via <code>@</code>.<br><br>
///                 Example:
///                 <pre><code>
/// #[account(mut)]
/// pub data_account: Account&lt;'info, MyData&gt;,
/// #[account(mut @ MyError::MyErrorCode)]
/// pub data_account_two: Account&lt;'info, MyData&gt;
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(dup)]</code> <br><br>
///             </td>
///             <td>
///                 Allows the same mutable account to be passed multiple times within the same instruction context.<br>
///                 By default, Anchor will prevents duplicate mutable accounts to avoid potential security issues and unintended behavior.<br>
///                 The <code>dup</code> constraint explicitly allows this for cases where it's intentional and safe.<br>
///                 This constraint only applies to mutable account (`mut`) types that serialize on exit. Other types like <br>
///                `UncheckedAccount`, `Signer`, `SystemAccount`, `AccountLoader`, `Program`, `Interface` and Readonly accounts <br>
///                 naturally allow duplicates as they don't serialize data on exit.<br>
///                 Example:
///                 <pre><code>
/// #[account(mut)]
/// pub account1: Account&lt;'info, Counter&gt;,
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(init, payer = &lt;target_account&gt;, space = &lt;num_bytes&gt;)]</code>
///             </td>
///             <td>
///                 Creates the account via a CPI to the system program and
///                 initializes it (sets its account discriminator). The annotated account is required to sign for this instruction
///                 unless `seeds` is provided. <br>
///                 Marks the account as mutable and is mutually exclusive with <code>mut</code>.<br>
///                 Makes the account rent exempt unless skipped with <code>rent_exempt = skip</code>.<br><br>
///                 Use <code>#[account(zero)]</code> for accounts larger than 10 Kibibyte.<br><br>
///                 <code>init</code> has to be used with additional constraints:
///                 <ul>
///                     <li>
///                         Requires the <code>payer</code> constraint to also be on the account.
///                         The <code>payer</code> account pays for the
///                         account creation.
///                     </li>
///                     <li>
///                         Requires the system program to exist on the struct
///                         and be called <code>system_program</code>.
///                     </li>
///                     <li>
///                         Requires that the <code>space</code> constraint is specified.
///                         When using the <code>space</code> constraint, one must remember to add 8 to it
///                         which is the size of the account discriminator. This only has to be done
///                         for accounts owned by anchor programs.<br>
///                         The given space number is the size of the account in bytes, so accounts that hold
///                         a variable number of items such as a <code>Vec</code> should allocate sufficient space for all items that may
///                         be added to the data structure because account size is fixed.
///                         Check out the <a href = "https://www.anchor-lang.com/docs/space" target = "_blank" rel = "noopener noreferrer">space reference</a>
///                         and the <a href = "https://borsh.io/" target = "_blank" rel = "noopener noreferrer">borsh library</a>
///                         (which anchor uses under the hood for serialization) specification to learn how much
///                         space different data structures require.
///                     </li>
///                 <br>
///                 Example:
///                 <pre>
/// #[account]
/// pub struct MyData {
/// &nbsp;&nbsp;&nbsp;&nbsp;pub data: u64
/// }&#10;
/// #[derive(Accounts)]
/// pub struct Initialize&lt;'info&gt; {
/// &nbsp;&nbsp;&nbsp;&nbsp;#[account(init, payer = payer, space = 8 + 8)]
/// &nbsp;&nbsp;&nbsp;&nbsp;pub data_account_two: Account&lt;'info, MyData&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;#[account(mut)]
/// &nbsp;&nbsp;&nbsp;&nbsp;pub payer: Signer&lt;'info&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;pub system_program: Program&lt;'info, System&gt;,
/// }
///                 </pre>
///                 </ul>
///                 <code>init</code> can be combined with other constraints (at the same time):
///                 <ul>
///                     <li>
///                         By default <code>init</code> sets the owner field of the created account to the
///                         currently executing program. Add the <code>owner</code> constraint to specify a
///                         different program owner.
///                     </li>
///                     <li>
///                         Use the <code>seeds</code> constraint together with <code>bump</code>to create PDAs.<br>
///                         <code>init</code> uses <code>find_program_address</code> to calculate the pda so the
///                         bump value can be left empty.<br>
///                         However, if you want to use the bump in your instruction,
///                         you can pass it in as instruction data and set the bump value like shown in the example,
///                         using the <code>instruction_data</code> attribute.
///                         Anchor will then check that the bump returned by <code>find_program_address</code> equals
///                         the bump in the instruction data.<br>
///                         <code>seeds::program</code> cannot be used together with init because the creation of an
///                         account requires its signature which for PDAs only the currently executing program can provide.
///                     </li>
///                 </ul>
///                 Example:
///                 <pre>
/// #[derive(Accounts)]
/// #[instruction(bump: u8)]
/// pub struct Initialize&lt;'info&gt; {
/// &nbsp;&nbsp;&nbsp;&nbsp;#[account(
/// &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;init, payer = payer, space = 8 + 8
/// &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;seeds = [b"example_seed"], bump = bump
/// &nbsp;&nbsp;&nbsp;&nbsp;)]
/// &nbsp;&nbsp;&nbsp;&nbsp;pub pda_data_account: Account&lt;'info, MyData&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;#[account(
/// &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;init, payer = payer,
/// &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;space = 8 + 8, owner = other_program.key()
/// &nbsp;&nbsp;&nbsp;&nbsp;)]
/// &nbsp;&nbsp;&nbsp;&nbsp;pub account_for_other_program: UncheckedAccount&lt;'info&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;#[account(
/// &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;init, payer = payer, space = 8 + 8,
/// &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;owner = other_program.key(),
/// &nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;seeds = [b"other_seed"], bump
/// &nbsp;&nbsp;&nbsp;&nbsp;)]
/// &nbsp;&nbsp;&nbsp;&nbsp;pub pda_for_other_program: UncheckedAccount&lt;'info&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;#[account(mut)]
/// &nbsp;&nbsp;&nbsp;&nbsp;pub payer: Signer&lt;'info&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;pub system_program: Program&lt;'info, System&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;pub other_program: Program&lt;'info, OtherProgram&gt;
/// }
///                 </pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(init_if_needed, payer = &lt;target_account&gt;)]</code><br><br>
///                 <code>#[account(init_if_needed, payer = &lt;target_account&gt;, space = &lt;num_bytes&gt;)]</code>
///             </td>
///             <td>
///                 Exact same functionality as the <code>init</code> constraint but only runs if the account does not exist yet.<br>
///                 If the account does exist, it still checks whether the given init constraints are correct,
///                 e.g. that the account has the expected amount of space and, if it's a PDA, the correct seeds etc.<br><br>
///                 This feature should be used with care and is therefore behind a feature flag.
///                 You can enable it by importing <code>anchor-lang</code> with the <code>init-if-needed</code> cargo feature.<br>
///                 When using <code>init_if_needed</code>, you need to make sure you properly protect yourself
///                 against re-initialization attacks. You need to include checks in your code that check
///                 that the initialized account cannot be reset to its initial settings after the first time it was
///                 initialized (unless that it what you want).<br>
///                 Because of the possibility of re-initialization attacks and the general guideline that instructions
///                 should avoid having multiple execution flows (which is important so they remain easy to understand),
///                 consider breaking up your instruction into two instructions - one for initializing and one for using
///                 the account - unless you have a good reason not to do so.
///                 <br><br>
///                 Example:
///                 <pre>
/// #[account]
/// #[derive(Default)]
/// pub struct MyData {
/// &nbsp;&nbsp;&nbsp;&nbsp;pub data: u64
/// }&#10;
/// #[account]
/// pub struct OtherData {
/// &nbsp;&nbsp;&nbsp;&nbsp;pub data: u64
/// }&#10;
/// #[derive(Accounts)]
/// pub struct Initialize&lt;'info&gt; {
/// &nbsp;&nbsp;&nbsp;&nbsp;#[account(init_if_needed, payer = payer)]
/// &nbsp;&nbsp;&nbsp;&nbsp;pub data_account: Account&lt;'info, MyData&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;#[account(init_if_needed, payer = payer, space = 8 + 8)]
/// &nbsp;&nbsp;&nbsp;&nbsp;pub data_account_two: Account&lt;'info, OtherData&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;#[account(mut)]
/// &nbsp;&nbsp;&nbsp;&nbsp;pub payer: Signer&lt;'info&gt;,
/// &nbsp;&nbsp;&nbsp;&nbsp;pub system_program: Program&lt;'info, System&gt;
/// }
///                 </pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(seeds = &lt;seeds&gt;, bump)]</code><br><br>
///                 <code>#[account(seeds = &lt;seeds&gt;, bump, seeds::program = &lt;expr&gt;)]</code><br><br>
///                 <code>#[account(seeds = &lt;seeds&gt;, bump = &lt;expr&gt;)]</code><br><br>
///                 <code>#[account(seeds = &lt;seeds&gt;, bump = &lt;expr&gt;, seeds::program = &lt;expr&gt;)]</code><br><br>
///             </td>
///             <td>
///                 Checks that given account is a PDA derived from the currently executing program,
///                 the seeds, and if provided, the bump. If not provided, anchor uses the canonical
///                 bump. <br>
///                 Add <code>seeds::program = &lt;expr&gt;</code> to derive the PDA from a different
///                 program than the currently executing one.<br>
///                 This constraint behaves slightly differently when used with <code>init</code>.
///                 See its description.
///                 <br><br>
///                 Example:
///                 <pre><code>
/// #[derive(Accounts)]
/// #[instruction(first_bump: u8, second_bump: u8)]
/// pub struct Example {
///     #[account(seeds = [b"example_seed"], bump)]
///     pub canonical_pda: UncheckedAccount&lt;'info&gt;,
///     #[account(
///         seeds = [b"example_seed"],
///         bump,
///         seeds::program = other_program.key()
///     )]
///     pub canonical_pda_two: UncheckedAccount&lt;'info&gt;,
///     #[account(seeds = [b"other_seed"], bump = first_bump)]
///     pub arbitrary_pda: UncheckedAccount&lt;'info&gt;
///     #[account(
///         seeds = [b"other_seed"],
///         bump = second_bump,
///         seeds::program = other_program.key()
///     )]
///     pub arbitrary_pda_two: UncheckedAccount&lt;'info&gt;,
///     pub other_program: Program&lt;'info, OtherProgram&gt;
/// }
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(has_one = &lt;target_account&gt;)]</code><br><br>
///                 <code>#[account(has_one = &lt;target_account&gt; @ &lt;custom_error&gt;)]</code>
///             </td>
///             <td>
///                 Checks the <code>target_account</code> field on the account matches the
///                 key of the <code>target_account</code> field in the Accounts struct.<br>
///                 Custom errors are supported via <code>@</code>.<br><br>
///                 Example:
///                 <pre><code>
/// #[account(mut, has_one = authority)]
/// pub data: Account&lt;'info, MyData&gt;,
/// pub authority: Signer&lt;'info&gt;
///                 </code></pre>
///                 In this example <code>has_one</code> checks that <code>data.authority = authority.key()</code>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(address = &lt;expr&gt;)]</code><br><br>
///                 <code>#[account(address = &lt;expr&gt; @ &lt;custom_error&gt;)]</code>
///             </td>
///             <td>
///                 Checks the account key matches the pubkey.<br>
///                 Custom errors are supported via <code>@</code>.<br><br>
///                 Example:
///                 <pre><code>
/// #[account(address = crate::ID)]
/// pub data: Account&lt;'info, MyData&gt;,
/// #[account(address = crate::ID @ MyError::MyErrorCode)]
/// pub data_two: Account&lt;'info, MyData&gt;
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(owner = &lt;expr&gt;)]</code><br><br>
///                 <code>#[account(owner = &lt;expr&gt; @ &lt;custom_error&gt;)]</code>
///             </td>
///             <td>
///                 Checks the account owner matches <code>expr</code>.<br>
///                 Custom errors are supported via <code>@</code>.<br><br>
///                 Example:
///                 <pre><code>
/// #[account(owner = Token::ID @ MyError::MyErrorCode)]
/// pub data: Account&lt;'info, MyData&gt;,
/// #[account(owner = token_program.key())]
/// pub data_two: Account&lt;'info, MyData&gt;,
/// pub token_program: Program&lt;'info, Token&gt;
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(executable)]</code>
///             </td>
///             <td>
///                 Checks the account is executable (i.e. the account is a program).<br>
///                 You may want to use the <code>Program</code> type instead.<br><br>
///                 Example:
///                 <pre><code>
/// #[account(executable)]
/// pub my_program: AccountInfo&lt;'info&gt;
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(rent_exempt = skip)]</code><br><br>
///                 <code>#[account(rent_exempt = enforce)]</code>
///             </td>
///             <td>
///                 Enforces rent exemption with <code>= enforce</code>.<br>
///                 Skips rent exemption check that would normally be done
///                 through other constraints with <code>= skip</code>,
///                 e.g. when used with the <code>zero</code> constraint<br><br>
///                 Example:
///                 <pre><code>
/// #[account(zero, rent_exempt = skip)]
/// pub skipped_account: Account&lt;'info, MyData&gt;,
/// #[account(rent_exempt = enforce)]
/// pub enforced_account: AccountInfo&lt;'info&gt;
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(zero)]</code>
///             </td>
///             <td>
///                 Checks the account discriminator is zero.<br>
///                 Enforces rent exemption unless skipped with <code>rent_exempt = skip</code>.<br><br>
///                 Use this constraint if you want to create an account in a previous instruction
///                 and then initialize it in your instruction instead of using <code>init</code>.
///                 This is necessary for accounts that are larger than 10 Kibibyte because those
///                 accounts cannot be created via a CPI (which is what <code>init</code> would do).<br><br>
///                 Anchor adds internal data to the account when using <code>zero</code> just like it
///                 does with <code>init</code> which is why <code>zero</code> implies <code>mut</code>.
///                 <br><br>
///                 Example:
///                 <pre><code>
/// #[account(zero)]
/// pub my_account: Account&lt;'info, MyData&gt;
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(close = &lt;target_account&gt;)]</code>
///             </td>
///             <td>
///                 Closes the account by:<br>
///                 &nbsp;&nbsp;&nbsp;&nbsp;- Sending the lamports to the specified account<br>
///                 &nbsp;&nbsp;&nbsp;&nbsp;- Assigning the owner to the System Program<br>
///                 &nbsp;&nbsp;&nbsp;&nbsp;- Resetting the data of the account<br><br>
///                 Requires <code>mut</code> to exist on the account.
///                 <br><br>
///                 Example:
///                 <pre><code>
/// #[account(mut, close = receiver)]
/// pub data_account: Account&lt;'info, MyData&gt;,
/// #[account(mut)]
/// pub receiver: SystemAccount&lt;'info&gt;
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(constraint = &lt;expr&gt;)]</code><br><br><code>#[account(constraint = &lt;expr&gt; @ &lt;custom_error&gt;)]</code>
///             </td>
///             <td>
///                 Constraint that checks whether the given expression evaluates to true.<br>
///                 Use this when no other constraint fits your use case.
///                 <br><br>
///                 Example:
///                 <pre><code>
/// #[account(constraint = one.keys[0].age == two.apple.age)]
/// pub one: Account&lt;'info, MyData&gt;,
/// pub two: Account&lt;'info, OtherData&gt;
///                 </code></pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(realloc = &lt;space&gt;, realloc::payer = &lt;target&gt;, realloc::zero = &lt;bool&gt;)]</code>
///             </td>
///             <td>
///                 Used to <a href="https://docs.rs/solana-program/latest/solana_program/account_info/struct.AccountInfo.html#method.realloc" target = "_blank" rel = "noopener noreferrer">realloc</a>
///                 program account space at the beginning of an instruction.
///                 <br><br>
///                 The account must be marked as <code>mut</code> and applied to either <code>Account</code> or <code>AccountLoader</code> types.
///                 <br><br>
///                 If the change in account data length is additive, lamports will be transferred from the <code>realloc::payer</code> into the
///                 program account in order to maintain rent exemption. Likewise, if the change is subtractive, lamports will be transferred from
///                 the program account back into the <code>realloc::payer</code>.
///                 <br><br>
///                 The <code>realloc::zero</code> constraint is required in order to determine whether the new memory should be zero initialized after
///                 reallocation. Please read the documentation on the <code>AccountInfo::realloc</code> function linked above to understand the
///                 caveats regarding compute units when providing <code>true</code> or <code>false</code> to this flag.
///                 <br><br>
///                 The manual use of `AccountInfo::realloc` is discouraged in favor of the `realloc` constraint group due to the lack of native runtime checks
///                 to prevent reallocation over the `MAX_PERMITTED_DATA_INCREASE` limit (which can unintentionally cause account data overwrite other accounts).
///                 The constraint group also ensure account reallocation idempotency but checking and restricting duplicate account reallocation within a single ix.
///                 <br><br>
///                 Example:
///                 <pre>
/// #[derive(Accounts)]
/// pub struct Example {
///     #[account(mut)]
///     pub payer: Signer&lt;'info&gt;,
///     #[account(
///         mut,
///         seeds = [b"example"],
///         bump,
///         realloc = 8 + std::mem::size_of::&lt;MyType&gt;() + 100,
///         realloc::payer = payer,
///         realloc::zero = false,
///     )]
///     pub acc: Account&lt;'info, MyType&gt;,
///     pub system_program: Program&lt;'info, System&gt;,
/// }
///                 </pre>
///             </td>
///         </tr>
///     </tbody>
/// </table>
///
/// # SPL Constraints
///
/// Anchor provides constraints that make verifying SPL accounts easier.
///
/// <table>
///     <thead>
///         <tr>
///             <th>Attribute</th>
///             <th>Description</th>
///         </tr>
///     </thead>
///     <tbody>
///         <tr>
///             <td>
///                 <code>#[account(token::mint = &lt;target_account&gt;, token::authority = &lt;target_account&gt;)]</code>
///             <br><br>
///                 <code>#[account(token::mint = &lt;target_account&gt;, token::authority = &lt;target_account&gt;, token::token_program = &lt;target_account&gt;)]</code>
///             </td>
///             <td>
///                 Can be used as a check or with <code>init</code> to create a token
///                 account with the given mint address and authority.<br>
///                  When used as a check, it's possible to only specify a subset of the constraints.
///                 <br><br>
///                 Example:
///                 <pre>
/// use anchor_spl::{mint, token::{TokenAccount, Mint, Token}};
/// ...&#10;
/// #[account(
///     init,
///     payer = payer,
///     token::mint = mint,
///     token::authority = payer,
/// )]
/// pub token: Account&lt;'info, TokenAccount&gt;,
/// #[account(address = mint::USDC)]
/// pub mint: Account&lt;'info, Mint&gt;,
/// #[account(mut)]
/// pub payer: Signer&lt;'info&gt;,
/// pub token_program: Program&lt;'info, Token&gt;,
/// pub system_program: Program&lt;'info, System&gt;
///                 </pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(mint::authority = &lt;target_account&gt;, mint::decimals = &lt;expr&gt;)]</code>
///                 <br><br>
///                 <code>#[account(mint::authority = &lt;target_account&gt;, mint::decimals = &lt;expr&gt;, mint::freeze_authority = &lt;target_account&gt;)]</code>
///             </td>
///             <td>
///                 Can be used as a check or with <code>init</code> to create a mint
///                 account with the given mint decimals and mint authority.<br>
///                 The freeze authority is optional when used with <code>init</code>.<br>
///                 When used as a check, it's possible to only specify a subset of the constraints.
///                 <br><br>
///                 Example:
///                 <pre>
/// use anchor_spl::token::{Mint, Token};
/// ...&#10;
/// #[account(
///     init,
///     payer = payer,
///     mint::decimals = 9,
///     mint::authority = payer,
/// )]
/// pub mint_one: Account&lt;'info, Mint&gt;,
/// #[account(
///     init,
///     payer = payer,
///     mint::decimals = 9,
///     mint::authority = payer,
///     mint::freeze_authority = payer
/// )]
/// pub mint_two: Account&lt;'info, Mint&gt;,
/// #[account(mut)]
/// pub payer: Signer&lt;'info&gt;,
/// pub token_program: Program&lt;'info, Token&gt;,
/// pub system_program: Program&lt;'info, System&gt;
///                 </pre>
///             </td>
///         </tr>
///         <tr>
///             <td>
///                 <code>#[account(associated_token::mint = &lt;target_account&gt;, associated_token::authority = &lt;target_account&gt;)]</code>
///                <br><br>
///                 <code>#[account(associated_token::mint = &lt;target_account&gt;, associated_token::authority = &lt;target_account&gt;, associated_token::token_program = &lt;target_account&gt;)]</code>
///             </td>
///             <td>
///                 Can be used as a standalone as a check or with <code>init</code> to create an associated token
///                 account with the given mint address and authority.
///                 <br><br>
///                 Example:
///                 <pre>
/// use anchor_spl::{
///     associated_token::AssociatedToken,
///     mint,
///     token::{TokenAccount, Mint, Token}
/// };
/// ...&#10;
/// #[account(
///     init,
///     payer = payer,
///     associated_token::mint = mint,
///     associated_token::authority = payer,
/// )]
/// pub token: Account&lt;'info, TokenAccount&gt;,
/// #[account(
///     associated_token::mint = mint,
///     associated_token::authority = payer,
/// )]
/// pub second_token: Account&lt;'info, TokenAccount&gt;,
/// #[account(address = mint::USDC)]
/// pub mint: Account&lt;'info, Mint&gt;,
/// #[account(mut)]
/// pub payer: Signer&lt;'info&gt;,
/// pub token_program: Program&lt;'info, Token&gt;,
/// pub associated_token_program: Program&lt;'info, AssociatedToken&gt;,
/// pub system_program: Program&lt;'info, System&gt;
///                 </pre>
///             </td>
///         </tr><tr>
///             <td>
///                 <code>#[account(*::token_program = &lt;target_account&gt;)]</code>
///             </td>
///             <td>
///                 The <code>token_program</code> can optionally be overridden.
///                 <br><br>
///                 Example:
///                 <pre>
/// use anchor_spl::token_interface::{TokenInterface, TokenAccount, Mint};
/// ...&#10;
/// #[account(
///     mint::token_program = token_a_token_program,
/// )]
/// pub token_a_mint: InterfaceAccount&lt;'info, Mint&gt;,
/// #[account(
///     mint::token_program = token_b_token_program,
/// )]
/// pub token_b_mint: InterfaceAccount&lt;'info, Mint&gt;,
/// #[account(
///     init,
///     payer = payer,
///     token::mint = token_a_mint,
///     token::authority = payer,
///     token::token_program = token_a_token_program,
/// )]
/// pub token_a_account: InterfaceAccount&lt;'info, TokenAccount&gt;,
/// #[account(
///     init,
///     payer = payer,
///     token::mint = token_b_mint,
///     token::authority = payer,
///     token::token_program = token_b_token_program,
/// )]
/// pub token_b_account: InterfaceAccount&lt;'info, TokenAccount&gt;,
/// pub token_a_token_program: Interface&lt;'info, TokenInterface&gt;,
/// pub token_b_token_program: Interface&lt;'info, TokenInterface&gt;,
/// #[account(mut)]
/// pub payer: Signer&lt;'info&gt;,
/// pub system_program: Program&lt;'info, System&gt;
///                 </pre>
///             </td>
///         </tr>
///     </tbody>
/// </table>
#[proc_macro_derive(Accounts, attributes(account, instruction))]
pub fn derive_accounts(item: TokenStream) -> TokenStream {
    parse_macro_input!(item as anchor_syn::AccountsStruct)
        .to_token_stream()
        .into()
}
