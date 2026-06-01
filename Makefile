DICTIONARY_FILE ?= .cspell-anchor-dictionary.txt

.PHONY: build-cli
build-cli:
	cargo build -p anchor-cli --release
	cp target/release/anchor cli/npm-package/anchor

# Unified coverage for the v2 stack.
#
# Produces one HTML report combining:
#   1. SBF runtime coverage (lang-v2/spl-v2 as exercised inside the VM) —
#      collected via litesvm register tracing, mapped to source via DWARF.
#   2. Host-side coverage (derive macros, tests-v2 harness) — collected via
#      cargo-llvm-cov's LLVM instrumentation.
#
# Requirements: cargo-llvm-cov (cargo install cargo-llvm-cov), lcov (brew
# install lcov or apt install lcov — provides lcov + genhtml).
COVERAGE_DIR := target/coverage
COVERAGE_HTML := $(COVERAGE_DIR)/html
TESTS_V2_COVERAGE_TESTS := \
	account_address_constraints \
	account_meta_signer_overrides \
	accounts \
	borsh_realloc \
	client_builders \
	constraints \
	cpi \
	custom_constraints \
	declare_program \
	derives \
	dispatch_remaining \
	dup_mut \
	equivalence_metadata \
	equivalence_spl \
	event_cpi \
	idl_convert \
	idl_metadata \
	init_space_usability \
	ix_macro \
	optional_accounts \
	program_interface \
	seeds \
	spl \
	spl_ata \
	sysvar_idl \
	token_2022_extensions \
	token_interface
TESTS_V2_COVERAGE_ARGS := $(foreach test,$(TESTS_V2_COVERAGE_TESTS),--test $(test))

.PHONY: coverage-v2
coverage-v2:
	$(MAKE) coverage-v2-sbf
	$(MAKE) coverage-v2-host
	$(MAKE) coverage-v2-report
	@echo ""
	@echo "Coverage report: $(COVERAGE_HTML)/index.html"

# SBF runtime coverage: one-shot via `anchor coverage`, which sets up the
# DWARF build env (RUSTC_WRAPPER + CARGO_PROFILE_RELEASE_DEBUG=2 — same as
# `anchor debugger`), runs `cargo test` with register-tracing via
# SBF_TRACE_DIR, then maps PCs to source via addr2line and emits LCOV.
# tests-v2 sees SBF_TRACE_DIR and lowers fixture program optimization to
# release opt-level=1. Full debug/no-opt SBF builds overflow VM stack limits for
# larger fixtures; opt-level=1 keeps valid SBF objects while reducing optimizer
# pressure for coverage/debug attribution.
#
# `--skip-build` because tests-v2 isn't a cdylib itself — the test harness
# builds its programs per-manifest via `build_program()`, inheriting the
# wrapper from the parent env.
.PHONY: coverage-v2-sbf
coverage-v2-sbf: build-anchor-debug
	@echo "==> SBF runtime coverage"
	cd $(PWD)/tests-v2 && $(PWD)/target/debug/anchor coverage \
		--skip-build \
		--trace-dir $(PWD)/$(COVERAGE_DIR)/traces \
		--output $(PWD)/$(COVERAGE_DIR)/sbf.lcov \
		-- $(TESTS_V2_COVERAGE_ARGS)

# Always invoke cargo (it handles incremental) so the anchor binary picks up
# any changes to coverage.rs / source_resolver / etc.
.PHONY: build-anchor-debug
build-anchor-debug:
	cargo build -p anchor-cli

# Host-side coverage (derive macros + integration test harness).
#
# Shares `CARGO_PROFILE_RELEASE_DEBUG=2` with the sbf step so the SBF programs
# rebuilt here by tests-v2 reuse the same cached artifacts (otherwise the
# profile flag diff would force a full SBF rebuild).
.PHONY: coverage-v2-host
coverage-v2-host:
	$(MAKE) coverage-v2-host-clean
	$(MAKE) coverage-v2-host-lang
	$(MAKE) coverage-v2-host-spl
	$(MAKE) coverage-v2-host-tests
	$(MAKE) coverage-v2-host-idl-build
	$(MAKE) coverage-v2-host-report

