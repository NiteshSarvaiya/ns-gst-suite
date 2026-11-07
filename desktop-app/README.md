# NS Accounts

NS Accounts is an offline-first Windows desktop accounting application built with Tauri 2, React, TypeScript, Rust and SQLite. The current development build is a **bookkeeping prototype / release candidate for controlled testing**, not yet a complete statutory accounting or GST/TDS filing product.

## Current implementation

- Multi-line double-entry voucher entry for Contra, Payment, Receipt and Journal.
- Atomic SQLite voucher posting, paise-based integer storage and backend validation.
- Ledger master, opening balances, voucher register, voucher details and keyboard-first voucher entry.
- Day Book, ledger statement with running balances, as-of Trial Balance and preliminary group-wise P&L / Balance Sheet previews.
- Company profile and financial-year settings for the current single-company database.
- Opening-balance debit/credit diagnostic; no silent suspense plug is created.
- Verified SQLite online backup: a consistent database snapshot is written to the app's backups folder and checked with PRAGMA integrity_check before success is reported.
- Fresh database setup uses zero opening balances and includes initial GST and TDS control ledgers. Existing non-empty ledgers are not overwritten by the seed routine.
- A schema-version marker is recorded in SQLite. A full multi-version migration manager is still required.

## Windows development build

The GitHub Actions workflow builds the frontend, runs Rust unit tests, creates a Windows MSI installer and uploads it as NS-Accounts-Windows-MSI.

To run locally, install Node.js, Rust stable, Microsoft C++ Build Tools and WebView2. See the Tauri prerequisites: https://v2.tauri.app/start/prerequisites/.

```powershell
cd desktop-app
npm install
npm run tauri dev
```

To build locally:

```powershell
npm run tauri -- build --bundles msi
```

## Keyboard shortcuts

- Ctrl+N: New voucher
- F2: Voucher date
- F4 / F5 / F6 / F7: Contra / Payment / Receipt / Journal
- Ctrl+L: Add voucher line
- Ctrl+Enter: Post voucher
- Esc: Close voucher
- Alt+D / V / B / L / S / T / P / H / C: Dashboard / Vouchers / Day Book / Ledgers / Ledger Statement / Trial Balance / Profit & Loss / Balance Sheet / Company Settings

## Data safety

- The app stores the SQLite company database in the operating system's application-data directory.
- Backup creation is implemented and integrity-checked. **Restore, retention management, encrypted backups and automated off-device backup are not implemented yet.**
- Existing user databases must be backed up before any test build or schema change. Do not delete the application-data directory to troubleshoot.
- New-company zero-balance setup does not rewrite existing non-empty ledgers. This is not a replacement for a tested migration framework.

## Not yet implemented — do not use for live statutory books

The following are required before selling this as a full accounting and taxation solution:

- Multi-company isolation, branch / financial-year controls and tested database migrations.
- A balanced opening journal / opening-balance import and a complete accounting chart hierarchy.
- Independent reconciliation of Trial Balance, P&L, Balance Sheet, retained earnings and opening / closing balances against accounting acceptance fixtures.
- Voucher alteration, cancellation by reversal, immutable audit log, approval workflow and durable backend idempotency.
- Sales and purchase invoice sub-ledgers, debtor / creditor ageing, credit/debit notes, receipt allocations and document numbering controls.
- GST tax engine and document workflows: intra/inter-state place of supply, HSN/SAC, tax slabs with effective dates, exemptions, RCM, cess, e-invoice/e-way bill integration, GSTR-1, GSTR-3B, 2B/ITC reconciliation, refunds and statutory exports.
- TDS section/rule master with effective dates, threshold and rate configuration, deduction events, challans, return preparation/export, Form 16/16A and reconciliation.
- Inventory, valuation, units, batches/serials, bank reconciliation, fixed assets/depreciation, cost centres, payroll and budgeting.
- Backup restore verification, import/export, access controls, encryption, disaster recovery and release signing.
- Full automated accounting/tax regression tests, end-to-end keyboard/UI testing, independent CA validation and user acceptance testing.

Tax rates, thresholds, due dates and statutory schemas must be maintained as versioned, effective-dated rules with source references and reviewed updates. Do not hardcode tax law as permanent constants. Direct portal filing or API integrations must not be advertised unless the required official integration access and approvals have been obtained and tested.

## Product principle

The product should make correct accounting and compliance easier, not merely expose more screens. Every posted transaction must reconcile to the ledger and reports; every statutory calculation must be explainable and testable; and every tax output must be traceable to the rule version and source used.
