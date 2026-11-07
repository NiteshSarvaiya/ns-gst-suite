use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Ledger { id:i64, name:String, group_name:String, opening_balance:i64, balance_type:String }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Voucher { id:i64, voucher_no:String, voucher_type:String, voucher_date:String, narration:String, total:i64 }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrialBalanceRow { id:i64, name:String, group_name:String, debit:i64, credit:i64 }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewLedger { name:String, group_name:String, opening_balance:i64, balance_type:String }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoucherEntry { ledger_id:i64, debit:i64, credit:i64 }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewVoucher { voucher_type:String, voucher_date:String, narration:String, entries:Vec<VoucherEntry> }

fn database_path(app:&AppHandle)->Result<PathBuf,String>{
 let dir=app.path().app_data_dir().map_err(|e|e.to_string())?;
 fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 Ok(dir.join("ns_accounts.db"))
}
fn open_db(app:&AppHandle)->Result<Connection,String>{
 let conn=Connection::open(database_path(app)?).map_err(|e|e.to_string())?;
 conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;").map_err(|e|e.to_string())?;
 Ok(conn)
}
fn initialize(conn:&Connection)->Result<(),String>{
 conn.execute_batch(r#"
 CREATE TABLE IF NOT EXISTS ledgers(id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL COLLATE NOCASE UNIQUE,group_name TEXT NOT NULL,opening_balance INTEGER NOT NULL DEFAULT 0 CHECK(opening_balance>=0),balance_type TEXT NOT NULL CHECK(balance_type IN ('Dr','Cr')),created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE TABLE IF NOT EXISTS vouchers(id INTEGER PRIMARY KEY AUTOINCREMENT,voucher_no TEXT NOT NULL UNIQUE,voucher_type TEXT NOT NULL,voucher_date TEXT NOT NULL,narration TEXT NOT NULL DEFAULT '',total INTEGER NOT NULL CHECK(total>0),created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE TABLE IF NOT EXISTS voucher_lines(id INTEGER PRIMARY KEY AUTOINCREMENT,voucher_id INTEGER NOT NULL REFERENCES vouchers(id),ledger_id INTEGER NOT NULL REFERENCES ledgers(id),debit INTEGER NOT NULL DEFAULT 0 CHECK(debit>=0),credit INTEGER NOT NULL DEFAULT 0 CHECK(credit>=0),CHECK((debit>0 AND credit=0) OR (credit>0 AND debit=0)));
 CREATE INDEX IF NOT EXISTS idx_vouchers_date ON vouchers(voucher_date,id);
 CREATE INDEX IF NOT EXISTS idx_lines_ledger ON voucher_lines(ledger_id);
 "#).map_err(|e|e.to_string())?;
 let count:i64=conn.query_row("SELECT COUNT(*) FROM ledgers",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if count==0{
  let seed=[("Cash","Cash-in-Hand",2500000_i64,"Dr"),("Bank Account","Bank Accounts",12500000,"Dr"),("Capital Account","Capital Account",15000000,"Cr"),("Sales","Sales Accounts",0,"Cr"),("Purchase","Purchase Accounts",0,"Dr"),("Output CGST","Duties & Taxes",0,"Cr"),("Output SGST","Duties & Taxes",0,"Cr"),("Output IGST","Duties & Taxes",0,"Cr"),("Input CGST","Duties & Taxes",0,"Dr"),("Input SGST","Duties & Taxes",0,"Dr"),("Input IGST","Duties & Taxes",0,"Dr"),("Sundry Debtors","Sundry Debtors",0,"Dr"),("Sundry Creditors","Sundry Creditors",0,"Cr"),("Office Expenses","Indirect Expenses",0,"Dr")];
  for(name,group,balance,side)in seed{conn.execute("INSERT INTO ledgers(name,group_name,opening_balance,balance_type) VALUES(?1,?2,?3,?4)",params![name,group,balance,side]).map_err(|e|e.to_string())?;}
 }
 Ok(())
}
#[tauri::command] fn init_db(app:AppHandle)->Result<(),String>{initialize(&open_db(&app)?)}
#[tauri::command] fn list_ledgers(app:AppHandle)->Result<Vec<Ledger>,String>{
 let conn=open_db(&app)?;initialize(&conn)?;
 let mut s=conn.prepare("SELECT id,name,group_name,opening_balance,balance_type FROM ledgers ORDER BY name").map_err(|e|e.to_string())?;
 let rows=s.query_map([],|r|Ok(Ledger{id:r.get(0)?,name:r.get(1)?,group_name:r.get(2)?,opening_balance:r.get(3)?,balance_type:r.get(4)?})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command] fn list_vouchers(app:AppHandle)->Result<Vec<Voucher>,String>{
 let conn=open_db(&app)?;initialize(&conn)?;
 let mut s=conn.prepare("SELECT id,voucher_no,voucher_type,voucher_date,narration,total FROM vouchers ORDER BY voucher_date DESC,id DESC").map_err(|e|e.to_string())?;
 let rows=s.query_map([],|r|Ok(Voucher{id:r.get(0)?,voucher_no:r.get(1)?,voucher_type:r.get(2)?,voucher_date:r.get(3)?,narration:r.get(4)?,total:r.get(5)?})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command] fn create_ledger(app:AppHandle,input:NewLedger)->Result<i64,String>{
 let name=input.name.trim();if name.is_empty(){return Err("Ledger name is required.".into())}
 if input.group_name.trim().is_empty(){return Err("Select a ledger group.".into())}
 if input.opening_balance<0{return Err("Opening balance cannot be negative.".into())}
 if input.balance_type!="Dr"&&input.balance_type!="Cr"{return Err("Select Dr or Cr.".into())}
 let conn=open_db(&app)?;initialize(&conn)?;
 conn.execute("INSERT INTO ledgers(name,group_name,opening_balance,balance_type) VALUES(?1,?2,?3,?4)",params![name,input.group_name.trim(),input.opening_balance,input.balance_type]).map_err(|e|format!("Could not create ledger: {e}"))?;
 Ok(conn.last_insert_rowid())
}
#[tauri::command] fn post_voucher(app:AppHandle,input:NewVoucher)->Result<String,String>{
 if input.voucher_date.len()!=10{return Err("Enter a valid voucher date.".into())}
 if input.entries.len()<2{return Err("A voucher needs at least two ledger lines.".into())}
 let mut debit=0_i64;let mut credit=0_i64;
 for line in &input.entries{
  if line.ledger_id<=0||line.debit<0||line.credit<0{return Err("Invalid ledger or amount.".into())}
  if(line.debit>0)==(line.credit>0){return Err("Each line must contain either debit or credit, not both.".into())}
  debit=debit.checked_add(line.debit).ok_or("Debit total overflow.")?;
  credit=credit.checked_add(line.credit).ok_or("Credit total overflow.")?;
 }
 if debit==0||debit!=credit{return Err("Voucher is not balanced. Total debit must equal total credit.".into())}
 let mut conn=open_db(&app)?;initialize(&conn)?;
 let tx=conn.transaction().map_err(|e|e.to_string())?;
 let seq:i64=tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM vouchers",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 let number=format!("VCH/{seq:06}");
 tx.execute("INSERT INTO vouchers(voucher_no,voucher_type,voucher_date,narration,total) VALUES(?1,?2,?3,?4,?5)",params![number,input.voucher_type,input.voucher_date,input.narration.trim(),debit]).map_err(|e|e.to_string())?;
 let id=tx.last_insert_rowid();
 for line in &input.entries{
  let exists:i64=tx.query_row("SELECT COUNT(*) FROM ledgers WHERE id=?1",params![line.ledger_id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if exists==0{return Err(format!("Ledger {} does not exist.",line.ledger_id))}
  tx.execute("INSERT INTO voucher_lines(voucher_id,ledger_id,debit,credit) VALUES(?1,?2,?3,?4)",params![id,line.ledger_id,line.debit,line.credit]).map_err(|e|e.to_string())?;
 }
 tx.commit().map_err(|e|e.to_string())?;Ok(number)
}
#[tauri::command] fn trial_balance(app:AppHandle)->Result<Vec<TrialBalanceRow>,String>{
 let conn=open_db(&app)?;initialize(&conn)?;
 let mut s=conn.prepare("SELECT l.id,l.name,l.group_name,MAX(0,(CASE WHEN l.balance_type='Dr' THEN l.opening_balance ELSE -l.opening_balance END)+COALESCE(SUM(v.debit-v.credit),0)),MAX(0,-((CASE WHEN l.balance_type='Dr' THEN l.opening_balance ELSE -l.opening_balance END)+COALESCE(SUM(v.debit-v.credit),0))) FROM ledgers l LEFT JOIN voucher_lines v ON v.ledger_id=l.id GROUP BY l.id ORDER BY l.name").map_err(|e|e.to_string())?;
 let rows=s.query_map([],|r|Ok(TrialBalanceRow{id:r.get(0)?,name:r.get(1)?,group_name:r.get(2)?,debit:r.get(3)?,credit:r.get(4)?})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[cfg_attr(mobile,tauri::mobile_entry_point)]
pub fn run(){tauri::Builder::default().invoke_handler(tauri::generate_handler![init_db,list_ledgers,list_vouchers,create_ledger,post_voucher,trial_balance]).run(tauri::generate_context!()).expect("error while running NS Accounts");}