.PHONY: coverage-v2-host-clean
coverage-v2-host-clean:
	@echo "==> Host-side coverage (derive macros + test harness)"
	@command -v cargo-llvm-cov >/dev/null || { \
		echo "cargo-llvm-cov not installed. Run: cargo install cargo-llvm-cov"; \
		exit 1; \
	}
	cargo llvm-cov clean --workspace

.PHONY: coverage-v2-host-lang
coverage-v2-host-lang:
	# First pass: `anchor-lang-v2` with its `testing` feature enabled — the
	# Miri-witnesses integration tests need the `anchor_lang_v2::testing`
	# scaffold. Split into its own invocation because `testing` is a
	# lang-v2-only feature, and passing it to a multi-package run would
	# fail when `anchor-spl-v2` / `tests-v2` don't define it.
	CARGO_PROFILE_RELEASE_DEBUG=2 \
	cargo llvm-cov --no-report -p anchor-lang-v2 --features testing

.PHONY: coverage-v2-host-spl
coverage-v2-host-spl:
	# Second pass: `anchor-spl-v2` under default features.
	CARGO_PROFILE_RELEASE_DEBUG=2 \
	cargo llvm-cov --no-report -p anchor-spl-v2

.PHONY: coverage-v2-host-tests
coverage-v2-host-tests:
	# Third pass: the tests-v2 runtime/e2e suite under default features.
	# Keep compile_fail.rs out of the coverage path: it exercises diagnostics
	# by repeatedly compiling throwaway fixtures, which is slow and does not
	# execute framework runtime paths for source coverage.
	CARGO_PROFILE_RELEASE_DEBUG=2 \
	cargo llvm-cov --no-report -p tests-v2 $(TESTS_V2_COVERAGE_ARGS)

.PHONY: coverage-v2-host-idl-build
coverage-v2-host-idl-build:
	# Fourth pass: the test programs that declare a local `idl-build` feature,
	# run with `--features idl-build` so the IDL-emission code paths fire —
	# derive-emitted `IdlAccountType` impls, `__idl_accounts()` /
	# `__idl_register_deps()` / `__idl_errors()` fns, and the
	# `__anchor_private_print_idl_*` test bodies. After the IDL redesign the
	# `idl-build` feature lives only in end-user crates; lang-v2 and spl-v2
	# ship the supporting trait + helpers unconditionally and have no
	# feature of their own. `--no-report` accumulates profile data across
	# passes; the merged report is assembled by the final `llvm-cov report`
	# below.
	CARGO_PROFILE_RELEASE_DEBUG=2 \
	cargo llvm-cov --no-report --features idl-build \
		-p constraints -p derives-test -p accounts-test -p spl-test

.PHONY: coverage-v2-host-report
coverage-v2-host-report:
	@command -v cargo-llvm-cov >/dev/null || { \
		echo "cargo-llvm-cov not installed. Run: cargo install cargo-llvm-cov"; \
		exit 1; \
	}
	cargo llvm-cov report \
		--lcov \
		--output-path $(COVERAGE_DIR)/host.lcov \
		--ignore-filename-regex='(\.cargo|target/(debug|release)/build)'

# Merge the two LCOV files into one.
#
# `cargo-llvm-cov` instruments every line of every file it compiles, including
# SBF-runtime code pulled in as a path dep of `tests-v2` (lang-v2/src/*.rs,
# spl-v2/src/*.rs). Some of those lines get erased by LLVM's `#[inline(always)]`
# handling — SBF's DWARF sees the line executed, but host-side instrumentation
# records `DA:N,0` because the machine code lives at the callsite, not the
# source site. Naive merge ends up displaying those lines as uncovered.
#
# The filter scrubs host's `DA:N,0` entries when SBF recorded a hit at the same
# (file, line). It also removes host zeroes for non-executable Rust source
# lines: delimiter-only lines, plus function signature continuation lines when
# SBF hit the function's `fn` line. Zero-hit executable lines on both sides
# remain in the report as genuinely uncovered — which matters: earlier versions
# of this filter stripped every zero-hit line for any file that appeared in
# sbf.lcov, which made files with genuine uncovered regions (e.g.
# spl-v2/src/token.rs's many untested CPI branches) look artificially at 100%.
# See analysis in `ci.lcov` vs Codecov comparison (spl-v2/src/token.rs was
# reported as 82/82 locally vs. 82/347 on CI, all because of this scrub).
# Source-aware line filtering preserves the SBF-only-executed fix without
# over-reaching.
.PHONY: coverage-v2-merge
coverage-v2-merge: build-anchor-debug $(COVERAGE_DIR)/sbf.lcov $(COVERAGE_DIR)/host.lcov
	$(MAKE) coverage-v2-merge-existing

