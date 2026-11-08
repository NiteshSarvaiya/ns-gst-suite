# NS Accounts — Accounting Core Audit

**Audit baseline:** `feature/accounting-v1.2-reports-company`  
**Status:** Static source review started; no claim of runtime or Windows verification.

## Release decision

**Not suitable for live books or statutory accounting yet.** The current build is a prototype. The UI launching and TypeScript/Vite build succeeding do not establish accounting correctness.

## Critical findings from the source review

| Area | Finding | Risk | Required correction |
|---|---|---:|---|
| Company onboarding | A single hard-coded company row is inserted automatically. | Critical | Explicit company creation and isolated books per company. Demo data must be opt-in and clearly labelled. |
| Opening balances | Each ledger accepts an independent opening balance. No balanced opening journal/control account is enforced. | Critical | Opening-balance import/workflow must validate total Dr = total Cr before activation. |
| Financial year | Trial Balance aggregates every voucher without a date boundary. | Critical | All reports must use explicit as-of/period dates and enforce company FY rules. |
| Voucher rules | The backend accepts a free-form voucher type and does not enforce Contra/Payment/Receipt/Journal account-side rules. | High | Validate allowed type and its accounting constraints in the backend. |
| Duplicate posting | Frontend has an in-memory posting guard, but there is no idempotency token or database-level request identity. | High | Add durable idempotency protection and regression tests for rapid/repeated submission. |
| Blank voucher rows | Frontend filters lines with zero debit and credit. This must be tested alongside row focus/add/remove and repeated keyboard submit. | High | Validate only populated rows, reject partially populated rows, and prove one post per user action. |
| Date validation | Backend currently checks only that date text has length 10. | High | Parse strict ISO dates and validate period boundaries. |
| Reports | P&L and Balance Sheet are group-name aggregation previews; closing profit, group hierarchy and full presentation are absent. | Critical | Build report logic from a formal chart-of-accounts hierarchy and independently reconcile totals. |
| Voucher lifecycle | Posted vouchers are read-only; no alteration, cancellation, reversal or audit trail exists. | High | Implement controlled alteration/cancellation with immutable audit history. |
| Database evolution | Tables are created with `IF NOT EXISTS`; no schema version/migration framework is present. | High | Introduce versioned, transactional migrations and backup-before-upgrade. |
| Data safety | No backup/restore workflow or tested recovery path is present. | Critical | Implement verified backup, restore, integrity check and recovery tests before production use. |
| Keyboard UX | The UI uses a global key listener and local form key handling. Shortcut behaviour varies with focused controls and needs a central, tested key map. | Medium | Centralize shortcut routing and test every workflow without mouse input. |

## Work completed on the audit branch\n\n- Added pure Rust validation for strict real calendar dates (including leap years).\n- Voucher types are restricted to Contra, Payment, Receipt and Journal at the backend boundary.\n- Completely blank trailing voucher rows are ignored safely. Partially completed rows, invalid ledger IDs, negative amounts, same-line debit/credit, zero-value vouchers and unbalanced vouchers are rejected.\n- Added Rust unit tests covering balanced journal acceptance, blank trailing row handling, partially completed row rejection, unbalanced voucher rejection, same-line debit/credit rejection, unsupported voucher types and real calendar date validation.\n- These tests are committed but have **not yet been executed** in a Rust toolchain in this environment. Do not treat them as passing until `cargo test` runs successfully.\n\n## Required accounting acceptance tests

The test suite must seed a disposable company and verify both ledger-level movements and report totals.

1. Capital introduced: Cash Dr / Capital Cr.
2. Contra: Bank Dr / Cash Cr.
3. Credit purchase: Purchase Dr / Supplier Cr.
4. Cash sale: Cash Dr / Sales Cr.
5. Purchase return: Supplier Dr / Purchase Return Cr.
6. Supplier payment: Supplier Dr / Bank Cr.
7. Multi-line voucher: multiple debit and credit lines balance exactly.
8. Blank trailing row: ignored safely; it must not create an empty line or duplicate voucher.
9. Partially completed row: rejected with a precise validation message.
10. Repeated Ctrl+Enter / rapid double-click: one voucher only.
11. Unbalanced voucher: rejected and database remains unchanged.
12. Unknown ledger: rejected and database remains unchanged.
13. Invalid calendar date and out-of-period date: rejected according to company policy.
14. Trial Balance: independently calculated Dr and Cr totals agree.
15. Ledger statement: opening balance, period movement and closing balance reconcile.
16. P&L and Balance Sheet: reconcile to the same trial balance and period.
17. Cancellation/reversal: original posting remains auditable and balances remain correct.
18. Backup/restore: restored database matches source balances and voucher counts.

## Engineering sequence

1. Freeze feature expansion and preserve existing user data.
2. Add pure accounting-domain functions and automated unit tests.
3. Add versioned database migrations and company isolation.
4. Replace independent opening balances with a balanced opening journal workflow.
5. Harden voucher validation and idempotent posting.
6. Rebuild date-aware accounting reports and reconcile them against test fixtures.
7. Centralize keyboard navigation and add keyboard-only UI tests.
8. Add voucher alteration/cancellation audit history.
9. Add backup/restore and recovery tests.
10. Only then resume GST and inventory feature development.

