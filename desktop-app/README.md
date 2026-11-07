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