.PHONY: coverage-v2-merge-existing
coverage-v2-merge-existing: build-anchor-debug
	@echo "==> Merging coverage"
	@command -v lcov >/dev/null || { \
		echo "lcov not installed. Run: brew install lcov  (or apt install lcov)"; \
		exit 1; \
	}
	@test -s $(COVERAGE_DIR)/sbf.lcov || { \
		echo "Missing $(COVERAGE_DIR)/sbf.lcov. Run: make coverage-v2-sbf"; \
		exit 1; \
	}
	@test -s $(COVERAGE_DIR)/host.lcov || { \
		echo "Missing $(COVERAGE_DIR)/host.lcov. Run: make coverage-v2-host"; \
		exit 1; \
	}
	$(PWD)/target/debug/anchor coverage-filter-host \
		--sbf-lcov $(COVERAGE_DIR)/sbf.lcov \
		--host-lcov $(COVERAGE_DIR)/host.lcov \
		--output $(COVERAGE_DIR)/host.filtered.lcov
	# Recompute LF/LH for files we mutated — lcov -a keeps the original
	# counts if they don't agree with the DA lines. Drop stale summary
	# lines and let lcov rebuild them on load.
	sed -i.bak '/^LF:/d;/^LH:/d' $(COVERAGE_DIR)/host.filtered.lcov
	rm -f $(COVERAGE_DIR)/host.filtered.lcov.bak
	lcov -a $(COVERAGE_DIR)/sbf.lcov -a $(COVERAGE_DIR)/host.filtered.lcov \
		-o $(COVERAGE_DIR)/combined.lcov \
		--ignore-errors empty
	# Drop cargo-registry, target-generated, and tests-v2 source paths. The
	# tests-v2 crates still drive host/SBF execution above, but the report is
	# meant to show framework coverage rather than test harness/program code.
	#
	# Lcov's glob handling
	# varies by version: the shell-style patterns below look redundant but
	# cover both behaviors. Older lcov (Ubuntu 1.14/1.16) treats `*` as
	# non-slash-crossing, so `*/.cargo/*` doesn't match an absolute path
	# like `/home/runner/.cargo/registry/.../file.rs`; adding `**/.cargo/**`
	# is a no-op on strict shell globs but handled by lcov 2.x as a
	# recursive glob. Having both keeps the filter correct on local macOS
	# (lcov 2.x), Ubuntu CI runners (lcov 1.x), and anywhere in between.
	lcov --remove $(COVERAGE_DIR)/combined.lcov \
		'*/.cargo/*' '**/.cargo/**' '*.cargo*' \
		'*/target/*' '**/target/**' \
		'*/tests-v2/*' '**/tests-v2/**' \
		-o $(COVERAGE_DIR)/combined.lcov \
		--ignore-errors unused \
		--ignore-errors empty
	# Self-merge to collapse duplicate `SF:` blocks. Host coverage runs
	# three `cargo llvm-cov --no-report` passes (lang-v2 testing, spl-v2 +
	# tests-v2 default, test programs + idl-build), plus the SBF pass.
	# Ubuntu's lcov 1.x
	# concatenates records from each pass rather than merging them,
	# yielding 4x duplicate `SF:` entries per file in the combined
	# output. lcov 2.x merges on read, so this step is a no-op locally
	# but essential for the report Codecov ingests from CI.
	lcov -a $(COVERAGE_DIR)/combined.lcov \
		-o $(COVERAGE_DIR)/combined.lcov \
		--ignore-errors empty,format,inconsistent

$(COVERAGE_DIR)/sbf.lcov: coverage-v2-sbf
$(COVERAGE_DIR)/host.lcov: coverage-v2-host

