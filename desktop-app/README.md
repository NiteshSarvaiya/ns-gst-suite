# NS Accounts — V1.1 Keyboard Accounting

Offline-first desktop accounting prototype for Indian businesses.

## V1.1
- Tauri 2 desktop shell, React + TypeScript.
- Native Rust + rusqlite SQLite data layer in the OS application data directory.
- Atomic multi-line double-entry voucher posting in a SQLite transaction.
- Keyboard-first voucher entry: F2 date, F4 contra, F5 payment, F6 receipt, F7 journal, Ctrl+Enter post, Esc close.
- Ledger creation with opening balances, voucher register and trial balance.
- Values stored as integer paise.

## Run on Windows
Install Node.js 20+, Rust stable, Microsoft C++ Build Tools and WebView2. See https://v2.tauri.app/start/prerequisites/.

```powershell
cd desktop-app
npm install
npm run tauri dev
```

Build installer: `npm run tauri build`.

## Important limitations
This is still a prototype. Initial ledgers are demonstration data. Company and financial-year setup, balanced opening-balance import, voucher editing/cancellation, audit trail, GST invoices and returns, inventory, bank reconciliation, backup/restore, encryption and statutory report validation remain future work. Do not use for statutory books or live client accounting yet.


## V1.2 additions

- Company profile settings: company name, financial-year dates, state and optional GSTIN.
- Day Book with date-range filtering.
- Ledger Statement with opening balance, voucher movements and running balance.
- Group-wise Profit & Loss preview for the selected date range.
- Group-wise Balance Sheet preview as of a selected date.
- Company settings are stored in the local SQLite database.

### V1.2 reporting limitations

This remains an early accounting prototype, not production accounting or GST filing software. The reports are group-wise previews and have not yet been independently reconciled against a full accounting test suite. Profit & Loss closing transfer, full Balance Sheet presentation, group hierarchy, multi-company isolation, strict financial-year enforcement, voucher edit/cancel audit trail, GST tax invoice/return logic, backup/restore and Windows installer validation remain future work. Opening-balance entry is still a simplified ledger-level field and must be reconciled before using real books.

### Upgrade note

V1.2 adds a company_settings table using CREATE TABLE IF NOT EXISTS; existing V1.1 local databases are retained. Back up the local database before testing any development build. Demo seed ledgers are sample data only.
