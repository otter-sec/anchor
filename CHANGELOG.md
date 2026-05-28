# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

**Note:** Version 0 of Semantic Versioning is handled differently from version 1 and above.
The minor version will be incremented upon a breaking change and the patch version will be incremented for features.

## [Unreleased]

### Features

- ts: Add `decodeIdlAccountRaw` ([#4375](https://github.com/solana-foundation/anchor/pull/4375)).
- cli: Add `--stdout` flag to the `expand` command ([#4400](https://github.com/solana-foundation/anchor/pull/4400)).
- client: Add versioned tx support ([#4207](https://github.com/solana-foundation/anchor/pull/4207)).
- cli: Add `edition` and `rust-version` to template ([#4048](https://github.com/solana-foundation/anchor/pull/4048))
- lang: Add `program_id` verification to CPI return values ([#4411](https://github.com/solana-foundation/anchor/pull/4411)).
- spl: Add pausable mint extension support ([#4092](https://github.com/solana-foundation/anchor/pull/4092)).
- cli: Resolve the target directory via `cargo metadata` to support target directory overrides ([#3817](https://github.com/solana-foundation/anchor/pull/3817)).
- spl: Added `token_metadata_remove_key` to support removing keys from token metadata extension ([#3717](https://github.com/solana-foundation/anchor/pull/3717)).
- cli: Allow configuring IDL JSON location in workspace config ([#4483](https://github.com/solana-foundation/anchor/pull/4483)).
- lang: Derive `Clone`, `Debug`, `Copy`, and `Default` on generated client / CPI account structs and instruction args where the field types allow it ([#4085](https://github.com/solana-foundation/anchor/pull/4085)).
- lang: Add `AccountLoader::new_unchecked` for constructing an `AccountLoader` without performing owner or discriminator checks ([#4162](https://github.com/solana-foundation/anchor/pull/4162)).

### Fixes

- idl: Bump version to 0.1.3 ([#4453](https://github.com/solana-foundation/anchor/pull/4453)).
- lang: Migrate `anchor-syn` from syn 1.x to syn 2.0, allowing use of modern Rust syntax ([#4523](https://github.com/solana-foundation/anchor/issues/4523)).
- lang: Avoid fatal errors in IDL building when modern Rust syntax is in use ([#4520](https://github.com/solana-foundation/anchor/pull/4520)).
- client: Avoid panic in `parse_logs_response` when a program-emitted log line ends with `invoke [1]` ([#4461](https://github.com/solana-foundation/anchor/issues/4461)).
- cli: Correctly honor `--skip-seed-phrase-validation` in `keygen recover` ([#4417](https://github.com/solana-foundation/anchor/pull/4417)).
- spl: Fix wrong owner pubkey in CPI Guard enable/disable ([#4322](https://github.com/solana-foundation/anchor/pull/4322)).
- ts: Fix `sha256.hash()` returning corrupted output by using hex encoding ([#4404](https://github.com/solana-foundation/anchor/pull/4404)).
- spl: Add missing auth account to `group_pointer_update` ([#4324](https://github.com/solana-foundation/anchor/pull/4324)).
- lang: Support module constants in `max_len` attribute ([#3879](https://github.com/solana-foundation/anchor/pull/3879)).
- spl: Deprecate broken `cpi_guard_enable/disable` functions ([#4465](https://github.com/solana-foundation/anchor/pull/4465)).
- cli: Bump `cargo_toml` to allow parsing `resolver = "3"` ([#4515](https://github.com/solana-foundation/anchor/pull/4515)).

### Breaking

## [1.0.2] - 2026-05-02

### Features

### Fixes

- client: Replace `solana-program` with `solana-hash` ([#4468](https://github.com/solana-foundation/anchor/pull/4468)).
- lang: Make idl build time way faster by caching `CrateContext` ([#4325](https://github.com/solana-foundation/anchor/pull/4325)).
- cli: Bind `localnet` to `127.0.0.1` by default to fix a panic in `solana-test-validator` version `3.1.10` ([#4397](https://github.com/solana-foundation/anchor/pull/4397)).
- cli: Fallback to a priority fee of 0 on localnet ([#4259](https://github.com/solana-foundation/anchor/pull/4259/)).
- lang/syn: Fix compile error with `init` and a runtime seeds expression ([#4495](https://github.com/solana-foundation/anchor/pull/4495)).

### Breaking

## [1.0.1] - 2026-04-16

### Features

### Fixes

- lang: Handle user-provided `borsh` attributes in derives ([#4380](https://github.com/solana-foundation/anchor/pull/4380)).

### Breaking

## [1.0.0] - 2026-04-02

### Features

- lang: Add `Migration<'info, From, To>` account type for schema migrations between account types ([#4060](https://github.com/solana-foundation/anchor/pull/4060)).
- cli: Added a `check_program_id_mismatch` in build time to check if the program ID in the source code matches the program ID in the keypair file ([#4018](https://github.com/solana-foundation/anchor/pull/4018)). This check will be skipped during `anchor test`.
- lang: Add instruction parser to `declare_program!` ([#4118](https://github.com/solana-foundation/anchor/pull/4118)).
- ts: Export all IDL types from the root. Users can now update `dist/cjs/idl` imports to import directly from `@anchor-lang/core` ([#3948](https://github.com/solana-foundation/anchor/pull/3948)).
- lang: Add `declare_program!` support with just `anchor_client` and not `anchor_lang` ([#4157](https://github.com/solana-foundation/anchor/pull/4157)).
- lang: Export `Owners` from `prelude` ([#4189](https://github.com/solana-foundation/anchor/pull/4189)).
- cli: Use surfpool by default for `anchor test` and `anchor localnet` commands ([#4106](https://github.com/solana-foundation/anchor/pull/4106)).
- lang: Optimize enums with all unit variants and empty arrays with `Lazy` ([#4237](https://github.com/solana-foundation/anchor/pull/4237)).
- lang: Include `init_if_needed` accounts in duplicate mutable account checks ([#4239](https://github.com/solana-foundation/anchor/pull/4239)).
- client: Accept `FnMut` for events closure ([#4024](https://github.com/solana-foundation/anchor/pull/4024)).
- client: Export all types used by the public API ([#4211](https://github.com/solana-foundation/anchor/pull/4211)).
- lang: Make `common::close` accept references ([#4178](https://github.com/solana-foundation/anchor/pull/4178)).
- cli/idl: Add `--allow-localnet` option for IDL commands; fix panic when run outside a workspace ([#4252](https://github.com/solana-foundation/anchor/pull/4252)).
- lang: Check owner on account reload ([#3837](https://github.com/solana-foundation/anchor/pull/3837)).
- lang/ts: Upgrade borsh to 1.5.7 ([#4012](https://github.com/solana-foundation/anchor/pull/4012)).
- syn: Relax seeds syntax to allow more flexible PDA seed expressions ([#3813](https://github.com/solana-foundation/anchor/pull/3813)).
- lang: Deprecate `AccountInfo` usage in `Accounts` macro with a compile-time warning ([#3854](https://github.com/solana-foundation/anchor/pull/3854)).
- cli: Update `anchor init` to use the `multiple` program template by default ([#3958](https://github.com/solana-foundation/anchor/pull/3958)).
- cli: Add `hooks` section to `Anchor.toml` for `{pre,post}-{build,test,deploy}` lifecycle hooks ([#3862](https://github.com/solana-foundation/anchor/pull/3862)).
- lang: Add generic program validation support to `Program` type allowing `Program<'info>` for executable-only validation ([#3878](https://github.com/solana-foundation/anchor/pull/3878)).
- cli: Added `litesvm` test template and made it the default option on `anchor init` ([#4316](https://github.com/solana-foundation/anchor/pull/4316))
- cli: Added `--install-agent-skills` to automatically install Solana agent skills during `anchor init` ([#4307](https://github.com/solana-foundation/anchor/pull/4307))
- avm: Added flags and version labels to explicitly handle pre-releases (`avm list --pre-release`, `avm update --pre-release` and `avm install latest-pre-release`). ([#4335](https://github.com/solana-foundation/anchor/pull/4335))
- avm: Added `avm self-update` command and passive version check warning for out of date avm ([#4338](https://github.com/solana-foundation/anchor/pull/4338))
- lang, cli, client: Updated solana dependencies to the latest compatible versions. Bumping CI and docker builds to use Solana CLI version 3.1.10 ([#4317](https://github.com/solana-foundation/anchor/pull/4317/))

### Fixes

- lang: Add missing `Lazy` bound on generics ([#4240](https://github.com/solana-foundation/anchor/pull/4240)).
- lang: Fix wrong generated error code in declare_program! ([#4129](https://github.com/solana-foundation/anchor/pull/4129)).
- idl: Fix defined types with unsupported fields not producing an error ([#4088](https://github.com/solana-foundation/anchor/pull/4088)).
- lang: Fix using non-instruction composite accounts multiple times with `declare_program!` ([#4113](https://github.com/solana-foundation/anchor/pull/4113)).
- lang: Fix `declare_program!` messing up IDL errors generation ([#4126](https://github.com/solana-foundation/anchor/pull/4126)).
- idl: Fix `address` constraint not resolving constants that have numbers in their identifiers ([#4144](https://github.com/solana-foundation/anchor/pull/4144)).
- lang: Fix constant nested string generation in `declare_program!` ([#4158](https://github.com/solana-foundation/anchor/pull/4158)).
- idl: Fix `local_file` method not found for `proc_macro2::Span` error ([#4187](https://github.com/solana-foundation/anchor/pull/4187)).
- lang: Relax duplicate mutable account constraint to only check types that serialize on exit (`Account`, `LazyAccount`, `InterfaceAccount`, `Migration`) ([#4202](https://github.com/solana-foundation/anchor/pull/4202)).
- idl: Make `serde_json` optional ([#4296](https://github.com/solana-foundation/anchor/pull/4296)).
- lang: Fix `declare_program!` instruction parser with optional accounts ([#4180](https://github.com/solana-foundation/anchor/pull/4180)).
- lang: Use original `borsh` derives ([#4205](https://github.com/solana-foundation/anchor/pull/4205)).
- lang: Fix unexpected account substitution in `InterfaceAccount` ([#4139](https://github.com/solana-foundation/anchor/pull/4139)).
- idl: Respect `offset = ...` in IDL generation for custom errors ([#4040](https://github.com/solana-foundation/anchor/pull/4040)).
- cli: Relax separate dependency check for `solana-program` ([#4166](https://github.com/solana-foundation/anchor/pull/4166)).
- lang: Enforce type and count matching between instruction handler and `#[instruction(..)]` args ([#4000](https://github.com/solana-foundation/anchor/pull/4000)).
- lang: Handle invalid camelCase identifiers more gracefully ([#4021](https://github.com/solana-foundation/anchor/pull/4021)).
- cli: Fix `i128`/`u128` deserialization ([#3938](https://github.com/solana-foundation/anchor/pull/3938)).
- ts: Fix incorrect Anchor dependency version requirements ([#4138](https://github.com/solana-foundation/anchor/pull/4138)).
- lang: Omit `parsers` module of `declare_program!` during on-chain (Solana) builds ([#4109](https://github.com/solana-foundation/anchor/pull/4109)).
- docs: Fixed broken links and replaced coral-xyz github references to solana-foundation ([#4320](https://github.com/solana-foundation/anchor/pull/4320))
- avm: Fixed handling of new Cargo.toml version location. Fixed handling of pre-release version parsing. ([#4335](https://github.com/solana-foundation/anchor/pull/4335))
- client: Fix deadlock when having multiple websocket listeners ([#4250](https://github.com/solana-foundation/anchor/pull/4250)).
- lang: Fix incorrect deserialization for dynamically sized types when using `lazy-account` ([#4319](https://github.com/solana-foundation/anchor/pull/4319))
- avm: Using a temporary installation dir on cargo install calls to prevent cargo erroring out due to existing `anchor` symlink in `.avm/bin` ([4343](https://github.com/solana-foundation/anchor/pull/4343))

### Breaking

- cli: Remove program `arch` options ([#4295](https://github.com/solana-foundation/anchor/pull/4295)).
- lang: Disallow duplicate mutable accounts by default. But allows duplicate mutable accounts in instruction contexts using `dup` constraint ([#3946](https://github.com/solana-foundation/anchor/pull/3946)).
- cli: Remove program id arguments of `idl init` and `idl upgrade` commands ([#4130](https://github.com/solana-foundation/anchor/pull/4130)).
- lang: Rename `utils` module of `declare_program!` to `parsers` ([#4151](https://github.com/solana-foundation/anchor/pull/4151)).
- lang: Remove the `interface-instructions` feature and the `#[interface]` attribute ([#4156](https://github.com/solana-foundation/anchor/pull/4156)).
- cli: Remove the `login` command ([#4182](https://github.com/solana-foundation/anchor/pull/4182)).
- idl: Exclude external accounts ([#4197](https://github.com/solana-foundation/anchor/pull/4197)).
- idl: Remove the conflicting account names check ([#4294](https://github.com/solana-foundation/anchor/pull/4294)).
- deps: Update to Solana 3.0 ([#4031](https://github.com/solana-foundation/anchor/pull/4031)).
- idl: Remove legacy IDL instructions and integrate Program Metadata for IDL management ([#3798](https://github.com/solana-foundation/anchor/pull/3798)).
- ts: Rename TypeScript packages from `@coral-xyz/anchor` to `@anchor-lang/core` ([#4141](https://github.com/solana-foundation/anchor/pull/4141)).
- lang: Remove program account info from CPI context ([#2762](https://github.com/solana-foundation/anchor/pull/2762)).
- cli: Remove dependency on the external `solana` CLI; native implementations provided for balance, airdrop, address, deploy, and other commands ([#4099](https://github.com/solana-foundation/anchor/pull/4099)).
- idl: Disallow multiple `#[error_code]` definitions in a single program ([#4300](https://github.com/solana-foundation/anchor/pull/4300)).
- cli: Remove the `[registry]` section from `Anchor.toml` ([#4299](https://github.com/solana-foundation/anchor/pull/4299)).
- client: Make sending a tx not panic and instead return an Error when signing fails ([#3865](https://github.com/solana-foundation/anchor/pull/3865)).
- lang: Rename `errors` and `ProgramError` of `declare_program!` ([#4347](https://github.com/solana-foundation/anchor/pull/4347)).
- client: Remove the `solana-account-decoder` crate export ([#4373](https://github.com/solana-foundation/anchor/pull/4373)).

## [0.32.1] - 2025-10-09

### Features

### Fixes

- lang: Fix deprecation warnings on alloc and add solana-program to prelude
  ([#3975](https://github.com/solana-foundation/anchor/pull/3975)).
- cli: Fix race condition that could happen when deploying a program
  ([#3976](https://github.com/solana-foundation/anchor/pull/3976)).

### Breaking

## [0.32.0] - 2025-10-08

### Features

- lang: Add `#[error]` attribute to `declare_program!` ([#3757](https://github.com/solana-foundation/anchor/pull/3757)).
- cli: Replace `anchor verify` to use `solana-verify` under the hood, adding automatic installation via AVM, local path support, and future-proof argument passing ([#3768](https://github.com/solana-foundation/anchor/pull/3768)).
- lang: Replace `solana-program` crate with smaller crates ([#3819](https://github.com/solana-foundation/anchor/pull/3819)).
- cli: Make `anchor deploy` to upload the IDL to the cluster by default unless `--no-idl` is passed ([#3863](https://github.com/solana-foundation/anchor/pull/3863)).
- lang: Use `solana-invoke` instead of `solana_cpi::invoke` ([#3900](https://github.com/solana-foundation/anchor/pull/3900)).
- client: remove `solana-client` from `anchor-client` and `cli` ([#3877](https://github.com/solana-foundation/anchor/pull/3877)).
- idl: Build IDL on stable Rustc ([#3842](https://github.com/solana-foundation/anchor/pull/3842)).
- lang: Add custom error when using init on SystemAccount ([#3828](https://github.com/solana-foundation/anchor/pull/3828)).
- ts: Add support for Bun as a package manager ([#3586](https://github.com/solana-foundation/anchor/pull/3586)).
- lang: Add support for tuple types in space calculation ([#3744](https://github.com/solana-foundation/anchor/pull/3744)).
- lang: Add missing pubkey const generation ([#3677](https://github.com/solana-foundation/anchor/pull/3677)).
- cli: Add the Minimum Supported Rust Version (MSRV) to the Rust template, since an arbitrary compiler version isn't supported ([#3873](https://github.com/solana-foundation/anchor/pull/3873)).
- lang: Update mollusk test template dependencies to `~0.4` and remove redundant `solana-program` dev-dependency from generated template ([#3790](https://github.com/solana-foundation/anchor/pull/3790)).

### Fixes

- docker: Upgrade `node` to 20.18.0 LTS ([#3687](https://github.com/solana-foundation/anchor/pull/3687)).
- cli: Fix using deprecated commitment `recent` in migration scripts ([#3725](https://github.com/solana-foundation/anchor/pull/3725)).
- cli: Fix not respecting `provider.cluster` in `keys sync` command ([#3761](https://github.com/solana-foundation/anchor/pull/3761)).
- lang: Fix deprecated `realloc`, `store_current_index` and clippy warnings ([#3819](https://github.com/solana-foundation/anchor/pull/3819)).
- avm: fix AVM instability with solana-verify ([#3867](https://github.com/solana-foundation/anchor/pull/3867)).
- avm: update AVM to only use non-draft releases([#3931](https://github.com/solana-foundation/anchor/pull/3931)).
- lang: update bytemuck ([#3858](https://github.com/solana-foundation/anchor/pull/3858)).
- idl: disable Locale in camelCase ([#3845](https://github.com/solana-foundation/anchor/pull/3845)).
- ts: Remove event parsing panic ([#3657](https://github.com/solana-foundation/anchor/pull/3657)).
- avm: Expose `anchor` on `PATH` via `$CARGO_HOME/bin` symlink ([#3835](https://github.com/solana-foundation/anchor/pull/3835)).

### Breaking

- spl: Update SPL dependencies to latest compatible versions ([#3860](https://github.com/solana-foundation/anchor/pull/3860)).
- cli: Replace `anchor verify` to use `solana-verify` under the hood, adding automatic installation via AVM, local path support, and future-proof argument passing ([#3768](https://github.com/solana-foundation/anchor/pull/3768)).
- cli: Upload IDL by default with an option to skip ((#3863)[https://github.com/solana-foundation/anchor/pull/3863]).
- lang: remove Solang ([#3824](https://github.com/solana-foundation/anchor/pull/3824)).
- cli: remove `anchor publish` command ([#3795](https://github.com/solana-foundation/anchor/pull/3795)).

## [0.31.1] - 2025-04-19

This release uses a new docker image found at `solanafoundation/anchor` for the `anchor verify` command. New images will be pushed to this organization in the future.

Releases for anchor will also be published under the `solanafoundation` Github organization from now on.

### Features

- cli, docker: Replace `backpackapp/build` Docker image with `solanafoundation/anchor` ([#3619](https://github.com/solana-foundation/anchor/pull/3619)).
- ts: Make `Provider` require publicKey instead of wallet in accounts resolver ([#3613](https://github.com/solana-foundation/anchor/pull/3613))

### Fixes

- idl: Update `proc-macro2` usage for latest nightly ([#3663](https://github.com/solana-foundation/anchor/pull/3663))
- ts: Fix parsing IDL with multiple const generics ([#3665](https://github.com/solana-foundation/anchor/pull/3665))

### Breaking

## [0.31.0] - 2025-03-08

### Features

- client: Make `solana_account_decoder` dep public in anchor client ([#3455](https://github.com/solana-foundation/anchor/pull/3455)).
- ts: Add optional `options.blockhash` to `Provider.sendAndConfirm` ([#3070](https://github.com/solana-foundation/anchor/pull/3070)).
- ts: Add optional `commitment` parameter to `Program.addEventListener` ([#3052](https://github.com/solana-foundation/anchor/pull/3052)).
- cli, idl: Pass `cargo` args to IDL generation when building program or IDL ([#3059](https://github.com/solana-foundation/anchor/pull/3059)).
- cli: Add checks for incorrect usage of `idl-build` feature ([#3061](https://github.com/solana-foundation/anchor/pull/3061)).
- lang: Export `Discriminator` trait from `prelude` ([#3075](https://github.com/solana-foundation/anchor/pull/3075)).
- lang: Add `Account` utility type to get accounts from bytes ([#3091](https://github.com/solana-foundation/anchor/pull/3091)).
- client: Add option to pass in mock rpc client when using anchor_client ([#3053](https://github.com/solana-foundation/anchor/pull/3053)).
- lang: Get discriminator length dynamically ([#3101](https://github.com/solana-foundation/anchor/pull/3101)).
- lang: Add non-8-byte discriminator support in `declare_program!` ([#3103](https://github.com/solana-foundation/anchor/pull/3103)).
- client: Make `ThreadSafeSigner` trait public ([#3107](https://github.com/solana-foundation/anchor/pull/3107)).
- lang: Update `dispatch` function to support dynamic discriminators ([#3104](https://github.com/solana-foundation/anchor/pull/3104)).
- lang: Remove the fallback function shortcut in `try_entry` function ([#3109](https://github.com/solana-foundation/anchor/pull/3109)).
- ts: Get discriminator lengths dynamically ([#3120](https://github.com/solana-foundation/anchor/pull/3120)).
- client: Support non-8-byte discriminators ([#3125](https://github.com/solana-foundation/anchor/pull/3125)).
- spl: Add `withdraw_withheld_tokens_from_accounts` instruction ([#3128](https://github.com/solana-foundation/anchor/pull/3128)).
- ts: Add optional `wallet` property to the `Provider` interface ([#3130](https://github.com/solana-foundation/anchor/pull/3130)).
- cli: Warn if `anchor-spl/idl-build` is missing ([#3133](https://github.com/solana-foundation/anchor/pull/3133)).
- client: Add `internal_rpc` method for `mock` feature ([#3135](https://github.com/solana-foundation/anchor/pull/3135)).
- lang: Add `#[instruction]` attribute proc-macro to override default instruction discriminators ([#3137](https://github.com/solana-foundation/anchor/pull/3137)).
- lang: Use associated discriminator constants instead of hardcoding in `#[account]` ([#3144](https://github.com/solana-foundation/anchor/pull/3144)).
- lang: Add `discriminator` argument to `#[account]` attribute ([#3149](https://github.com/solana-foundation/anchor/pull/3149)).
- lang: Add `discriminator` argument to `#[event]` attribute ([#3152](https://github.com/solana-foundation/anchor/pull/3152)).
- idl: Check ambiguous discriminators ([#3157](https://github.com/solana-foundation/anchor/pull/3157)).
- idl: Disallow all zero account discriminators ([#3159](https://github.com/solana-foundation/anchor/pull/3159)).
- cli: Support non-8-byte discriminators ([#3165](https://github.com/solana-foundation/anchor/pull/3165)).
- idl: Disallow empty discriminators ([#3166](https://github.com/solana-foundation/anchor/pull/3166)).
- cli: Add `--no-idl` option to the `test` command ([#3175](https://github.com/solana-foundation/anchor/pull/3175)).
- spl: Add `burn_checked`, `mint_to_checked` and `approve_checked` instructions ([#3186](https://github.com/solana-foundation/anchor/pull/3186)).
- cli: Migrate to `agave-install` when `solana_version` is `>= 1.18.19` ([#3185](https://github.com/solana-foundation/anchor/pull/3185)).
- idl: Add `IdlBuilder` ([#3188](https://github.com/solana-foundation/anchor/pull/3188)).
- cli: Make `clean` command also remove the `.anchor` directory ([#3192](https://github.com/solana-foundation/anchor/pull/3192)).
- lang: Deprecate `#[interface]` attribute ([#3195](https://github.com/solana-foundation/anchor/pull/3195)).
- ts: Include unresolved accounts in the resolution error message ([#3207](https://github.com/solana-foundation/anchor/pull/3207)).
- lang: Add `LazyAccount` ([#3194](https://github.com/solana-foundation/anchor/pull/3194)).
- avm: Ask whether to install if the version is not installed with the `use` command ([#3230](https://github.com/solana-foundation/anchor/pull/3230)).
- cli: Warn if a manifest has `solana-program` dependency ([#3250](https://github.com/solana-foundation/anchor/pull/3250)).
- cli: Add completions command to generate shell completions via the clap_complete crate ([#3251](https://github.com/solana-foundation/anchor/pull/3251)).
- cli: Always convert IDLs ([#3265](https://github.com/solana-foundation/anchor/pull/3265)).
- cli: Check whether the `idl-build` feature exists when using the `idl build` command ([#3273](https://github.com/solana-foundation/anchor/pull/3273)).
- cli: Build IDL if there is only one program when using the `idl build` command ([#3275](https://github.com/solana-foundation/anchor/pull/3275)).
- cli: Add short alias for the `idl build` command ([#3283](https://github.com/solana-foundation/anchor/pull/3283)).
- cli: Add `--program-id` option to `idl convert` command ([#3309](https://github.com/solana-foundation/anchor/pull/3309)).
- lang: Generate documentation of constants in `declare_program!` ([#3311](https://github.com/solana-foundation/anchor/pull/3311)).
- cli: Add support for fetching legacy IDLs ([#3324](https://github.com/solana-foundation/anchor/pull/3324)).
- avm: Add short alias for `install` and `list` commands ([#3326](https://github.com/solana-foundation/anchor/pull/3326)).
- avm: Add Windows support for renaming anchor binary ([#3325](https://github.com/solana-foundation/anchor/pull/3325)).
- cli: Add optional `package-manager` flag in `init` command to set package manager field in Anchor.toml ([#3328](https://github.com/solana-foundation/anchor/pull/3328)).
- cli: Add test template for [Mollusk](https://github.com/buffalojoec/mollusk) ([#3352](https://github.com/solana-foundation/anchor/pull/3352)).
- idl: Disallow account discriminators that can conflict with the `zero` constraint ([#3365](https://github.com/solana-foundation/anchor/pull/3365)).
- cli: Include recommended solana args by default and add new `--max-retries` option to the `deploy` command ([#3354](https://github.com/solana-foundation/anchor/pull/3354)).
- avm: Make installation download binaries by default ([#3445](https://github.com/solana-foundation/anchor/pull/3445)).
- idl: Support PDA resolution of call expressions that don't have any arguments ([#3485](https://github.com/solana-foundation/anchor/pull/3485)).
- spl: Add `anchor-debug` feature ([#3511](https://github.com/solana-foundation/anchor/pull/3511)).
- avm: Always install commit hash inputs from source ([#3461](https://github.com/solana-foundation/anchor/pull/3461)).

### Fixes

- idl: Make safety comment checks fail silently when program path env is not set ([#3045](https://github.com/solana-foundation/anchor/pull/3045)).
- idl: Avoid interference from rust tests during IDL generation ([#3058](https://github.com/solana-foundation/anchor/pull/3058)).
- lang: Fix `align` repr support in `declare-program!` ([#3056](https://github.com/solana-foundation/anchor/pull/3056)).
- lang: Make stack frames slimmer on ATA creation ([#3065](https://github.com/solana-foundation/anchor/pull/3065)).
- lang: Remove `getrandom` dependency ([#3072](https://github.com/solana-foundation/anchor/pull/3072)).
- lang: Make `InitSpace` support unnamed & unit structs ([#3084](https://github.com/solana-foundation/anchor/pull/3084)).
- lang: Fix using `owner` constraint with `Box`ed accounts ([#3087](https://github.com/solana-foundation/anchor/pull/3087)).
- lang: Add a sanity check for unimplemented token extensions ([#3090](https://github.com/solana-foundation/anchor/pull/3090)).
- cli: Skip IDL checks if `--no-idl` option is passed ([#3093](https://github.com/solana-foundation/anchor/pull/3093)).
- lang: Fix compilation error when an `#[account]` struct has generics ([#3148](https://github.com/solana-foundation/anchor/pull/3148)).
- lang: Remove unnecessary clone in account exit routine ([#3139](https://github.com/solana-foundation/anchor/pull/3139)).
- cli: Fix installation with `--locked` argument using Rust v1.80 due to `time` crate issue ([#3143](https://github.com/solana-foundation/anchor/pull/3143)).
- lang: Fix compilation warnings due to unused deprecated program id macros ([#3170](https://github.com/solana-foundation/anchor/pull/3170)).
- ts: Remove `crypto-hash` dependency ([#3171](https://github.com/solana-foundation/anchor/pull/3171)).
- ts: Improve error message of unsupported `view` method ([#3177](https://github.com/solana-foundation/anchor/pull/3177)).
- idl: Fix panicking on tests ([#3197](https://github.com/solana-foundation/anchor/pull/3197)).
- lang: Remove `arrayref` dependency ([#3201](https://github.com/solana-foundation/anchor/pull/3201)).
- cli: Fix template code shouldn't escape ([#3210](https://github.com/solana-foundation/anchor/pull/3210)).
- idl: Fix using `address` constraint with non-const expressions ([#3216](https://github.com/solana-foundation/anchor/pull/3216)).
- idl: Fix using full path types with `Program` ([#3228](https://github.com/solana-foundation/anchor/pull/3228)).
- lang: Use closures for `init` constraints to reduce the stack usage of `try_accounts` ([#2939](https://github.com/solana-foundation/anchor/pull/2939)).
- lang: Allow the `cfg` attribute above the instructions ([#2339](https://github.com/solana-foundation/anchor/pull/2339)).
- idl: Log output with `ANCHOR_LOG` on failure and improve build error message ([#3284](https://github.com/solana-foundation/anchor/pull/3284)).
- lang: Fix constant bytes declarations when using `declare_program!` ([#3287](https://github.com/solana-foundation/anchor/pull/3287)).
- lang: Fix using non-instruction composite accounts with `declare_program!` ([#3290](https://github.com/solana-foundation/anchor/pull/3290)).
- idl: Fix instructions with tuple parameters not producing an error([#3294](https://github.com/solana-foundation/anchor/pull/3294)).
- ts: Update `engines.node` to `>= 17` ([#3301](https://github.com/solana-foundation/anchor/pull/3301)).
- cli: Use OS-agnostic paths ([#3307](https://github.com/solana-foundation/anchor/pull/3307)).
- avm: Use `rustc 1.79.0` when installing versions older than v0.31 ([#3315](https://github.com/solana-foundation/anchor/pull/3315)).
- cli: Fix priority fee calculation causing panic on localnet ([#3318](https://github.com/solana-foundation/anchor/pull/3318)).
- cli: Fix `shell` command failing due to outdated program initialization ([#3351](https://github.com/solana-foundation/anchor/pull/3351)).
- idl: Fix detecting false-positives from doc comments during module path conversion ([#3359](https://github.com/solana-foundation/anchor/pull/3359)).
- cli: Remove passing the rent sysvar account to IDL instructions ([#3372](https://github.com/solana-foundation/anchor/pull/3372)).
- lang: Fix `cpi` feature instructions not accounting for discriminator overrides ([#3376](https://github.com/solana-foundation/anchor/pull/3376)).
- idl: Ignore compiler warnings during builds ([#3396](https://github.com/solana-foundation/anchor/pull/3396)).
- cli: Avoid extra IDL generation during `verify` ([#3398](https://github.com/solana-foundation/anchor/pull/3398)).
- lang: Require `zero` accounts to be unique ([#3409](https://github.com/solana-foundation/anchor/pull/3409)).
- lang: Deduplicate `zero` accounts against `init` accounts ([#3422](https://github.com/solana-foundation/anchor/pull/3422)).
- cli: Fix custom `provider.cluster` ([#3428](https://github.com/solana-foundation/anchor/pull/3428)).
- cli: Ignore non semver solana/agave releases to avoid panic ([#3432](https://github.com/solana-foundation/anchor/pull/3432)).
- ts: Fix loading programs with numbers in their names using `workspace` ([#3450](https://github.com/solana-foundation/anchor/pull/3450)).
- lang: Remove a potential panic while getting the IDL in `declare_program!` ([#3458](https://github.com/solana-foundation/anchor/pull/3458)).
- cli: Fix altering user-provided lib names ([#3467](https://github.com/solana-foundation/anchor/pull/3467)).
- idl: Fix missing `program::seed` resolution ([#3474](https://github.com/solana-foundation/anchor/pull/3474)).
- lang: Fix adding `derive`s and `repr`s to type alias definitions in `declare_program!` ([#3504](https://github.com/solana-foundation/anchor/pull/3504)).
- idl: Fix using constant identifiers as generic arguments ([#3522](https://github.com/solana-foundation/anchor/pull/3522)).
- client: Remove `std::process::exit` usage ([#3544](https://github.com/solana-foundation/anchor/pull/3544)).
- idl: Fix using `Pubkey` constants with `seeds::program` ([#3559](https://github.com/solana-foundation/anchor/pull/3559)).
- lang: Fix instructions with no accounts causing compilation errors when using `declare_program!` ([#3567](https://github.com/solana-foundation/anchor/pull/3567)).
- idl: Fix using account or arg values for `seeds::program` ([#3570](https://github.com/solana-foundation/anchor/pull/3570)).
- lang: Fix using `data` as an instruction parameter name in `declare_program!` ([#3574](https://github.com/solana-foundation/anchor/pull/3574)).
- cli: Use camelCase for program name in `anchor.workspace` templates ([#3581](https://github.com/solana-foundation/anchor/pull/3581)).
- cli: Fix the default JS update command being incorrect ([#3337](https://github.com/solana-foundation/anchor/pull/3337)).
- cli: Remove stripping workspace prefix from program id of the `upgrade` command ([#3345](https://github.com/solana-foundation/anchor/pull/3345)).
- cli: Convert package managers to lowercase during serialization ([#3531](https://github.com/solana-foundation/anchor/pull/3531)).

### Breaking

- syn: Remove `bpf` target support in `hash` feature ([#3078](https://github.com/solana-foundation/anchor/pull/3078)).
- client: Add `tokio` support to `RequestBuilder` with `async` feature ([#3057](https://github.com/solana-foundation/anchor/pull/3057)).
- lang: Remove `EventData` trait ([#3083](https://github.com/solana-foundation/anchor/pull/3083)).
- client: Remove `async_rpc` method ([#3053](https://github.com/solana-foundation/anchor/pull/3053)).
- lang: Make discriminator type unsized ([#3098](https://github.com/solana-foundation/anchor/pull/3098)).
- lang: Require `Discriminator` trait impl when using the `zero` constraint ([#3118](https://github.com/solana-foundation/anchor/pull/3118)).
- ts: Remove `DISCRIMINATOR_SIZE` constant ([#3120](https://github.com/solana-foundation/anchor/pull/3120)).
- lang: `#[account]` attribute arguments no longer parses identifiers as namespaces ([#3140](https://github.com/solana-foundation/anchor/pull/3140)).
- spl: Rename metadata interface instruction fields from `token_program_id` to `program_id` ([#3076](https://github.com/solana-foundation/anchor/pull/3076)).
- lang, ts: Remove "8 byte" requirement from discriminator error messages ([#3161](https://github.com/solana-foundation/anchor/pull/3161)).
- lang: Remove `discriminator` method from `Discriminator` trait ([#3163](https://github.com/solana-foundation/anchor/pull/3163)).
- docker: Upgrade `node` to 20.16.0 LTS ([#3179](https://github.com/solana-foundation/anchor/pull/3179)).
- ts: Change the `Program` constructor's `idl` parameter type to `any` ([#3181](https://github.com/solana-foundation/anchor/pull/3181)).
- lang, spl: Remove `borsh 0.9` support ([#3199](https://github.com/solana-foundation/anchor/pull/3199)).
- ts: Upgrade `typescript` to `5.5.4` and remove the generic parameters of `SimulateResponse` ([#3221](https://github.com/solana-foundation/anchor/pull/3221)).
- ts: Remove `StateCoder`([#3224](https://github.com/solana-foundation/anchor/pull/3224)).
- cli: Accept integers for `warp_slot` ([#3235](https://github.com/solana-foundation/anchor/pull/3235)).
- lang: Remove `EventIndex` ([#3244](https://github.com/solana-foundation/anchor/pull/3244)).
- spl: Remove `dex` feature ([#3257](https://github.com/solana-foundation/anchor/pull/3257)).
- client, lang, spl: Upgrade Solana to v2 and SPL to the latest ([#3219](https://github.com/solana-foundation/anchor/pull/3219)).
- cli: Install Solana from anza.xyz domain in Docker verifiable builds ([#3271](https://github.com/solana-foundation/anchor/pull/3271)).
- spl: Upgrade SPL deps to latest ([#3346](https://github.com/solana-foundation/anchor/pull/3346)).
- cli: Upgrade `typescript` version of templates to v5 ([#3480](https://github.com/solana-foundation/anchor/pull/3480)).
- ts: Remove `snake-case` dependency ([#3507](https://github.com/solana-foundation/anchor/pull/3507)).
- deps: Upgrade Solana to 2.1.0 ([#3339](https://github.com/solana-foundation/anchor/pull/3339)).
- spl: Update `mpl-token-metadata` version requirement to `5` ([#3356](https://github.com/solana-foundation/anchor/pull/3356)).

## [0.30.1] - 2024-06-20

### Features

- idl: Allow overriding the idl build toolchain with the `RUSTUP_TOOLCHAIN` environment variable ([#2941](https://github.com/solana-foundation/anchor/pull/2941)).
- avm: Support customizing the installation location using `AVM_HOME` environment variable ([#2917](https://github.com/solana-foundation/anchor/pull/2917)).
- avm: Optimize `avm list` when GitHub API rate limits are reached ([#2962](https://github.com/solana-foundation/anchor/pull/2962))
- idl, ts: Add accounts resolution for associated token accounts ([#2927](https://github.com/solana-foundation/anchor/pull/2927)).
- cli: Add `--no-install` option to the `init` command ([#2945](https://github.com/solana-foundation/anchor/pull/2945)).
- lang: Implement `TryFromIntError` for `Error` to be able to propagate integer conversion errors ([#2950](https://github.com/solana-foundation/anchor/pull/2950)).
- idl: Add ability to convert legacy IDLs ([#2986](https://github.com/solana-foundation/anchor/pull/2986)).
- ts: Extract Anchor error codes into their own package ([#2983](https://github.com/solana-foundation/anchor/pull/2983)).
- cli: Add additional solana arguments to the `upgrade` command ([#2998](https://github.com/solana-foundation/anchor/pull/2998)).
- spl: Export `spl-associated-token-account` crate ([#2999](https://github.com/solana-foundation/anchor/pull/2999)).
- lang: Support legacy IDLs with `declare_program!` ([#2997](https://github.com/solana-foundation/anchor/pull/2997)).
- cli: Add `idl convert` command ([#3009](https://github.com/solana-foundation/anchor/pull/3009)).
- cli: Add `idl type` command ([#3017](https://github.com/solana-foundation/anchor/pull/3017)).
- lang: Add `anchor_lang::pubkey` macro for declaring `Pubkey` const values ([#3021](https://github.com/solana-foundation/anchor/pull/3021)).
- cli: Sync program ids on the initial build ([#3023](https://github.com/solana-foundation/anchor/pull/3023)).
- idl: Remove `anchor-syn` dependency ([#3030](https://github.com/solana-foundation/anchor/pull/3030)).
- lang: Add `const` of program ID to `declare_id!` and `declare_program!` ([#3019](https://github.com/solana-foundation/anchor/pull/3019)).
- idl: Add separate spec crate ([#3036](https://github.com/solana-foundation/anchor/pull/3036)).
- lang: Include `pubkey!` macro in `prelude` ([#3026](https://github.com/solana-foundation/anchor/pull/3026)).
- cli: Print total steps in IDL buffer writes ([#2944](https://github.com/solana-foundation/anchor/pull/2944)).

### Fixes

- lang: Eliminate variable allocations that build up stack space for token extension code generation ([#2913](https://github.com/solana-foundation/anchor/pull/2913)).
- ts: Fix incorrect `maxSupportedTransactionVersion` in `AnchorProvider.send*()` methods ([#2922](https://github.com/solana-foundation/anchor/pull/2922)).
- cli: Use npm's configured default license for new projects made with `anchor init` ([#2929](https://github.com/solana-foundation/anchor/pull/2929)).
- cli: add filename to 'Unable to read keypair file' errors ([#2932](https://github.com/solana-foundation/anchor/pull/2932)).
- idl: Fix path resolution of the `Cargo.lock` of the project when generating idls for external types ([#2946](https://github.com/solana-foundation/anchor/pull/2946)).
- idl: Fix potential panic on external type resolution ([#2954](https://github.com/solana-foundation/anchor/pull/2954)).
- lang: Fix using defined types in instruction parameters with `declare_program!` ([#2959](https://github.com/solana-foundation/anchor/pull/2959)).
- lang: Fix using const generics with `declare_program!` ([#2965](https://github.com/solana-foundation/anchor/pull/2965)).
- lang: Fix using `Vec<u8>` type with `declare_program!` ([#2966](https://github.com/solana-foundation/anchor/pull/2966)).
- lang: Fix `ProgramError::ArithmeticOverflow` not found error ([#2975](https://github.com/solana-foundation/anchor/pull/2975)).
- lang: Fix using optional accounts with `declare_program!` ([#2967](https://github.com/solana-foundation/anchor/pull/2967)).
- lang: Fix instruction return type generation with `declare_program!` ([#2977](https://github.com/solana-foundation/anchor/pull/2977)).
- cli: Fix IDL write getting corrupted from retries ([#2964](https://github.com/solana-foundation/anchor/pull/2964)).
- idl: Fix `unexpected_cfgs` build warning ([#2992](https://github.com/solana-foundation/anchor/pull/2992)).
- lang: Make tuple struct fields public in `declare_program!` ([#2994](https://github.com/solana-foundation/anchor/pull/2994)).
- Remove `rust-version` from crate manifests ([#3000](https://github.com/solana-foundation/anchor/pull/3000)).
- cli: Fix upgradeable program clones ([#3010](https://github.com/solana-foundation/anchor/pull/3010)).
- ts: Fix using IDLs that have defined types as generic arguments ([#3016](https://github.com/solana-foundation/anchor/pull/3016)).
- idl: Fix generation with unsupported expressions ([#3033](https://github.com/solana-foundation/anchor/pull/3033)).
- idl: Fix using `address` constraint with field expressions ([#3034](https://github.com/solana-foundation/anchor/pull/3034)).
- lang: Fix using `bytemuckunsafe` account serialization with `declare_program!` ([#3037](https://github.com/solana-foundation/anchor/pull/3037)).
- ts: Upgrade `@babel/traverse` to 7.24.1 ([#2919](https://github.com/solana-foundation/anchor/pull/2919)).

### Breaking

- deps: Upgrade Solana to 1.18.17 ([#3039](https://github.com/solana-foundation/anchor/pull/3039)).

## [0.30.0] - 2024-04-15

See the [Anchor 0.30 release notes](https://www.anchor-lang.com/release-notes/0.30.0) for a high-level overview of how to update.

### Features

- cli: Allow force `init` and `new` ([#2698](https://github.com/solana-foundation/anchor/pull/2698)).
- cli: Add verifiable option when `deploy` ([#2705](https://github.com/solana-foundation/anchor/pull/2705)).
- cli: Add support for passing arguments to the underlying `solana program deploy` command with `anchor deploy` ([#2709](https://github.com/solana-foundation/anchor/pull/2709)).
- lang: Add `InstructionData::write_to` implementation ([#2733](https://github.com/solana-foundation/anchor/pull/2733)).
- lang: Add `#[interface(..)]` attribute for instruction discriminator overrides ([#2728](https://github.com/solana-foundation/anchor/pull/2728)).
- ts: Add `.interface(..)` method for instruction discriminator overrides ([#2728](https://github.com/solana-foundation/anchor/pull/2728)).
- cli: Check `anchor-lang` and CLI version compatibility ([#2753](https://github.com/solana-foundation/anchor/pull/2753)).
- ts: Add missing IDL PDA seed types ([#2752](https://github.com/solana-foundation/anchor/pull/2752)).
- cli: `idl close` accepts optional `--idl-address` parameter ([#2760](https://github.com/solana-foundation/anchor/pull/2760)).
- cli: Add support for simple wildcard patterns in Anchor.toml's `workspace.members` and `workspace.exclude`. ([#2785](https://github.com/solana-foundation/anchor/pull/2785)).
- cli: Add `--test-template` option for `init` command ([#2805](https://github.com/solana-foundation/anchor/pull/2805)).
- cli: `anchor test` is able to run multiple commands ([#2799](https://github.com/solana-foundation/anchor/pull/2799)).
- cli: Check `@coral-xyz/anchor` package and CLI version compatibility ([#2813](https://github.com/solana-foundation/anchor/pull/2813)).
- cli: Accept package name as program name ([#2816](https://github.com/solana-foundation/anchor/pull/2816)).
- cli: Add ability to build and test only a specified program ([#2823](https://github.com/solana-foundation/anchor/pull/2823)).
- idl: Add new IDL spec ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- idl: Add support for `repr`s ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- idl: Add support for expression evaluation ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- idl: Add support for using external types when generating the IDL ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- idl, ts: Add unit and tuple struct support ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- idl, ts: Add generics support ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- ts: Add `accountsPartial` method to keep the old `accounts` method behavior ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- ts: Make `opts` parameter of `AnchorProvider` constructor optional ([#2843](https://github.com/solana-foundation/anchor/pull/2843)).
- cli: Add `--no-idl` flag to the `build` command ([#2847](https://github.com/solana-foundation/anchor/pull/2847)).
- cli: Add priority fees to idl commands ([#2845](https://github.com/solana-foundation/anchor/pull/2845)).
- ts: Add `prepend` option to MethodBuilder `preInstructions` method ([#2863](https://github.com/solana-foundation/anchor/pull/2863)).
- lang: Add `declare_program!` macro ([#2857](https://github.com/solana-foundation/anchor/pull/2857)).
- cli: Add `deactivate_feature` flag to `solana-test-validator` config in Anchor.toml ([#2872](https://github.com/solana-foundation/anchor/pull/2872)).
- idl: Add `docs` field for constants ([#2887](https://github.com/solana-foundation/anchor/pull/2887)).
- idl: Store deployment addresses for other clusters ([#2892](https://github.com/solana-foundation/anchor/pull/2892)).
- lang: Add `Event` utility type to get events from bytes ([#2897](https://github.com/solana-foundation/anchor/pull/2897)).
- lang, spl: Add support for [token extensions](https://solana.com/solutions/token-extensions) ([#2789](https://github.com/solana-foundation/anchor/pull/2789)).
- lang: Return overflow error from `Lamports` trait operations ([#2907](https://github.com/solana-foundation/anchor/pull/2907)).
- lang: Add composite accounts support for `declare_program!` ([#2894](https://github.com/solana-foundation/anchor/pull/2894)).
- client: Expose more parts from logs parsing ([#2707](https://github.com/solana-foundation/anchor/pull/2707)).

### Fixes

- syn: Add missing `new_from_array` method to `Hash` ([#2682](https://github.com/solana-foundation/anchor/pull/2682)).
- cli: Switch to Cargo feature resolver(`resolver = "2"`) ([#2676](https://github.com/solana-foundation/anchor/pull/2676)).
- cli: Fix using user specific path for `provider.wallet` in `Anchor.toml` ([#2696](https://github.com/solana-foundation/anchor/pull/2696)).
- syn: Fix IDL constant seeds parsing ([#2699](https://github.com/solana-foundation/anchor/pull/2699)).
- cli: Display errors if toolchain override restoration fails ([#2700](https://github.com/solana-foundation/anchor/pull/2700)).
- cli: Fix commit based `anchor_version` override ([#2704](https://github.com/solana-foundation/anchor/pull/2704)).
- spl: Fix compilation with `shmem` feature enabled ([#2722](https://github.com/solana-foundation/anchor/pull/2722)).
- cli: Localhost default test validator address changes from `localhost` to `127.0.0.1`, NodeJS 17 IP resolution changes for IPv6 ([#2725](https://github.com/solana-foundation/anchor/pull/2725)).
- lang: Eliminate temporary Vec allocations when serializing data with discriminant and set the default capacity to 256 bytes ([#2691](https://github.com/solana-foundation/anchor/pull/2691)).
- lang: Allow custom lifetime in Accounts structure ([#2741](https://github.com/solana-foundation/anchor/pull/2741)).
- lang: Remove `try_to_vec` usage while setting the return data in order to reduce heap memory usage ([#2744](https://github.com/solana-foundation/anchor/pull/2744))
- cli: Show installation progress if Solana tools are not installed when using toolchain overrides ([#2757](https://github.com/solana-foundation/anchor/pull/2757)).
- ts: Fix formatting enums ([#2763](https://github.com/solana-foundation/anchor/pull/2763)).
- cli: Fix `migrate` command not working without global `ts-node` installation ([#2767](https://github.com/solana-foundation/anchor/pull/2767)).
- client, lang, spl, syn: Enable all features for docs.rs build ([#2774](https://github.com/solana-foundation/anchor/pull/2774)).
- ts: Fix construction of field layouts for type aliased instruction arguments ([#2821](https://github.com/solana-foundation/anchor/pull/2821))
- idl: Fix IDL ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- idl, ts: Make casing consistent ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- ts: Fix not being able to use numbers in instruction, account, or event names in some cases due to case conversion ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- cli: Fix excessive test validator requests ([#2828](https://github.com/solana-foundation/anchor/pull/2828)).
- client: Fix `parse_logs_response` to prevent panics when more than 1 outer instruction exists in logs ([#2856](https://github.com/solana-foundation/anchor/pull/2856)).
- avm, cli: Fix `stdsimd` feature compilation error from `ahash` when installing the CLI using newer Rust versions ([#2867](https://github.com/solana-foundation/anchor/pull/2867)).
- spl: Fix not being able to deserialize newer token 2022 extensions ([#2876](https://github.com/solana-foundation/anchor/pull/2876)).
- client: Fix erroneous Cluster websocket ports ([#2690](https://github.com/solana-foundation/anchor/pull/2690)).
- spl: Remove `solana-program` dependency ([#2900](https://github.com/solana-foundation/anchor/pull/2900)).
- spl: Make `TokenAccount` and ` Mint` `Copy` ([#2904](https://github.com/solana-foundation/anchor/pull/2904)).
- ts: Add missing errors ([#2906](https://github.com/solana-foundation/anchor/pull/2906)).

### Breaking

- cli: Make `cargo build-sbf` the default build command ([#2694](https://github.com/solana-foundation/anchor/pull/2694)).
- cli: Require explicit `overflow-checks` flag ([#2716](https://github.com/solana-foundation/anchor/pull/2716)).
- ts: Remove `anchor-deprecated-state` feature ([#2717](https://github.com/solana-foundation/anchor/pull/2717)).
- lang: Remove `CLOSED_ACCOUNT_DISCRIMINATOR` ([#2726](https://github.com/solana-foundation/anchor/pull/2726)).
- lang: Make bumps of optional accounts `Option<u8>` rather than `u8` ([#2730](https://github.com/solana-foundation/anchor/pull/2730)).
- spl: Remove `shared-memory` program ([#2747](https://github.com/solana-foundation/anchor/pull/2747)).
- ts: Remove `associated`, `account.associated` and `account.associatedAddress` methods ([#2749](https://github.com/solana-foundation/anchor/pull/2749)).
- cli: `idl upgrade` command closes the IDL buffer account ([#2760](https://github.com/solana-foundation/anchor/pull/2760)).
- cli: Remove `--jest` option from the `init` command ([#2805](https://github.com/solana-foundation/anchor/pull/2805)).
- cli: Require `idl-build` feature in program `Cargo.toml` ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- cli: Rename `seeds` feature to `resolution` and make it enabled by default ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- cli: Remove `idl parse` command ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- idl: Change IDL spec ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- syn: Remove `idl-parse` and `seeds` features ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- ts: Change `accounts` method to no longer accept resolvable accounts ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- ts: `Program` instances use camelCase for everything ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- ts: Remove discriminator functions ([#2824](https://github.com/solana-foundation/anchor/pull/2824)).
- ts: Remove `programId` parameter of the `Program` constructor ([#2864](https://github.com/solana-foundation/anchor/pull/2864)).
- idl, syn: Move IDL types from the `anchor-syn` crate to the new IDL crate ([#2882](https://github.com/solana-foundation/anchor/pull/2882)).
- idl: Add `#[non_exhaustive]` to IDL enums ([#2890](https://github.com/solana-foundation/anchor/pull/2890)).
- idl: Rename IDL crate from `anchor-idl` to `anchor-lang-idl` ([#2908](https://github.com/solana-foundation/anchor/pull/2908)).
- deps: Upgrade Solana to 1.18.8 ([#2867](https://github.com/solana-foundation/anchor/pull/2867)).

## [0.29.0] - 2023-10-16

See the [Anchor 0.29 release notes](https://www.anchor-lang.com/release-notes/0.29.0) for a high-level overview of how to update.

### Features

- lang: Change all accounts to have a reference to `AccountInfo` ([#2656](https://github.com/solana-foundation/anchor/pull/2656)).
- lang: Add `get_lamports`, `add_lamports` and `sub_lamports` methods for all account types ([#2552](https://github.com/solana-foundation/anchor/pull/2552)).
- client: Add a helper struct `DynSigner` to simplify use of `Client<C> where <C: Clone + Deref<Target = impl Signer>>` with Solana clap CLI utils that loads `Signer` as `Box<dyn Signer>` ([#2550](https://github.com/solana-foundation/anchor/pull/2550)).
- lang: Allow CPI calls matching an interface without pinning program ID ([#2559](https://github.com/solana-foundation/anchor/pull/2559)).
- cli, lang: Add IDL generation through compilation. `anchor build` still uses parsing method to generate IDLs, use `anchor idl build` to generate IDLs with the build method ([#2011](https://github.com/solana-foundation/anchor/pull/2011)).
- avm: Add support for the `.anchorversion` file to facilitate switching between different versions of the `anchor-cli` ([#2553](https://github.com/solana-foundation/anchor/pull/2553)).
- ts: Add ability to access workspace programs independent of the casing used, e.g. `anchor.workspace.myProgram`, `anchor.workspace.MyProgram`... ([#2579](https://github.com/solana-foundation/anchor/pull/2579)).
- bench: Add benchmarking for program binary size ([#2591](https://github.com/solana-foundation/anchor/pull/2591)).
- spl: Export `mpl-token-metadata` crate ([#2583](https://github.com/solana-foundation/anchor/pull/2583)).
- spl: Add `TokenRecordAccount` for pNFTs ([#2597](https://github.com/solana-foundation/anchor/pull/2597)).
- ts: Add support for unnamed(tuple) enum in accounts ([#2601](https://github.com/solana-foundation/anchor/pull/2601)).
- cli: Add program template with multiple files for instructions, state... ([#2602](https://github.com/solana-foundation/anchor/pull/2602)).
- bench: Add benchmarking for stack memory usage ([#2617](https://github.com/solana-foundation/anchor/pull/2617)).
- lang: `Box` the inner enums of `anchor_lang::error::Error` to optimize `anchor_lang::Result` ([#2600](https://github.com/solana-foundation/anchor/pull/2600)).
- ts: Add strong type support for `Program.addEventListener` method ([#2627](https://github.com/solana-foundation/anchor/pull/2627)).
- syn: Add `IdlBuild` trait to implement IDL support for custom types ([#2629](https://github.com/solana-foundation/anchor/pull/2629)).
- spl: Add `idl-build` feature. IDL build method will not work without enabling this feature when using `anchor-spl` ([#2629](https://github.com/solana-foundation/anchor/pull/2629)).
- lang: Add support for type aliases in IDLs ([#2637](https://github.com/solana-foundation/anchor/pull/2637)).
- cli: Add `test.upgradeable`, `test.genesis.upgradeable` setting in `Anchor.toml` to support testing upgradeable programs ([#2642](https://github.com/solana-foundation/anchor/pull/2642)).
- cli, client, lang, spl: Update Solana toolchain and dependencies to `1.17.0`, `1.16` remains supported ([#2645](https://github.com/solana-foundation/anchor/pull/2645)).
- spl: Add support for memo program ([#2661](https://github.com/solana-foundation/anchor/pull/2661)).
- avm: Add `anchor-cli` installation from commit ([#2659](https://github.com/solana-foundation/anchor/pull/2659)).
- cli: Add `toolchain` property in `Anchor.toml` to override Anchor and Solana versions ([#2649](https://github.com/solana-foundation/anchor/pull/2649)).

### Fixes

- ts: Packages no longer depend on `assert` ([#2535](https://github.com/solana-foundation/anchor/pull/2535)).
- lang: Support for `const` in the `InitSpace` macro ([#2555](https://github.com/solana-foundation/anchor/pull/2555)).
- cli: Support workspace inheritance ([#2570](https://github.com/solana-foundation/anchor/pull/2570)).
- client: Compile with Solana `1.14` ([#2572](https://github.com/solana-foundation/anchor/pull/2572)).
- cli: Fix `anchor build --no-docs` adding docs to the IDL ([#2575](https://github.com/solana-foundation/anchor/pull/2575)).
- ts: Load workspace programs on-demand rather than loading all of them at once ([#2579](https://github.com/solana-foundation/anchor/pull/2579)).
- lang: Fix `associated_token::token_program` constraint ([#2603](https://github.com/solana-foundation/anchor/pull/2603)).
- cli: Fix `anchor account` command panicking outside of workspace ([#2620](https://github.com/solana-foundation/anchor/pull/2620)).
- lang: IDL named enum variant fields are now camelCase as opposed to snake_case, consistent with the other IDL types ([#2633](https://github.com/solana-foundation/anchor/pull/2633)).
- avm: Remove excessive panics and handle the errors gracefully ([#2671](https://github.com/solana-foundation/anchor/pull/2671)).

### Breaking

- lang: Switch to type safe bumps in context ([#2542](https://github.com/solana-foundation/anchor/pull/2542)).
- syn: `idl` feature has been replaced with `idl-build`, `idl-parse` and `idl-types` features ([#2011](https://github.com/solana-foundation/anchor/pull/2011)).
- syn: IDL `parse` method now returns `Result<Idl>` instead of `Result<Option<Idl>>` ([#2582](https://github.com/solana-foundation/anchor/pull/2582)).
- spl: Update `mpl-token-metadata` dependency to use the client SDK instead of the program crate ([#2632](https://github.com/solana-foundation/anchor/pull/2632)).
- ts: Remove `base64-js` dependency ([#2635](https://github.com/solana-foundation/anchor/pull/2635)).
- syn: `IdlTypeDefinitionTy` enum has a new variant `Alias` ([#2637](https://github.com/solana-foundation/anchor/pull/2637)).
- cli, client, lang, spl: Solana `1.14` is no longer supported, minimum required Solana version is `1.16.0` ([#2645](https://github.com/solana-foundation/anchor/pull/2645)).
- cli: `anchor_version` and `solana_version` property in `Anchor.toml` that was being used in verifiable builds are moved inside `toolchain`. They are now being used for all commands in the workspace, not just verifiable builds ([#2649](https://github.com/solana-foundation/anchor/pull/2649)).

## [0.28.0] - 2023-06-09

### Features

- client: Add `async` feature flag to use an asynchronous anchor-client ([#2488](https://github.com/solana-foundation/anchor/pull/2488)).
- spl: Add metadata wrappers `approve_collection_authority`, `bubblegum_set_collection_size`, `burn_edition_nft`, `burn_nft`, `revoke_collection_authority`, `set_token_standard`, `utilize`, `unverify_sized_collection_item`, `unverify_collection` ([#2430](https://github.com/solana-foundation/anchor/pull/2430))
- spl: Add `token_program` constraint to `Token`, `Mint`, and `AssociatedToken` accounts in order to override required `token_program` fields and use different token interface implementations in the same instruction ([#2460](https://github.com/solana-foundation/anchor/pull/2460))
- cli: Add support for Solidity programs. `anchor init` and `anchor new` take an option `--solidity` which creates solidity code rather than rust. `anchor build` and `anchor test` work accordingly ([#2421](https://github.com/solana-foundation/anchor/pull/2421))
- bench: Add benchmarking for compute units usage ([#2466](https://github.com/solana-foundation/anchor/pull/2466))
- cli: `idl set-buffer`, `idl set-authority` and `idl close` take an option `--print-only`. which prints transaction in a base64 Borsh compatible format but not sent to the cluster. It's helpful when managing authority under a multisig, e.g., a user can create a proposal for a `Custom Instruction` in SPL Governance ([#2486](https://github.com/solana-foundation/anchor/pull/2486)).
- lang: Add `emit_cpi!` and `#[event_cpi]` macros(behind `event-cpi` feature flag) to store event logs in transaction metadata ([#2438](https://github.com/solana-foundation/anchor/pull/2438)).
- cli: Add `keys sync` command to sync program id declarations ([#2505](https://github.com/solana-foundation/anchor/pull/2505)).
- cli: Create new programs with correct program ids ([#2509](https://github.com/solana-foundation/anchor/pull/2509)).
- cli, client, lang, spl: Update Solana toolchain and dependencies to `1.16.0` and specify maximum version of `<1.17.0` ([#2512](https://github.com/solana-foundation/anchor/pull/2512)).
- cli: `anchor deploy` command's `--program-name` argument accepts program lib names ([#2519](https://github.com/solana-foundation/anchor/pull/2519)).

### Fixes

- ts: Narrowed `AccountClient` type to it's appropriate account type ([#2440](https://github.com/solana-foundation/anchor/pull/2440))
- lang: Fix inability to use identifiers `program_id`, `accounts`, `ix_data`, `remaining_accounts` in instruction arguments ([#2464](https://github.com/solana-foundation/anchor/pull/2464))
- cli: Fix incorrect `metadata.address` generation in IDL after deploying with a custom keypair ([#2485](https://github.com/solana-foundation/anchor/pull/2485))
- cli: IDL commands no longer hang when the payer doesn't have funds to pay for the transaction fee ([#2492](https://github.com/solana-foundation/anchor/pull/2492))
- cli: Fix `anchor new` not updating `Anchor.toml` ([#2516](https://github.com/solana-foundation/anchor/pull/2516)).
- client, lang, spl: Allow wider range of dependency versions to reduce dependency issues ([#2524](https://github.com/solana-foundation/anchor/pull/2524)).

### Breaking

- lang: Identifiers that are intended for internal usage(`program_id`, `accounts`, `ix_data`, `remaining_accounts`) have been renamed with `__` prefix ([#2464](https://github.com/solana-foundation/anchor/pull/2464))
- spl: Remove the `metadata::create_metadata_account_v2` deprecated wrapper since it was removed from token metadata program ([#2480](https://github.com/solana-foundation/anchor/pull/2480))

## [0.27.0] - 2023-03-08

### Features

- spl: Add `MasterEditionAccount` account deserialization to spl metadata ([#2393](https://github.com/solana-foundation/anchor/pull/2393)).
- lang: Add the `InitSpace` derive macro to automatically calculate the space at the initialization of an account ([#2346](https://github.com/solana-foundation/anchor/pull/2346)).
- cli: Add `env` option to verifiable builds ([#2325](https://github.com/solana-foundation/anchor/pull/2325)).
- cli: Add `idl close` command to close a program's IDL account ([#2329](https://github.com/solana-foundation/anchor/pull/2329)).
- cli: `idl init` now supports very large IDL files ([#2329](https://github.com/solana-foundation/anchor/pull/2329)).
- spl: Add `transfer_checked` function ([#2353](https://github.com/solana-foundation/anchor/pull/2353)).
- spl: Add `approve_checked` function ([#2401](https://github.com/solana-foundation/anchor/pull/2401)).
- cli: Add `--skip-build` option to the verify command ([#2387](https://github.com/solana-foundation/anchor/pull/2387)).
- client: Add support for multithreading to the rust client: use flag `--multithreaded` ([#2321](https://github.com/solana-foundation/anchor/pull/2321)).
- client: Add `async_rpc` a method which returns a nonblocking solana rpc client ([#2322](https://github.com/solana-foundation/anchor/pull/2322)).
- avm, cli: Use the `rustls-tls` feature of `reqwest` so that users don't need OpenSSL installed ([#2385](https://github.com/solana-foundation/anchor/pull/2385)).
- ts: Add `VersionedTransaction` support. Methods in the `Provider` class and `Wallet` interface now use the argument `tx: Transaction | VersionedTransaction` ([#2427](https://github.com/solana-foundation/anchor/pull/2427)).
- cli: Add `--arch sbf` option to compile programs using `cargo build-sbf` ([#2398](https://github.com/solana-foundation/anchor/pull/2398)).
- land: Support multiple programs with the same interface using `Interface` and `InterfaceAccount` types, related to token-2022 ([#2386](https://github.com/solana-foundation/anchor/pull/2386)).

### Fixes

- ts: Make the return type of `AccountClient.fetchMultiple` match the account type being fetched ([#2390](https://github.com/solana-foundation/anchor/pull/2390))
- cli: Don't regenerate idl in read_all_programs(). ([#2332](https://github.com/solana-foundation/anchor/pull/2332)).
- ts: `provider.simulate` will send the transaction with `sigVerify: false` if no `signers` are present ([#2331](https://github.com/solana-foundation/anchor/pull/2331)).
- cli: Failing commands will return the correct exit status. ([#2370](https://github.com/solana-foundation/anchor/pull/2370)).
- idl: Update the IDL program to use non-deprecated account types ([#2365](https://github.com/solana-foundation/anchor/pull/2365)).
- ts: Enum fields weren't being converted from snake_case to camelCase ([#2378](https://github.com/solana-foundation/anchor/pull/2378)).
- lang/cli: Update to solana-program version 1.14.16 and rust version 1.60, appears to still be incompatible with 1.15 CLI ([#2420](https://github.com/solana-foundation/anchor/pull/2420)).

### Breaking

- lang: Remove deprecated account types: `CpiAccount`, `Loader` and `ProgramAccount` ([#2375](https://github.com/solana-foundation/anchor/pull/2375)).
- lang: Remove `state` and `interface` attributes ([#2285](https://github.com/solana-foundation/anchor/pull/2285)).
- lang: Remove deprecated literal constraint which has been replaced by `#[account(constraint = {})]` ([#2379](https://github.com/solana-foundation/anchor/pull/2379)).
- lang: `account(zero_copy)` and `zero_copy` attributes now derive the `bytemuck::Pod` and `bytemuck::Zeroable` traits instead of using `unsafe impl` ([#2330](https://github.com/solana-foundation/anchor/pull/2330)). This imposes useful restrictions on the type, like not having padding bytes and all fields being `Pod` themselves. See [bytemuck::Pod](https://docs.rs/bytemuck/latest/bytemuck/trait.Pod.html) for details. This change requires adding `bytemuck = { version = "1.4.0", features = ["derive", "min_const_generics"]}` to your `cargo.toml`. Legacy applications can still use `#[account(zero_copy(unsafe))]` and `#[zero_copy(unsafe)]` for the old behavior.
- ts: Remove `createProgramAddressSync`, `findProgramAddressSync` (now available in `@solana/web3.js`) and update `associatedAddress` to be synchronous ([#2357](https://github.com/solana-foundation/anchor/pull/2357)).

## [0.26.0] - 2022-12-15

### Features

- cli: Add `--run` to `anchor test` for running a subset of test suites ([#1828](https://github.com/solana-foundation/anchor/issues/1828)).
- client: Add `transaction` functions to RequestBuilder ([#1958](https://github.com/solana-foundation/anchor/pull/1958)).
- spl: Add `create_metadata_accounts_v3` and `set_collection_size` wrappers ([#2119](https://github.com/solana-foundation/anchor/pull/2119)).
- spl: Add `MetadataAccount` account deserialization. ([#2014](https://github.com/solana-foundation/anchor/pull/2014)).
- spl: Add `update_primary_sale_happened_via_token` wrapper ([#2173](https://github.com/solana-foundation/anchor/pull/2173)).
- spl: Add `sign_metadata` and `remove_creator_verification` wrappers ([#2175](https://github.com/solana-foundation/anchor/pull/2175)).
- spl: Add `initialize_account3` and `initialize_mint2` ([#2265](https://github.com/solana-foundation/anchor/pull/2265)).
- spl: Change `serum-dex` to `openbook-dex` ([#2308](https://github.com/solana-foundation/anchor/pull/2308)).
- lang: Add parsing for consts from impl blocks for IDL PDA seeds generation ([#2128](https://github.com/solana-foundation/anchor/pull/2128)).
- lang: Account closing reassigns to system program and reallocates ([#2169](https://github.com/solana-foundation/anchor/pull/2169)).
- ts: Add coders for SPL programs ([#2143](https://github.com/solana-foundation/anchor/pull/2143)).
- ts: Add `has_one` relations inference so accounts mapped via has_one relationships no longer need to be provided ([#2160](https://github.com/solana-foundation/anchor/pull/2160)).
- ts: Add ability to set args after setting accounts and retrieving pubkeys ([#2160](https://github.com/solana-foundation/anchor/pull/2160)).
- ts: Add `.prepare()` to builder pattern ([#2160](https://github.com/solana-foundation/anchor/pull/2160)).
- spl: Add `freeze_delegated_account` and `thaw_delegated_account` wrappers ([#2164](https://github.com/solana-foundation/anchor/pull/2164)).
- ts: Add `feePayer` check to `AnchorProvider` methods, so that anchor writes the provider's wallet as fee payer if fee payer isn't already set ([#2186](https://github.com/solana-foundation/anchor/pull/2186)).
- ts: Add nested PDA inference ([#2194](https://github.com/solana-foundation/anchor/pull/2194)).
- ts: Add ability to resolve missing accounts with a custom resolver ([#2194](https://github.com/solana-foundation/anchor/pull/2194)).
- ts: Update the Solana web3 library used by anchor ts to version 1.64.0 ([#2220](https://github.com/solana-foundation/anchor/issues/2220)).
- lang: Updates `AccountsClose` to make it safe to call manually ([#2209](https://github.com/solana-foundation/anchor/pull/2209)).
- lang: Update rust used in the repo version 1.62 ([#2272](https://github.com/solana-foundation/anchor/pull/2272)).
- cli: Allow custom cluster config ([#2271](https://github.com/solana-foundation/anchor/pull/2271)).
- ts: Add optional flag to parseLogs to throw an error on decoding failure ([#2043](https://github.com/solana-foundation/anchor/pull/2043)).
- cli: Add `test.validator.geyser_plugin_config` support ([#2016](https://github.com/solana-foundation/anchor/pull/2016)).
- cli: Add `account` subcommand to cli ([#1923](https://github.com/solana-foundation/anchor/pull/1923))
- cli: Add `ticks_per_slot` option to Validator args ([#1875](https://github.com/solana-foundation/anchor/pull/1875)).

### Fixes

- lang: Fix parsing for bytes literals in the IDL ([#2261](https://github.com/solana-foundation/anchor/pull/2261)).
- lang: Fix IDL `seed` generation for byte string literals ([#2125](https://github.com/solana-foundation/anchor/pull/2125)).
- ts: Update seeds inference to allow nested user defined structs within the seeds ([#2198](https://github.com/solana-foundation/anchor/pull/2198)).
- event: Fix multiple event listeners with the same name ([#2165](https://github.com/solana-foundation/anchor/pull/2165)).
- lang: Prevent the payer account from being initialized as a program account ([#2284](https://github.com/solana-foundation/anchor/pull/2284)).
- ts: Fixing breaking change where null or undefined wallet throws an error ([#2303](https://github.com/solana-foundation/anchor/pull/2303)).
- ts: Fixed `.fetchNullable()` to be robust towards accounts only holding a balance ([#2301](https://github.com/solana-foundation/anchor/pull/2301)).
- lang: Only add public enums to the IDL ([#2309](https://github.com/solana-foundation/anchor/pull/2309)).
- lang: Fix heap intensive error mapping ([#2313](https://github.com/solana-foundation/anchor/pull/2313)).

### Breaking

- ts: SPL coders have been removed from the main Anchor package. ([#2155](https://github.com/solana-foundation/anchor/pull/2155))
- lang: Remove `rent` from constraints ([#2265](https://github.com/solana-foundation/anchor/pull/2265)).
- spl: Remove `rent` from `associated_token::Create` ([#2265](https://github.com/solana-foundation/anchor/pull/2265)).
- lang: Add `Discriminator` and `Owner` trait implementation for structures representing instructions ([#1997](https://github.com/solana-foundation/anchor/pull/1997)).
- ts: '@coral-xyz/borsh' package is now part of the yarn monorepo ([#2290](https://github.com/solana-foundation/anchor/pull/2290)). The borsh package needs to be built before the anchor package can be built but this should happen automatically when running `yarn build` in packages/anchor, see [#2299](https://github.com/solana-foundation/anchor/pull/2299) and [#2306](https://github.com/solana-foundation/anchor/pull/2306).
- lang: Add support for optionally passing in accounts using the syntax `Optional<Account<'info, T>>`. Shouldn't affect existing programs but may be a breaking change to tools that use the anchor generated IDL. [#2101](https://github.com/solana-foundation/anchor/pull/2101).
- ts: Switch from `@project-serum/anchor` to the `@coral-xyz/anchor` package [#2318](https://github.com/solana-foundation/anchor/pull/2318).

## [0.25.0] - 2022-07-05

### Features

- lang: Add `realloc`, `realloc::payer`, and `realloc::zero` as a new constraint group for program accounts ([#1986](https://github.com/solana-foundation/anchor/pull/1986)).
- lang: Add `PartialEq` and `Eq` for `anchor_lang::Error` ([#1544](https://github.com/solana-foundation/anchor/pull/1544)).
- cli: Add `--skip-build` to `anchor publish` ([#1786](https://github.com/solana-foundation/anchor/pull/1841)).
- cli: Add `--program-keypair` to `anchor deploy` ([#1786](https://github.com/solana-foundation/anchor/pull/1786)).
- cli: Add compilation optimizations to cli template ([#1807](https://github.com/solana-foundation/anchor/pull/1807)).
- cli: `build` now adds docs to idl. This can be turned off with `--no-docs` ([#1561](https://github.com/solana-foundation/anchor/pull/1561)).
- cli: Add `b` and `t` aliases for `build` and `test` respectively ([#1823](https://github.com/solana-foundation/anchor/pull/1823)).
- spl: Add more derived traits to `TokenAccount` to `Mint` ([#1818](https://github.com/solana-foundation/anchor/pull/1818)).
- spl: Add `sync_native` token program CPI wrapper function ([#1833](https://github.com/solana-foundation/anchor/pull/1833)).
- cli: Allow passing arguments to an underlying script with `anchor run` ([#1914](https://github.com/solana-foundation/anchor/pull/1914)).
- ts: Implement a coder for system program ([#1920](https://github.com/solana-foundation/anchor/pull/1920)).
- ts: Add `program.coder.types` for encoding/decoding user-defined types ([#1931](https://github.com/solana-foundation/anchor/pull/1931)).
- client: Add `send_with_spinner_and_config` function to RequestBuilder ([#1926](https://github.com/solana-foundation/anchor/pull/1926)).
- ts: Implement a coder for SPL associated token program ([#1939](https://github.com/solana-foundation/anchor/pull/1939)).
- ts: verbose error for missing `ANCHOR_WALLET` variable when using `NodeWallet.local()` ([#1958](https://github.com/solana-foundation/anchor/pull/1958)).
- ts: Add `MethodsBuilder#accountsStrict` for strict typing on ix account input ([#2019](https://github.com/solana-foundation/anchor/pull/2019)).
- Update solana dependencies to 1.10.29 ([#2027](https://github.com/solana-foundation/anchor/pull/2027)).

### Fixes

- cli: Fix `anchor keys list` reading the `target` folder in the wrong path ([#2063](https://github.com/solana-foundation/anchor/pull/2063)).
- cli: Move `overflow-checks` into workspace `Cargo.toml` so that it will not be ignored by compiler ([#1806](https://github.com/solana-foundation/anchor/pull/1806)).
- lang: Fix missing account name information when deserialization fails when using `init` or `zero` ([#1800](https://github.com/solana-foundation/anchor/pull/1800)).
- ts: Expose the wallet's publickey on the Provider ([#1845](https://github.com/solana-foundation/anchor/pull/1845)).

### Breaking

- ts: Change `BROWSER` env variable to `ANCHOR_BROWSER` ([#1233](https://github.com/solana-foundation/anchor/pull/1233)).
- ts: Add transaction signature to `EventCallback` parameters ([#1851](https://github.com/solana-foundation/anchor/pull/1851)).
- ts: Change `EventParser#parseLogs` implementation to be a generator instead of callback function ([#2018](https://github.com/solana-foundation/anchor/pull/2018)).
- lang: Adds a new `&mut reallocs: BTreeSet<Pubkey>` argument to `Accounts::try_accounts` ([#1986](https://github.com/solana-foundation/anchor/pull/1986)).

## [0.24.2] - 2022-04-13

### Fixes

- lang: Fix `returns` being serialized as `null` instead of `undefined` in IDL ([#1782](https://github.com/solana-foundation/anchor/pull/1782)).

## [0.24.1] - 2022-04-12

### Fixes

- lang: Fix `anchor build` failing if `Test.toml` included a relative path that didn't exist yet because it's created by `anchor build` ([#1772](https://github.com/solana-foundation/anchor/pull/1772)).
- cli: Update js/ts template to use new `AnchorProvider` class ([#1770](https://github.com/solana-foundation/anchor/pull/1770)).

## [0.24.0] - 2022-04-12

### Features

- lang: Add support for multiple test suites with separate local validators ([#1681](https://github.com/solana-foundation/anchor/pull/1681)).
- lang: Add return values to CPI client ([#1598](https://github.com/solana-foundation/anchor/pull/1598)).
- ts: Add view functions ([#1695](https://github.com/solana-foundation/anchor/pull/1695)).
- avm: New `avm update` command to update the Anchor CLI to the latest version ([#1670](https://github.com/solana-foundation/anchor/pull/1670)).
- cli: Update js/ts templates to use new `program.methods` syntax ([#1732](https://github.com/solana-foundation/anchor/pull/1732)).
- cli: Workspaces created with `anchor init` now come with the `prettier` formatter and scripts included ([#1741](https://github.com/solana-foundation/anchor/pull/1741)).
- ts: Add `pubkeys` function to methods builder to get all instruction account addresses ([#1733](https://github.com/solana-foundation/anchor/pull/1733)).
- ts: Export `LangErrorCode` and `LangErrorMessage` from `error.ts` ([#1756](https://github.com/solana-foundation/anchor/pull/1756)).

### Fixes

- avm: `avm install` no longer downloads the version if already installed in the machine ([#1670](https://github.com/solana-foundation/anchor/pull/1670)).
- cli: make `anchor test` fail when used with `--skip-deploy` option and without `--skip-local-validator` option but there already is a running validator ([#1675](https://github.com/solana-foundation/anchor/pull/1675)).
- lang: Return proper error instead of panicking if account length is smaller than discriminator in functions of `(Account)Loader` ([#1678](https://github.com/solana-foundation/anchor/pull/1678)).
- cli: Add `@types/bn.js` to `devDependencies` in cli template ([#1712](https://github.com/solana-foundation/anchor/pull/1712)).
- ts: Event listener no longer crashes on Program Upgrade or any other unexpected log ([#1757](https://github.com/solana-foundation/anchor/pull/1757)).

### Breaking

- avm: `avm install` switches to the newly installed version after installation finishes ([#1670](https://github.com/solana-foundation/anchor/pull/1670)).
- spl: Re-export the `spl_token` crate ([#1665](https://github.com/solana-foundation/anchor/pull/1665)).
- lang, cli, spl: Update solana toolchain to v1.9.13 ([#1653](https://github.com/solana-foundation/anchor/pull/1653) and [#1751](https://github.com/solana-foundation/anchor/pull/1751)).
- lang: `Program` type now deserializes `programdata_address` only on demand ([#1723](https://github.com/solana-foundation/anchor/pull/1723)).
- ts: Make `Provider` an interface and adjust its signatures and add `AnchorProvider` implementer class ([#1707](https://github.com/solana-foundation/anchor/pull/1707)).
- spl: Change "to" to "from" in `token::burn` ([#1080](https://github.com/solana-foundation/anchor/pull/1080)).

## [0.23.0] - 2022-03-20

### Features

- cli: Add `anchor clean` command that's the same as `cargo clean` but preserves keypairs inside `target/deploy` ([#1470](https://github.com/solana-foundation/anchor/issues/1470)).
- cli: Running `anchor init` now initializes a new git repository for the workspace. This can be disabled with the `--no-git` flag ([#1605](https://github.com/solana-foundation/anchor/pull/1605)).
- cli: Add support for `anchor idl fetch` to work outside anchor workspace ([#1509](https://github.com/solana-foundation/anchor/pull/1509)).
- cli: [[test.validator.clone]] also clones the program data account of programs owned by the bpf upgradeable loader ([#1481](https://github.com/solana-foundation/anchor/issues/1481)).
- lang: Add new `AccountSysvarMismatch` error code and test cases for sysvars ([#1535](https://github.com/solana-foundation/anchor/pull/1535)).
- lang: Replace `std::io::Cursor` with a custom `Write` impl that uses the Solana mem syscalls ([#1589](https://github.com/solana-foundation/anchor/pull/1589)).
- lang: Add `require_neq`, `require_keys_neq`, `require_gt`, and `require_gte` comparison macros ([#1622](https://github.com/solana-foundation/anchor/pull/1622)).
- lang: Handle arrays with const as size in instruction data ([#1623](https://github.com/solana-foundation/anchor/issues/1623).
- spl: Add support for revoke instruction ([#1493](https://github.com/solana-foundation/anchor/pull/1493)).
- ts: Add provider parameter to `Spl.token` factory method ([#1597](https://github.com/solana-foundation/anchor/pull/1597)).

### Fixes

- ts: Fix the loss of strict typing using the `methods` namespace on builder functions ([#1539](https://github.com/solana-foundation/anchor/pull/1539)).
- spl: Update `spl/governance` to use new errors ([#1582](https://github.com/solana-foundation/anchor/pull/1582)).
- client: Fix `Cluster`'s `FromStr` implementation ([#1362](https://github.com/solana-foundation/anchor/pull/1362)).
- lang: Implement `Key` for `Pubkey` again, so `associated_token::*` constraints can use pubkey targets again ([#1601](https://github.com/solana-foundation/anchor/pull/1601)).
- lang: Adjust error code so `#[error_code]` works with just importing `anchor_lang::error_code` ([#1610](https://github.com/solana-foundation/anchor/pull/1610)).
- ts: Fix `spl-token` coder account parsing ([#1604](https://github.com/solana-foundation/anchor/pull/1604)).
- cli: Fix `npm install` fallback if `yarn` install doesn't work ([#1643](https://github.com/solana-foundation/anchor/pull/1643)).
- lang: Fix bug where `owner = <target>` would not compile because of missing type annotation ([#1648](https://github.com/solana-foundation/anchor/pull/1648)).
- ts: Adjust `send` and `simulate` functions in `provider.ts`, so they use the return value of `Wallet.signTransaction`([#1527](https://github.com/solana-foundation/anchor/pull/1527)).

### Breaking

- ts: Mark `transaction`, `instruction`, `simulate` and `rpc` program namespaces as deprecated in favor of `methods` ([#1539](https://github.com/solana-foundation/anchor/pull/1539)).
- ts: No longer allow manual setting of globally resolvable program public keys in `methods#accounts()`. ([#1548][https://github.com/solana-foundation/anchor/pull/1548])
- lang/ts: Events are now emitted using the `sol_log_data` syscall ([#1608](https://github.com/solana-foundation/anchor/pull/1608)).
- lang: Remove space calculation using `#[derive(Default)]` ([#1519](https://github.com/solana-foundation/anchor/pull/1519)).
- lang: Add support for logging expected and actual values and pubkeys. Add `require_eq` and `require_keys_eq` macros. Add default error code to `require` macro ([#1572](https://github.com/solana-foundation/anchor/pull/1572)).
- lang: Add `system_program` CPI wrapper functions. Make `system_program` module public instead of re-exporting `system_program::System`([#1629](https://github.com/solana-foundation/anchor/pull/1629)).
- cli: `avm use` no long prompts [y/n] if an install is needed first - it just tells the user to `avm install` ([#1565](https://github.com/solana-foundation/anchor/pull/1565))
- ts: Add `AnchorError` with program stack and also a program stack for non-`AnchorError` errors ([#1640](https://github.com/solana-foundation/anchor/pull/1640)). `AnchorError` is not returned for `processed` tx that have `skipPreflight` set to `true` (it falls back to `ProgramError` or the raw solana library error).

## [0.22.1] - 2022-02-28

### Fixes

- cli: Fix rust template ([#1488](https://github.com/solana-foundation/anchor/pull/1488)).
- lang: Handle array sizes with variable sizes in events and array size casting in IDL parsing ([#1485](https://github.com/solana-foundation/anchor/pull/1485))

## [0.22.0] - 2022-02-20

### Features

- lang: Add check that declared id == program id ([#1451](https://github.com/solana-foundation/anchor/pull/1451)).
- ts: Added float types support ([#1425](https://github.com/solana-foundation/anchor/pull/1425)).
- cli: Add `--skip-lint` option to disable check linting introduced in ([#1452](https://github.com/solana-foundation/anchor/pull/1452)) for rapid prototyping ([#1482](https://github.com/solana-foundation/anchor/pull/1482)).

### Fixes

- ts: Allow nullable types for `Option<T>` mapped types ([#1428](https://github.com/solana-foundation/anchor/pull/1428)).

### Breaking

- lang: Enforce that the payer for an init-ed account be marked `mut` ([#1271](https://github.com/solana-foundation/anchor/pull/1271)).
- lang: All error-related code is now in the error module ([#1426](https://github.com/solana-foundation/anchor/pull/1426)).
- lang: Require doc comments when using AccountInfo or UncheckedAccount types ([#1452](https://github.com/solana-foundation/anchor/pull/1452)).
- lang: add [`error!`](https://docs.rs/anchor-lang/latest/anchor_lang/prelude/macro.error.html) and [`err!`](https://docs.rs/anchor-lang/latest/anchor_lang/prelude/macro.err.html) macro and `Result` type ([#1462](https://github.com/solana-foundation/anchor/pull/1462)).
  This change will break most programs. Do the following to upgrade:
  _ change all `ProgramResult`'s to `Result<()>`
  _ change `#[error]` to `#[error_code]`
  _ change all `Err(MyError::SomeError.into())` to `Err(error!(MyError::SomeError))` and all `Err(ProgramError::SomeProgramError)` to `Err(ProgramError::SomeProgramError.into())` or `Err(Error::from(ProgramError::SomeProgramError).with_source(source!()))` to provide file and line source of the error (`with_source` is most useful with `ProgramError`s. `error!` already adds source information for custom and anchor internal errors).
  _ change all `solana_program::program::invoke()` to `solana_program::program::invoke().map_err(Into::into)` and `solana_program::program::invoke_signed()` to `solana_program::program::invoke_signed().map_err(Into::into)`

## [0.21.0] - 2022-02-07

### Fixes

- ts: Fix the root type declaration of the `Wallet` / `NodeWallet` class ([#1363](https://github.com/solana-foundation/anchor/pull/1363)).
- ts: Improve type mapping of Account fields into Typescript with additional support for `Option<T>` and `Vec<String>` types ([#1393](https://github.com/solana-foundation/anchor/pull/1393)).

### Features

- lang: Add `seeds::program` constraint for specifying which program_id to use when deriving PDAs ([#1197](https://github.com/solana-foundation/anchor/pull/1197)).
- lang: `Context` now has a new `bumps: BTree<String, u8>` argument, mapping account name to bump seed "found" by the accounts context. This allows one to access bump seeds without having to pass them in from the client or recalculate them in the handler ([#1367](https://github.com/solana-foundation/anchor/pull/1367)).
- lang, ts: Automatically infer PDA addresses ([#1331](https://github.com/solana-foundation/anchor/pull/1331)).
- ts: Remove error logging in the event parser when log websocket encounters a program error ([#1313](https://github.com/solana-foundation/anchor/pull/1313)).
- ts: Add new `methods` namespace to the program client, introducing a more ergonomic builder API ([#1324](https://github.com/solana-foundation/anchor/pull/1324)).
- ts: Add registry utility for fetching the latest verified build ([#1371](https://github.com/solana-foundation/anchor/pull/1371)).
- cli: Expose the solana-test-validator --account flag in Anchor.toml via [[test.validator.account]] ([#1366](https://github.com/solana-foundation/anchor/pull/1366)).
- cli: Add avm, a tool for managing anchor-cli versions ([#1385](https://github.com/solana-foundation/anchor/pull/1385)).

### Breaking

- lang: Put `init_if_needed` behind a feature flag to decrease wrong usage ([#1258](https://github.com/solana-foundation/anchor/pull/1258)).
- lang: rename `loader_account` module to `account_loader` module ([#1279](https://github.com/solana-foundation/anchor/pull/1279))
- lang: The `Accounts` trait's `try_accounts` method now has an additional `bumps: &mut BTreeMap<String, u8>` argument, which accumulates bump seeds ([#1367](https://github.com/solana-foundation/anchor/pull/1367)).
- lang: Providing `bump = <target>` targets with `init` will now error. On `init` only, it is required to use `bump` without a target and access the seed inside function handlers via `ctx.bumps.get("<pda-account-name")`. For subsequent seeds constraints (without init), it is recommended to store the bump on your account and use it as a `bump = <target>` target to minimize compute units used ([#1380](https://github.com/solana-foundation/anchor/pull/1380)).
- ts: `Coder` is now an interface and the existing class has been renamed to `BorshCoder`. This change allows the generation of Anchor clients for non anchor programs ([#1259](https://github.com/solana-foundation/anchor/pull/1259/files)).
- cli: [[test.clone]] key in Anchor.toml is renamed to [[test.validator.clone]] ([#1366](https://github.com/solana-foundation/anchor/pull/1366)).

## [0.20.1] - 2022-01-09

### Fixes

- lang: Improved error msgs when required programs are missing when using the `init` constraint([#1257](https://github.com/solana-foundation/anchor/pull/1257))

### Features

- lang: Allow repr overrides for zero copy accounts ([#1273](https://github.com/solana-foundation/anchor/pull/1273)).

## [0.20.0] - 2022-01-06

### Fixes

- lang: `init_if_needed` now checks rent exemption when init is not needed ([#1250](https://github.com/solana-foundation/anchor/pull/1250)).
- lang: Add missing owner check when `associated_token::authority` is used ([#1240](https://github.com/solana-foundation/anchor/pull/1240)).
- ts: Add type declarations for conditional `workspace` and `Wallet` exports ([#1137](https://github.com/solana-foundation/anchor/pull/1137)).
- ts: Change commitment message `recent` to `processed` and `max` to `finalized` ([#1128](https://github.com/solana-foundation/anchor/pull/1128))
- ts: fix `translateAddress` which currently leads to failing browser code. Now uses `PublicKey` constructor instead of prototype chain constructor name checking which doesn't work in the presence of code minifying/mangling([#1138](https://github.com/solana-foundation/anchor/pull/1138))
- lang: add missing check that verifies that account is ATA when using `init_if_needed` and init is not needed([#1221](https://github.com/solana-foundation/anchor/pull/1221))

### Features

- lang: Add `programdata_address: Option<Pubkey>` field to `Program` account. Will be populated if account is a program owned by the upgradable bpf loader ([#1125](https://github.com/solana-foundation/anchor/pull/1125))
- lang,ts,ci,cli,docs: update solana toolchain to version 1.8.5([#1133](https://github.com/solana-foundation/anchor/pull/1133)).
- lang: Account wrappers for non-Anchor programs no longer have to implement the `serialize` function because it has a default impl now. Similarly, they no longer have to implement `try_deserialize` which now delegates to `try_deserialize_unchecked` by default([#1156](https://github.com/solana-foundation/anchor/pull/1156)).
- lang: Add `set_inner` method to `Account<'a, T>` to enable easy updates ([#1177](https://github.com/solana-foundation/anchor/pull/1177)).
- lang: Handle arrays with const as length ([#968](https://github.com/solana-foundation/anchor/pull/968)).
- ts: Add optional commitment argument to `fetch` and `fetchMultiple` ([#1171](https://github.com/solana-foundation/anchor/pull/1171)).
- lang: Implement `AsRef<T>` for `Account<'a, T>`([#1173](https://github.com/solana-foundation/anchor/pull/1173))
- cli: Add `anchor expand` command which wraps around `cargo expand` ([#1160](https://github.com/solana-foundation/anchor/pull/1160))

### Breaking

- client: Client::new and Client::new_with_options now accept `Rc<dyn Signer>` instead of `Keypair` ([#975](https://github.com/solana-foundation/anchor/pull/975)).
- lang, ts: Change error enum name and message for 'wrong program ownership' account validation ([#1154](https://github.com/solana-foundation/anchor/pull/1154)).
- lang: Change from `#[repr(packed)]` to `#[repr(C)]` for zero copy accounts ([#1106](https://github.com/solana-foundation/anchor/pull/1106)).
- lang: Account types can now be found either in the `prelude` module or the `accounts` module but not longer directly under the root.
  Deprecated account types are no longer imported by the prelude ([#1208](https://github.com/solana-foundation/anchor/pull/1208)).

## [0.19.0] - 2021-12-08

### Fixes

- lang: Add `deprecated` attribute to `ProgramAccount` ([#1014](https://github.com/solana-foundation/anchor/pull/1014)).
- cli: Add version number from programs `Cargo.toml` into extracted IDL ([#1061](https://github.com/solana-foundation/anchor/pull/1061)).
- lang: Add `deprecated` attribute to `Loader`([#1078](https://github.com/solana-foundation/anchor/pull/1078)).
- lang: the `init_if_needed` attribute now checks that given attributes (e.g. space, owner, token::authority etc.) are validated even when init is not needed ([#1096](https://github.com/solana-foundation/anchor/pull/1096)).

### Features

- lang: Add `ErrorCode::AccountNotInitialized` error to separate the situation when the account has the wrong owner from when it does not exist (#[1024](https://github.com/solana-foundation/anchor/pull/1024)).
- lang: Called instructions now log their name by default. This can be turned off with the `no-log-ix-name` flag ([#1057](https://github.com/solana-foundation/anchor/pull/1057)).
- lang: `ProgramData` and `UpgradableLoaderState` can now be passed into `Account` as generics. see [UpgradeableLoaderState](https://docs.rs/solana-program/latest/solana_program/bpf_loader_upgradeable/enum.UpgradeableLoaderState.html). `UpgradableLoaderState` can also be matched on to get `ProgramData`, but when `ProgramData` is used instead, anchor does the serialization and checking that it is actually program data for you ([#1095](https://github.com/solana-foundation/anchor/pull/1095)).
- ts: Add better error msgs in the ts client if something wrong (i.e. not a pubkey or a string) is passed in as an account in an instruction accounts object ([#1098](https://github.com/solana-foundation/anchor/pull/1098)).
- ts: Add inputs `postInstructions` and `preInstructions` as a replacement for (the now deprecated) `instructions` ([#1007](https://github.com/solana-foundation/anchor/pull/1007)).
- ts: Add `getAccountInfo` helper method to account namespace/client ([#1084](https://github.com/solana-foundation/anchor/pull/1084)).

### Breaking

- lang, ts: Error codes have been mapped to new numbers to allow for more errors per namespace ([#1096](https://github.com/solana-foundation/anchor/pull/1096)).

## [0.18.2] - 2021-11-14

- cli: Replace global JavaScript dependency installs with local.

### Features

- lang: Add `SystemAccount<'info>` account type for generic wallet addresses or accounts owned by the system program ([#954](https://github.com/solana-foundation/anchor/pull/954))

### Fixes

- cli: fix dns in NODE_OPTIONS ([#928](https://github.com/solana-foundation/anchor/pull/928)).
- cli: output TypeScript IDL in `idl parse` subcommand ([#941](https://github.com/solana-foundation/anchor/pull/941)).
- cli: Add fields `os` and `cpu` to npm package `@project-serum/anchor-cli` ([#976](https://github.com/solana-foundation/anchor/pull/976)).
- cli: Allow specify output directory for TypeScript IDL ([#940](https://github.com/solana-foundation/anchor/pull/940)).

### Breaking

- spl: Move permissioned markets into dex repository ([#962](https://github.com/solana-foundation/anchor/pull/962)).

## [0.18.0] - 2021-10-24

### Features

- cli: Add support for configuration options for `solana-test-validator` in Anchor.toml ([#834](https://github.com/solana-foundation/anchor/pull/834)).
- cli: `target/types` directory now created on build to store a TypeScript types file for each program's IDL ([#795](https://github.com/solana-foundation/anchor/pull/795)).
- ts: `Program<T>` can now be typed with an IDL type ([#795](https://github.com/solana-foundation/anchor/pull/795)).
- lang: Add `mint::freeze_authority` keyword for mint initialization within `#[derive(Accounts)]` ([#835](https://github.com/solana-foundation/anchor/pull/835)).
- lang: Add `AccountLoader` type for `zero_copy` accounts with support for CPI ([#792](https://github.com/solana-foundation/anchor/pull/792)).
- lang: Add `#[account(init_if_needed)]` keyword for allowing one to invoke the same instruction even if the account was created already ([#906](https://github.com/solana-foundation/anchor/pull/906)).
- lang: Add custom errors support for raw constraints ([#905](https://github.com/solana-foundation/anchor/pull/905)).
- lang, cli, spl: Update solana toolchain to v1.8.0 ([#886](https://github.com/solana-foundation/anchor/pull/886)).
- lang: Add custom errors support for `signer`, `mut`, `has_one`, `owner`, raw constraints and `address` ([#905](https://github.com/solana-foundation/anchor/pull/905), [#913](https://github.com/solana-foundation/anchor/pull/913)).

### Breaking

- lang: Accounts marked with the `#[account(signer)]` constraint now enforce signer when the `"cpi"` feature is enabled ([#849](https://github.com/solana-foundation/anchor/pull/849)).

## [0.17.0] - 2021-10-03

### Features

- cli: Add `localnet` command for starting a local `solana-test-validator` with the workspace deployed ([#820](https://github.com/solana-foundation/anchor/pull/820)).

### Breaking

- `CpiContext` accounts must now be used with the accounts struct generated in the `crate::cpi::accounts::*` module. These structs correspond to the accounts context for each instruction, except that each field is of type `AccountInfo` ([#824](https://github.com/solana-foundation/anchor/pull/824)).

## [0.16.2] - 2021-09-27

### Features

- lang: Add `--detach` flag to `anchor test` ([#770](https://github.com/solana-foundation/anchor/pull/770)).
- lang: Add `associated_token` keyword for initializing associated token accounts within `#[derive(Accounts)]` ([#790](https://github.com/solana-foundation/anchor/pull/790)).
- cli: Allow passing through cargo flags for build command ([#719](https://github.com/solana-foundation/anchor/pull/719)).
- cli: Allow passing through cargo flags for test, verify, and publish commands ([#804](https://github.com/solana-foundation/anchor/pull/804)).

### Fixes

- lang: Generated `AccountMeta`s for Rust clients now properly set the `isSigner` field ([#762](https://github.com/solana-foundation/anchor/pull/762)).

## [0.16.1] - 2021-09-17

### Fixes

- lang: `Signer` type now sets isSigner to true in the IDL ([#750](https://github.com/solana-foundation/anchor/pull/750)).

## [0.16.0] - 2021-09-16

### Features

- lang: `Program` type introduced for executable accounts ([#705](https://github.com/solana-foundation/anchor/pull/705)).
- lang: `Signer` type introduced for signing accounts where data is not used ([#705](https://github.com/solana-foundation/anchor/pull/705)).
- lang: `UncheckedAccount` type introduced as a preferred alias for `AccountInfo` ([#745](https://github.com/solana-foundation/anchor/pull/745)).

### Breaking Changes

- lang: `#[account(owner = <pubkey>)]` now requires a `Pubkey` instead of an account ([#691](https://github.com/solana-foundation/anchor/pull/691)).

## [0.15.0] - 2021-09-07

### Features

- lang: Add new `Account` type to replace `ProgramAccount` and `CpiAccount`, both of which are deprecated ([#686](https://github.com/solana-foundation/anchor/pull/686)).
- lang: `Box` can be used with `Account` types to reduce stack usage ([#686](https://github.com/solana-foundation/anchor/pull/686)).
- lang: Add `Owner` trait, which is automatically implemented by all `#[account]` structs ([#686](https://github.com/solana-foundation/anchor/pull/686)).
- lang: Check that ProgramAccount writable before mut borrow (`anchor-debug` only) ([#681](https://github.com/solana-foundation/anchor/pull/681)).

### Breaking Changes

- lang: All programs must now define their program id in source via `declare_id!` ([#686](https://github.com/solana-foundation/anchor/pull/686)).

## [0.14.0] - 2021-09-02

### Features

- lang: Ignore `Unnamed` structs instead of panic ([#605](https://github.com/solana-foundation/anchor/pull/605)).
- lang: Add constraints for initializing mint accounts as pdas, `#[account(init, seeds = [...], mint::decimals = <expr>, mint::authority = <expr>)]` ([#562](https://github.com/solana-foundation/anchor/pull/562)).
- lang: Add `AsRef<AccountInfo>` for `AccountInfo` wrappers ([#652](https://github.com/solana-foundation/anchor/pull/652)).
- lang: Optimize `trait Key` by removing `AccountInfo` cloning ([#652](https://github.com/solana-foundation/anchor/pull/652)).
- cli, client, lang: Update solana toolchain to v1.7.11 ([#653](https://github.com/solana-foundation/anchor/pull/653)).

### Breaking Changes

- lang: Change `#[account(init, seeds = [...], token = <expr>, authority = <expr>)]` to `#[account(init, token::mint = <expr> token::authority = <expr>)]` ([#562](https://github.com/solana-foundation/anchor/pull/562)).
- lang: `#[associated]` and `#[account(associated = <target>, with = <target>)]` are both removed ([#612](https://github.com/solana-foundation/anchor/pull/612)).
- cli: Removed `anchor launch` command ([#634](https://github.com/solana-foundation/anchor/pull/634)).
- lang: `#[account(init)]` now creates the account inside the same instruction to be consistent with initializing PDAs. To maintain the old behavior of `init`, replace it with `#[account(zero)]` ([#641](https://github.com/solana-foundation/anchor/pull/641)).
- lang: `bump` must be provided when using the `seeds` constraint. This has been added as an extra safety constraint to ensure that whenever a PDA is initialized via a constraint the bump used is the one created by `Pubkey::find_program_address` ([#641](https://github.com/solana-foundation/anchor/pull/641)).
- lang: `try_from_init` has been removed from `Loader`, `ProgramAccount`, and `CpiAccount` and replaced with `try_from_unchecked` ([#641](https://github.com/solana-foundation/anchor/pull/641)).
- lang: Remove `AccountsInit` trait ([#641](https://github.com/solana-foundation/anchor/pull/641)).
- lang: `try_from` methods for `ProgramAccount`, `Loader`, and `ProgramState` now take in an additional `program_id: &Pubkey` parameter ([#660](https://github.com/solana-foundation/anchor/pull/660)).

## [0.13.2] - 2021-08-11

### Fixes

- cli: Fix `anchor init` command "Workspace not found" regression ([#598](https://github.com/solana-foundation/anchor/pull/598)).

## [0.13.1] - 2021-08-10

### Features

- cli: Programs embedded into genesis during tests will produce program logs ([#594](https://github.com/solana-foundation/anchor/pull/594)).

### Fixes

- cli: Allows Cargo.lock to exist in workspace subdirectories when publishing ([#593](https://github.com/solana-foundation/anchor/pull/593)).

## [0.13.0] - 2021-08-08

### Features

- cli: Adds a `[registry]` section in the Anchor toml ([#570](https://github.com/solana-foundation/anchor/pull/570)).
- cli: Adds the `anchor login <api-token>` command ([#570](https://github.com/solana-foundation/anchor/pull/570)).
- cli: Adds the `anchor publish <package>` command ([#570](https://github.com/solana-foundation/anchor/pull/570)).
- cli: Adds a root level `anchor_version` field to the Anchor.toml for specifying the anchor docker image to use for verifiable builds ([#570](https://github.com/solana-foundation/anchor/pull/570)).
- cli: Adds a root level `solana_version` field to the Anchor.toml for specifying the solana toolchain to use for verifiable builds ([#570](https://github.com/solana-foundation/anchor/pull/570)).
- lang: Dynamically fetch rent sysvar for when using `init` ([#587](https://github.com/solana-foundation/anchor/pull/587)).

### Breaking

- cli: `[clusters.<network>]` Anchor.toml section has been renamed to `[programs.<network>]` ([#570](https://github.com/solana-foundation/anchor/pull/570)).
- cli: `[workspace]` member and exclude arrays must now be filepaths relative to the workspace root ([#570](https://github.com/solana-foundation/anchor/pull/570)).

## [0.12.0] - 2021-08-03

### Features

- cli: Add keys `members` / `exclude` in config `programs` section ([#546](https://github.com/solana-foundation/anchor/pull/546)).
- cli: Allow program address configuration for test command through `clusters.localnet` ([#554](https://github.com/solana-foundation/anchor/pull/554)).
- lang: IDLs are now parsed from the entire crate ([#517](https://github.com/solana-foundation/anchor/pull/517)).
- spl: Dex permissioned markets proxy ([#519](https://github.com/solana-foundation/anchor/pull/519), [#543](https://github.com/solana-foundation/anchor/pull/543)).

### Breaking Changes

- ts: Use `hex` by default for decoding Instruction ([#547](https://github.com/solana-foundation/anchor/pull/547)).
- lang: `CpiAccount::reload` mutates the existing struct instead of returning a new one ([#526](https://github.com/solana-foundation/anchor/pull/526)).
- cli: Anchor.toml now requires an explicit `[scripts]` test command ([#550](https://github.com/solana-foundation/anchor/pull/550)).

## [0.11.1] - 2021-07-09

### Features

- lang: Adds `require` macro for specifying assertions that return error codes on failure ([#483](https://github.com/solana-foundation/anchor/pull/483)).
- lang: Allow one to specify arbitrary programs as the owner when creating PDA ([#483](https://github.com/solana-foundation/anchor/pull/483)).
- lang: A new `bump` keyword is added to the accounts constraints, which is used to add an optional bump seed to the end of a `seeds` array. When used in conjunction with _both_ `init` and `seeds`, then the program executes `find_program_address` to assert that the given bump is the canonical bump ([#483](https://github.com/solana-foundation/anchor/pull/483)).

### Fixes

- lang: Preserve all instruction data for fallback functions ([#483](https://github.com/solana-foundation/anchor/pull/483)).
- ts: Event listener not firing when creating associated accounts ([#356](https://github.com/solana-foundation/anchor/issues/356)).

## [0.11.0] - 2021-07-03

### Features

- lang: Add fallback functions ([#457](https://github.com/solana-foundation/anchor/pull/457)).
- lang: Add feature flag for using the old state account discriminator. This is a temporary flag for those with programs built prior to v0.7.0 but want to use the latest Anchor version. Expect this to be removed in a future version ([#446](https://github.com/solana-foundation/anchor/pull/446)).
- lang: Add generic support to Accounts ([#496](https://github.com/solana-foundation/anchor/pull/496)).

### Breaking Changes

- cli: Remove `.spec` suffix on TypeScript tests files ([#441](https://github.com/solana-foundation/anchor/pull/441)).
- lang: Remove `belongs_to` constraint ([#459](https://github.com/solana-foundation/anchor/pull/459)).

## [0.10.0] - 2021-06-27

### Features

- lang: Add `#[account(address = <expr>)]` constraint for asserting the address of an account ([#400](https://github.com/solana-foundation/anchor/pull/400)).
- lang: Add `#[account(init, token = <mint-target>, authority = <token-owner-target>...)]` constraint for initializing SPL token accounts as program derived addresses for the program. Can be used when initialized via `seeds` or `associated` ([#400](https://github.com/solana-foundation/anchor/pull/400)).
- lang: Add `associated_seeds!` macro for generating signer seeds for CPIs signed by an `#[account(associated = <target>)]` account ([#400](https://github.com/solana-foundation/anchor/pull/400)).
- cli: Add `[scripts]` section to the Anchor.toml for specifying workspace scripts that can be run via `anchor run <script>` ([#400](https://github.com/solana-foundation/anchor/pull/400)).
- cli: `[clusters.<network>]` table entries can now also use `{ address = <base58-str>, idl = <filepath-str> }` to specify workspace programs ([#400](https://github.com/solana-foundation/anchor/pull/400)).

### Breaking Changes

- cli: Remove `--yarn` flag in favor of using `npx` ([#432](https://github.com/solana-foundation/anchor/pull/432)).

## [0.9.0] - 2021-06-15

### Features

- lang: Instruction data is now available to accounts constraints ([#386](https://github.com/solana-foundation/anchor/pull/386)).
- lang: Initialize program derived addresses with accounts constraints ([#386](https://github.com/solana-foundation/anchor/pull/386)).

### Breaking Changes

- lang: Event field names in IDLs are now mixed case. ([#379](https://github.com/solana-foundation/anchor/pull/379)).
- lang: Accounts trait now accepts an additional `&[u8]` parameter ([#386](https://github.com/solana-foundation/anchor/pull/386)).

## [0.8.0] - 2021-06-10

### Features

- cli: Add `--program-name` option for build command to build a single program at a time ([#362](https://github.com/solana-foundation/anchor/pull/362)).
- cli, client: Parse custom cluster urls from str ([#369](https://github.com/solana-foundation/anchor/pull/369)).
- cli, client, lang: Update solana toolchain to v1.7.1 ([#368](https://github.com/solana-foundation/anchor/pull/369)).
- ts: Instruction decoding and formatting ([#372](https://github.com/solana-foundation/anchor/pull/372)).
- lang: Add `#[account(close = <destination>)]` constraint for closing accounts and sending the rent exemption lamports to a specified destination account ([#371](https://github.com/solana-foundation/anchor/pull/371)).

### Fixes

- lang: Allows one to use `remaining_accounts` with `CpiContext` by implementing the `ToAccountMetas` trait on `CpiContext` ([#351](https://github.com/solana-foundation/anchor/pull/351/files)).

### Breaking

- lang, ts: Framework defined error codes are introduced, reserving error codes 0-300 for Anchor, and 300 and up for user defined error codes ([#354](https://github.com/solana-foundation/anchor/pull/354)).

## [0.7.0] - 2021-05-31

### Features

- cli: Add global options for override Anchor.toml values ([#313](https://github.com/solana-foundation/anchor/pull/313)).
- spl: Add `SetAuthority` instruction ([#307](https://github.com/solana-foundation/anchor/pull/307/files)).
- spl: Add init and close open orders instructions ([#245](https://github.com/solana-foundation/anchor/pull/245)).
- lang: `constraint = <expression>` added as a replacement for (the now deprecated) string literal constraints ([#341](https://github.com/solana-foundation/anchor/pull/341)).
- lang: Span information is now preserved, providing informative compiler error messages ([#341](https://github.com/solana-foundation/anchor/pull/341)).
- ts: Address metadata is now optional for `anchor.workspace` clients ([#310](https://github.com/solana-foundation/anchor/pull/310)).

### Breaking Changes

- ts: Retrieving deserialized accounts from the `<program>.account.<my-account>` and `<program>.state` namespaces now require explicitly invoking the `fetch` API. For example, `program.account.myAccount(<address>)` and `program.state()` is now `program.account.myAccount.fetch(<address>)` and `program.state.fetch()` ([#322](https://github.com/solana-foundation/anchor/pull/322)).
- lang: `#[account(associated)]` now requires `init` to be provided to create an associated account. If not provided, then the address will be assumed to exist, and a constraint will be added to ensure the correctness of the address ([#318](https://github.com/solana-foundation/anchor/pull/318)).
- lang, ts: Change account discriminator pre-image of the `#[state]` account discriminator to be namespaced by "state:" ([#320](https://github.com/solana-foundation/anchor/pull/320)).
- lang, ts: Change domain delimiters for the pre-image of the instruction sighash to be a single colon `:` to be consistent with accounts ([#321](https://github.com/solana-foundation/anchor/pull/321)).
- lang: Associated constraints no longer automatically implement `mut` ([#341](https://github.com/solana-foundation/anchor/pull/341)).
- lang: Associated `space` constraints must now be literal integers instead of literal strings ([#341](https://github.com/solana-foundation/anchor/pull/341)).

## [0.6.0] - 2021-05-23

### Features

- ts: Add `program.simulate` namespace ([#266](https://github.com/solana-foundation/anchor/pull/266)).
- ts: Introduce `Address` type, allowing one to use Base 58 encoded strings in public APIs ([#304](https://github.com/solana-foundation/anchor/pull/304)).
- ts: Replace deprecated `web3.Account` with `web3.Signer` in public APIs ([#296](https://github.com/solana-foundation/anchor/pull/296)).
- ts: Generated `anchor.workspace` clients can now be customized per network with `[cluster.<slug>]` in the Anchor.toml ([#308](https://github.com/solana-foundation/anchor/pull/308)).
- cli: Add yarn flag to test command ([#267](https://github.com/solana-foundation/anchor/pull/267)).
- cli: Add `--skip-build` flag to test command ([301](https://github.com/solana-foundation/anchor/pull/301)).
- cli: Add `anchor shell` command to spawn a node shell populated with an Anchor.toml based environment ([#303](https://github.com/solana-foundation/anchor/pull/303)).

### Breaking Changes

- cli: The Anchor.toml's `wallet` and `cluster` settings must now be under the `[provider]` table ([#305](https://github.com/solana-foundation/anchor/pull/305)).
- ts: Event coder `decode` API changed to decode strings directly instead of buffers ([#292](https://github.com/solana-foundation/anchor/pull/292)).
- ts: Event coder `encode` API removed ([#292](https://github.com/solana-foundation/anchor/pull/292)).

## [0.5.0] - 2021-05-07

### Features

- client: Adds support for state instructions ([#248](https://github.com/solana-foundation/anchor/pull/248)).
- lang: Add `anchor-debug` feature flag for logging ([#253](https://github.com/solana-foundation/anchor/pull/253)).
- ts: Add support for u16 ([#255](https://github.com/solana-foundation/anchor/pull/255)).

### Breaking Changes

- client: Renames `RequestBuilder::new` to `RequestBuilder::from` ([#248](https://github.com/solana-foundation/anchor/pull/248)).
- lang: Renames the generated `instruction::state::Ctor` struct to `instruction::state::New` ([#248](https://github.com/solana-foundation/anchor/pull/248)).

## [0.4.5] - 2021-04-29

- spl: Add serum DEX CPI client ([#224](https://github.com/solana-foundation/anchor/pull/224)).

## [0.4.4] - 2021-04-18

### Features

- lang: Allows one to specify multiple `with` targets when creating associated accounts ([#197](https://github.com/solana-foundation/anchor/pull/197)).
- lang, ts: Add array support ([#202](https://github.com/solana-foundation/anchor/pull/202)).
- lang: Zero copy deserialization for accounts ([#202](https://github.com/solana-foundation/anchor/pull/202), [#206](https://github.com/solana-foundation/anchor/pull/206)).
- lang, spl, cli, client: Upgrade solana toolchain to 1.6.6 ([#210](https://github.com/solana-foundation/anchor/pull/210)).

## [0.4.3] - 2021-04-13

### Features

- lang: CPI clients for program state instructions ([#43](https://github.com/solana-foundation/anchor/pull/43)).
- lang: Add `#[account(owner = <program>)]` constraint ([#178](https://github.com/solana-foundation/anchor/pull/178)).
- lang, cli, ts: Add `#[account(associated = <target>)]` and `#[associated]` attributes for creating associated program accounts within programs. The TypeScript package can fetch these accounts with a new `<program>.account.<account-name>.associated` (and `associatedAddress`) method ([#186](https://github.com/solana-foundation/anchor/pull/186)).

### Fixes

- lang: Unused `#[account]`s are now parsed into the IDL correctly ([#177](https://github.com/solana-foundation/anchor/pull/177)).

## [0.4.2] - 2021-04-10

### Features

- cli: Fund Anchor.toml configured wallet when testing ([#164](https://github.com/solana-foundation/anchor/pull/164)).
- spl: Add initialize_account instruction for spl tokens ([#166](https://github.com/solana-foundation/anchor/pull/166)).

## [0.4.1] - 2021-04-06

- cli: Version verifiable docker builder ([#145](https://github.com/solana-foundation/anchor/pull/145)).

## [0.4.0] - 2021-04-04

### Features

- cli: Specify test files to run ([#118](https://github.com/solana-foundation/anchor/pull/118)).
- lang: Allow overriding the `#[state]` account's size ([#121](https://github.com/solana-foundation/anchor/pull/121)).
- lang, client, ts: Add event emission and subscriptions ([#89](https://github.com/solana-foundation/anchor/pull/89)).
- lang/account: Allow namespacing account discriminators ([#128](https://github.com/solana-foundation/anchor/pull/128)).
- cli: TypeScript migrations ([#132](https://github.com/solana-foundation/anchor/pull/132)).
- lang: Add `#[account(executable)]` attribute ([#140](https://github.com/solana-foundation/anchor/pull/140)).

### Breaking Changes

- client: Replace url str with `Cluster` struct when constructing clients ([#89](https://github.com/solana-foundation/anchor/pull/89)).
- lang: Changes the account discriminator of `IdlAccount` to be namespaced by `"internal"` ([#128](https://github.com/solana-foundation/anchor/pull/128)).
- lang, spl, cli: Upgrade solana toolchain to 1.6.3, a major version upgrade even though only the minor version is incremented. This allows for the removal of `-#![feature(proc_macro_hygiene)]`. ([#139](https://github.com/solana-foundation/anchor/pull/139)).

## [0.3.0] - 2021-03-12

### Features

- ts: Allow preloading instructions for state rpc transactions ([cf9c84](https://github.com/solana-foundation/anchor/commit/cf9c847e4144989b5bc1936149d171e90204777b)).
- ts: Export sighash coder function ([734c75](https://github.com/solana-foundation/anchor/commit/734c751882f43beec7ea3f0f4d988b502e3f24e4)).
- cli: Specify programs to embed into local validator genesis via Anchor.toml while testing ([b3803a](https://github.com/solana-foundation/anchor/commit/b3803aec03fbbae1a794c9aa6a789e6cb58fda99)).
- cli: Allow skipping the creation of a local validator when testing against localnet ([#93](https://github.com/solana-foundation/anchor/pull/93)).
- cli: Adds support for tests with Typescript ([#94](https://github.com/solana-foundation/anchor/pull/94)).
- cli: Deterministic and verifiable builds ([#100](https://github.com/solana-foundation/anchor/pull/100)).
- cli, lang: Add write buffers for IDL upgrades ([#107](https://github.com/solana-foundation/anchor/pull/107)).

## Breaking Changes

- lang: Removes `IdlInstruction::Clear` ([#107](https://github.com/solana-foundation/anchor/pull/107)).

### Fixes

- cli: Propagates mocha test exit status on error ([79b791](https://github.com/solana-foundation/anchor/commit/79b791ffa85ffae5b6163fa853562aa568650f21)).

## [0.2.1] - 2021-02-11

### Features

- cli: Embed workspace programs into local validator genesis when testing ([733ec3](https://github.com/solana-foundation/anchor/commit/733ec300b0308e7d007873b0975585d836333fd4)).
- cli: Stream program logs to `.anchor/program-logs` directory when testing ([ce5ca7](https://github.com/solana-foundation/anchor/commit/ce5ca7ddab6e0fd579deddcd02094b3f798bbe6a)).
- spl: Add shared memory api [(d92cb1)](https://github.com/solana-foundation/anchor/commit/d92cb1516b78696d1257e41d0c5ac6821716300e).
- lang/attribute/access-control: Allow specifying multiple modifier functions ([845df6](https://github.com/solana-foundation/anchor/commit/845df6d1960bb544fa0f2e3331ec79cc804edeb6)).
- lang/syn: Allow state structs that don't have a ctor or impl block (just trait implementations) ([a78000](https://github.com/solana-foundation/anchor/commit/a7800026833d64579e5b19c90d724ecc20d2a455)).
- ts: Add instruction method to state namespace ([627c27](https://github.com/solana-foundation/anchor/commit/627c275e9cdf3dafafcf44473ba8146cc7979d44)).
- lang/syn, ts: Add support for u128 and i128 ([#83](https://github.com/solana-foundation/anchor/pull/83)).

## [0.2.0] - 2021-02-08

### Features

- lang: Adds the ability to create and use CPI program interfaces ([#66](https://github.com/solana-foundation/anchor/pull/66/files?file-filters%5B%5D=)).

### Breaking Changes

- lang, client, ts: Migrate from rust enum based method dispatch to a variant of sighash ([#64](https://github.com/solana-foundation/anchor/pull/64)).

## [0.1.0] - 2021-01-31

Initial release.

### Includes

- lang: `anchor-lang` crate providing a Rust eDSL for Solana.
- lang/attribute/access-control: Internal attribute macro for function modifiers.
- lang/attribute/account: Internal attribute macro for defining Anchor accounts.
- lang/attribute/error: Internal attribute macro for defining Anchor program errors.
- lang/attribute/program: Internal attribute macro for defining an Anchor program.
- lang/attribute/state: Internal attribute macro for defining an Anchor program state struct.
- lang/derive/accounts: Internal derive macro for defining deserialized account structs.
- lang/syn: Internal crate for parsing the Anchor eDSL, generating code, and an IDL.
- spl: `anchor-spl` crate providing CPI clients for Anchor programs.
- client: `anchor-client` crate providing Rust clients for Anchor programs.
- ts: `@project-serum/anchor` package for generating TypeScript clients.
- cli: Command line interface for managing Anchor programs.
