# NS Accounts — Desktop V1

Offline-first desktop accounting starter for Indian businesses. This is an independently implemented product; it does not include Tally source code, assets, or proprietary internals.

## Current V1 scope
- Tauri 2 desktop shell (Windows/macOS/Linux)
- Local SQLite database through the Tauri SQL plugin
- Starter chart of accounts
- Journal/payment/receipt/contra voucher entry with equal debit and credit
- Voucher register and ledger list
- Dashboard and navigation
- Browser preview uses demo-only localStorage; it is not the production data store

## Prerequisites
- Node.js 20+
- Rust toolchain (stable)
- Tauri platform prerequisites: https://v2.tauri.app/start/prerequisites/

## Run
```bash
cd desktop-app
npm install
npm run tauri dev
```

Build:
```bash
npm run build
npm run tauri build
```

## Important accounting limitations
This is an engineering prototype, not production-ready accounting software. V1 currently supports a simple two-line voucher model. It does not yet implement GST invoices/returns, inventory, financial statements, multi-user controls, audit trail, backup/restore, encryption, or multi-device sync. Do not use it for statutory books until the posting engine, data integrity, backup and reporting tests are completed.

## Database
SQLite file: `ns_accounts.db` in the Tauri app data directory. Schema initialization is currently performed at startup in `src/db.ts`. Future migrations must be versioned and tested. Currency values are stored as integer paise to avoid floating-point accounting errors.

## Roadmap
1. Company and financial-year setup; groups, ledgers and opening balances
2. Multi-line voucher engine with atomic posting, cancellation and audit log
3. Trial balance, ledger, day book, P&L and balance sheet
4. GST masters, tax invoice, CGST/SGST/IGST, HSN/SAC and GST reports
5. Inventory, bank reconciliation, import/export and backup/restore