## Verification policy

- Every accounting calculation needs an automated expected-result test.
- Every database write needs success, rejection and rollback tests.
- Every release candidate needs a clean frontend build, Rust test/build, Windows desktop launch test and a manual accounting acceptance run.
- A feature is not marked complete because its screen renders.
- Existing local databases must be backed up before testing builds that change the schema.


## Continuous verification added

- Added `.github/workflows/desktop-checks.yml` to run the frontend production build, Rust unit tests and Rust formatting check on Windows.
- The repository currently has no committed npm lockfile, so CI uses `npm install` rather than `npm ci`.
- The GitHub connector did not return an associated workflow run for the latest workflow commit. CI execution is therefore not yet confirmed; inspect the Actions tab and fix any runner/build failures before treating the checks as green.

## As-of Trial Balance and opening-balance diagnostics

- Trial Balance now accepts an explicit as-of date and includes voucher movements only through that date.
- The screen exposes an as-of date selector and refresh action.
- Opening debit and credit totals are calculated separately from voucher movements; an imbalance is shown as a warning rather than silently hidden.
- Company financial-year dates and report date ranges are validated as real calendar dates.
- The diagnostic does not auto-post a suspense balance or alter existing opening balances. A dedicated opening-balance finalisation/import workflow remains outstanding.
- These changes are committed; automated CI execution is still pending confirmation.


## Release engineering and data protection update — 7 November 2026

The following implementation and verification supersede earlier statements in this audit that said these items had not yet been run or built:

- Added the Windows application icon required by Tauri's Windows resource build.
- Updated the Windows GitHub Actions workflow to build the frontend, run Rust unit tests, build an x64 Windows MSI, and upload the MSI as a workflow artifact.
- CI for commit 4aa8d76803020674938802a79d63ba378fe26ed6 completed successfully on the pull-request workflow. The frontend build, Rust unit tests and MSI artifact upload passed. The artifact is a development installer, not a production certification.
- Added a SQLite online-backup command. It creates a consistent snapshot, verifies the backup using PRAGMA integrity_check, removes a failed backup file, and only reports success when integrity_check returns ok. Backup restore, encryption and off-device scheduling remain outstanding.
- Added a baseline schema_migrations marker and changed fresh database seed balances to zero. New companies should not inherit fictitious cash, bank or capital opening balances. Existing non-empty ledger tables are not overwritten by the seed logic.
- Added TDS Payable and TDS Receivable as control ledgers only. This is not a TDS calculation, deduction, challan or return module.

## Current release decision

The installer may be used for controlled development testing only. Do not migrate active client books into it or rely on its preliminary P&L / Balance Sheet for statutory reporting. The app is still single-company, the migration marker is not yet a complete migration framework, opening balances do not have a balanced import/finalisation workflow, and accounting reports need independent expected-result reconciliation. Voucher lifecycle/audit, restore, GST workflows, TDS workflows, inventory, bank reconciliation and role/access controls remain release blockers.

The successful build is evidence that the current application compiles and packages on the Windows CI runner. It is not evidence that all accounting, tax or statutory scenarios have been validated.


## Multi-company workspace update — 7 November 2026

- Added a separate local registry database (ns_accounts_registry.db) that maps company IDs to company database filenames and stores the active company ID.
- Each company has a separate SQLite database file. The existing ns_accounts.db is registered as the first/default company and stays at its original path.
- Added create/list/switch company commands and a company selector / company creation screen.
- Added a regression test that creates a legacy company database, confirms that it is registered as the first company, switches the active registry entry to a second company, and confirms that the second company resolves to a different database file while the original file remains present.
- The Windows CI workflow for code SHA 0a03dc8374832002d5fc867043372e4cced2686f passed frontend production build, Rust unit tests, MSI packaging and artifact upload.
- This is a local multi-company isolation foundation, not a full branch/group/financial-year architecture. Company deletion, company archive/export, per-company backup catalog, restore and full migration management remain outstanding. Do not copy or manually rename company database files while the app is running.


## GST sales invoice vertical slice — 7 November 2026

- Added GST sales invoice header and line tables linked to the customer ledger and the accounting voucher.
- Added a pure Rust line calculation module using integer paise, quantity in thousandths and percentage rates in basis points. It calculates taxable value after line discount, CGST/SGST for the current intra-state case, IGST for inter-state case, and percentage-based cess.
- Added a GST Sales register and invoice composer. The company supplier state and place of supply are selected from canonical Indian state/UT names. GST rates and cess rates must be entered explicitly; the UI no longer assumes 18% as the default.
- Backend re-calculates and validates every line, validates the financial-year date and customer ledger group, then writes the invoice header, invoice lines, and balanced sales voucher in a single SQLite transaction.
- Automated tests cover intra-state split, inter-state IGST, discount, cess, paise rounding, invalid line input, schema presence, balanced invoice posting and financial-year rejection without leaving a voucher behind.
- This is not yet a complete statutory GST workflow. The current rate is user-entered and not governed by a versioned statutory rate/HSN master. GSTIN validation is only a basic format check. Purchase/ITC, returns, 2B reconciliation, credit/debit notes, printable tax invoice, e-invoice/e-way bill, RCM/exemption treatment and portal integrations remain outstanding.
