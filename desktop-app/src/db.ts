import Database from "@tauri-apps/plugin-sql";

export type Ledger = { id: number; name: string; group_name: string; opening_balance: number; balance_type: "Dr" | "Cr" };
export type Voucher = { id: number; voucher_no: string; voucher_type: string; voucher_date: string; narration: string; total: number };
let db: Awaited<ReturnType<typeof Database.load>> | null = null;

const demoLedgers: Ledger[] = [
  { id: 1, name: "Cash", group_name: "Cash-in-Hand", opening_balance: 25000, balance_type: "Dr" },
  { id: 2, name: "Bank Account", group_name: "Bank Accounts", opening_balance: 125000, balance_type: "Dr" },
  { id: 3, name: "Sales", group_name: "Sales Accounts", opening_balance: 0, balance_type: "Cr" },
  { id: 4, name: "Purchase", group_name: "Purchase Accounts", opening_balance: 0, balance_type: "Dr" },
  { id: 5, name: "Output CGST", group_name: "Duties & Taxes", opening_balance: 0, balance_type: "Cr" },
  { id: 6, name: "Output SGST", group_name: "Duties & Taxes", opening_balance: 0, balance_type: "Cr" },
  { id: 7, name: "Sundry Debtors", group_name: "Sundry Debtors", opening_balance: 0, balance_type: "Dr" },
  { id: 8, name: "Sundry Creditors", group_name: "Sundry Creditors", opening_balance: 0, balance_type: "Cr" }
];

export async function connectDb() {
  if (!("__TAURI_INTERNALS__" in window)) return false;
  db = await Database.load("sqlite:ns_accounts.db");
  await db.execute("PRAGMA foreign_keys = ON");
  await db.execute(`CREATE TABLE IF NOT EXISTS ledgers (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, group_name TEXT NOT NULL, opening_balance INTEGER NOT NULL DEFAULT 0, balance_type TEXT NOT NULL CHECK(balance_type IN ('Dr','Cr')))`);
  await db.execute(`CREATE TABLE IF NOT EXISTS vouchers (id INTEGER PRIMARY KEY AUTOINCREMENT, voucher_no TEXT NOT NULL UNIQUE, voucher_type TEXT NOT NULL, voucher_date TEXT NOT NULL, narration TEXT NOT NULL DEFAULT '', total INTEGER NOT NULL CHECK(total > 0), created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)`);
  await db.execute(`CREATE TABLE IF NOT EXISTS voucher_lines (id INTEGER PRIMARY KEY AUTOINCREMENT, voucher_id INTEGER NOT NULL REFERENCES vouchers(id), ledger_id INTEGER NOT NULL REFERENCES ledgers(id), debit INTEGER NOT NULL DEFAULT 0, credit INTEGER NOT NULL DEFAULT 0, CHECK ((debit > 0 AND credit = 0) OR (credit > 0 AND debit = 0)))`);
  const rows = await db.select<{count:number}[]>("SELECT COUNT(*) as count FROM ledgers");
  if (rows[0].count === 0) for (const l of demoLedgers) await db.execute("INSERT INTO ledgers(name,group_name,opening_balance,balance_type) VALUES($1,$2,$3,$4)", [l.name,l.group_name,l.opening_balance,l.balance_type]);
  return true;
}

export async function getLedgers(): Promise<Ledger[]> {
  if (!db) return demoLedgers;
  return db.select<Ledger[]>("SELECT * FROM ledgers ORDER BY name");
}
export async function getVouchers(): Promise<Voucher[]> {
  if (!db) return JSON.parse(localStorage.getItem("ns-demo-vouchers") || "[]");
  return db.select<Voucher[]>("SELECT * FROM vouchers ORDER BY voucher_date DESC,id DESC");
}
export async function postVoucher(input: {type:string; date:string; narration:string; debitLedger:number; creditLedger:number; amount:number}) {
  if (!Number.isSafeInteger(input.amount) || input.amount <= 0) throw new Error("Enter a valid amount in whole paise.");
  if (input.debitLedger === input.creditLedger) throw new Error("Debit and credit ledgers must be different.");
  if (!db) {
    const items = await getVouchers();
    const next = { id: Date.now(), voucher_no: `DEMO/${String(items.length+1).padStart(4,"0")}`, voucher_type: input.type, voucher_date: input.date, narration: input.narration, total: input.amount };
    localStorage.setItem("ns-demo-vouchers", JSON.stringify([next,...items]));
    return;
  }
  const rows = await db.select<{n:number}[]>("SELECT COUNT(*) as n FROM vouchers");
  const no = `VCH/${String(rows[0].n+1).padStart(5,"0")}`;
  await db.execute("BEGIN IMMEDIATE");
  try {
    const result = await db.execute("INSERT INTO vouchers(voucher_no,voucher_type,voucher_date,narration,total) VALUES($1,$2,$3,$4,$5)", [no,input.type,input.date,input.narration,input.amount]);
    const id = result.lastInsertId;
    await db.execute("INSERT INTO voucher_lines(voucher_id,ledger_id,debit,credit) VALUES($1,$2,$3,0)", [id,input.debitLedger,input.amount]);
    await db.execute("INSERT INTO voucher_lines(voucher_id,ledger_id,debit,credit) VALUES($1,$2,0,$3)", [id,input.creditLedger,input.amount]);
    await db.execute("COMMIT");
  } catch (error) {
    await db.execute("ROLLBACK");
    throw error;
  }
}