# Generate HTML report. Cleans output dir first so stale files from a prior
# run don't leak into the tree.
.PHONY: coverage-v2-html
coverage-v2-html: $(COVERAGE_DIR)/combined.lcov
	$(MAKE) coverage-v2-html-existing

.PHONY: coverage-v2-html-existing
coverage-v2-html-existing:
	@echo "==> Generating HTML report"
	@command -v genhtml >/dev/null || { \
		echo "genhtml not installed (ships with lcov)."; \
		exit 1; \
	}
	@test -s $(COVERAGE_DIR)/combined.lcov || { \
		echo "Missing $(COVERAGE_DIR)/combined.lcov. Run: make coverage-v2-merge"; \
		exit 1; \
	}
	rm -rf $(COVERAGE_HTML)
	@GENHTML_IGNORE_ERRORS=source,unmapped,category; \
	GENHTML_PROBE="$(COVERAGE_DIR)/genhtml-range-probe.lcov"; \
	printf 'TN:\nSF:$(PWD)/Makefile\nDA:1,0\nend_of_record\n' > "$$GENHTML_PROBE"; \
	if genhtml "$$GENHTML_PROBE" \
		--output-directory "$(COVERAGE_DIR)/genhtml-range-probe" \
		--prefix "$(PWD)" \
		--ignore-errors "$$GENHTML_IGNORE_ERRORS,range" \
		--quiet >/dev/null 2>&1; then \
		GENHTML_IGNORE_ERRORS="$$GENHTML_IGNORE_ERRORS,range"; \
	fi; \
	rm -rf "$(COVERAGE_DIR)/genhtml-range-probe" "$$GENHTML_PROBE"; \
	genhtml $(COVERAGE_DIR)/combined.lcov \
		--output-directory $(COVERAGE_HTML) \
		--prefix $(PWD) \
		--ignore-errors "$$GENHTML_IGNORE_ERRORS" \
		--quiet

$(COVERAGE_DIR)/combined.lcov: coverage-v2-merge

.PHONY: coverage-v2-report
coverage-v2-report:
	$(MAKE) coverage-v2-merge-existing
	$(MAKE) coverage-v2-html-existing

.PHONY: coverage-v2-clean
coverage-v2-clean:
	rm -rf $(COVERAGE_DIR)

.PHONY: clean
clean:
	find . -type d -name .anchor -print0 | xargs -0 rm -rf
	find . -type d -name node_modules -print0 | xargs -0 rm -rf
	find . -type d -name target -print0 | xargs -0 rm -rf

.PHONY: publish
publish:
	cd lang/syn/ && cargo publish && cd ../../
	sleep 25
	cd lang/derive/accounts/ && cargo publish && cd ../../../
	sleep 25
	cd lang/derive/serde/ && cargo publish && cd ../../../
	sleep 25
	cd lang/derive/space/ && cargo publish && cd ../../../
	sleep 25
	cd lang/attribute/access-control/ && cargo publish && cd ../../../
	sleep 25
	cd lang/attribute/account/ && cargo publish && cd ../../../
	sleep 25
	cd lang/attribute/constant/ && cargo publish && cd ../../../
	sleep 25
	cd lang/attribute/error/ && cargo publish && cd ../../../
	sleep 25
	cd lang/attribute/program/ && cargo publish && cd ../../..
	sleep 25
	cd lang/attribute/event/ && cargo publish && cd ../../../
	sleep 25
	cd lang/ && cargo publish && cd ../
	sleep 25
	cd spl/ && cargo publish && cd ../
	sleep 25
	cd client/ && cargo publish && cd ../
	sleep 25
	cd cli/ && cargo publish && cd ../
	sleep 25

.PHONY: spellcheck
spellcheck:
	cspell *.md **/*.md **/*.mdx --config cspell.config.yaml --no-progress

.PHONY: update-dictionary
update-dictionary:
	echo $(DICTIONARY_FILE)
	cspell *.md **/*.md **/*.mdx --config cspell.config.yaml --words-only --unique --no-progress --quiet 2>/dev/null | sort --ignore-case > .new-dictionary-words
	cat .new-dictionary-words $(DICTIONARY_FILE) | sort --ignore-case > .new-$(DICTIONARY_FILE)
	mv .new-$(DICTIONARY_FILE) $(DICTIONARY_FILE)
	rm -f .new-dictionary-words
