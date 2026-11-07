use rusqlite::{backup::Backup, params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::{Duration, SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Ledger { id:i64, name:String, group_name:String, opening_balance:i64, balance_type:String }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Voucher { id:i64, voucher_no:String, voucher_type:String, voucher_date:String, narration:String, total:i64 }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpeningBalanceStatus { debit_total:i64, credit_total:i64, difference:i64 }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrialBalanceRow { id:i64, name:String, group_name:String, debit:i64, credit:i64 }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatementRow { voucher_id:i64, voucher_no:String, voucher_type:String, voucher_date:String, narration:String, debit:i64, credit:i64, running_balance:i64 }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupBalance { group_name:String, amount:i64 }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanyInfo { name:String, financial_year_start:String, financial_year_end:String, state:String, gstin:String }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompanyUpdate { name:String, financial_year_start:String, financial_year_end:String, state:String, gstin:String }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewLedger { name:String, group_name:String, opening_balance:i64, balance_type:String }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoucherEntry { ledger_id:i64, debit:i64, credit:i64 }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewVoucher { voucher_type:String, voucher_date:String, narration:String, entries:Vec<VoucherEntry> }


fn valid_iso_date(value:&str)->bool {
 let parts:Vec<&str>=value.split('-').collect();
 if parts.len()!=3 || parts[0].len()!=4 || parts[1].len()!=2 || parts[2].len()!=2 { return false; }
 let (Ok(y),Ok(m),Ok(d))=(parts[0].parse::<i32>(),parts[1].parse::<u32>(),parts[2].parse::<u32>()) else{return false;};
 if y<1900 || !(1..=12).contains(&m) {return false;}
 let leap=y%4==0 && (y%100!=0 || y%400==0);
 let days=match m {1|3|5|7|8|10|12=>31,4|6|9|11=>30,2=>if leap{29}else{28},_=>0};
 (1..=days).contains(&d)
}
fn validate_voucher_input(kind:&str,date:&str,entries:&[VoucherEntry])->Result<Vec<VoucherEntry>,String>{
 if !["Contra","Payment","Receipt","Journal"].contains(&kind){return Err("Unsupported voucher type.".into());}
 if !valid_iso_date(date){return Err("Enter a valid calendar date in YYYY-MM-DD format.".into());}
 let mut clean=Vec::new(); let(mut debit,mut credit)=(0_i64,0_i64);
 for line in entries {
  if line.ledger_id==0 && line.debit==0 && line.credit==0 {continue;}
  if line.ledger_id<=0 || line.debit<0 || line.credit<0{return Err("Every used line needs a valid ledger and non-negative amount.".into());}
  if (line.debit>0)==(line.credit>0){return Err("Each used line must contain either debit or credit, not both.".into());}
  debit=debit.checked_add(line.debit).ok_or("Debit total overflow.")?;
  credit=credit.checked_add(line.credit).ok_or("Credit total overflow.")?;
  clean.push(VoucherEntry{ledger_id:line.ledger_id,debit:line.debit,credit:line.credit});
 }
 if clean.len()<2{return Err("A voucher needs at least two completed ledger lines.".into());}
 if debit==0 || debit!=credit{return Err("Voucher is not balanced. Total debit must equal total credit.".into());}
 Ok(clean)
}
fn valid_date_range(start:&str,end:&str)->bool { valid_iso_date(start) && valid_iso_date(end) && start<=end }

fn database_path(app:&AppHandle)->Result<PathBuf,String>{let dir=app.path().app_data_dir().map_err(|e|e.to_string())?;fs::create_dir_all(&dir).map_err(|e|e.to_string())?;Ok(dir.join("ns_accounts.db"))}
fn open_db(app:&AppHandle)->Result<Connection,String>{let conn=Connection::open(database_path(app)?).map_err(|e|e.to_string())?;conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;").map_err(|e|e.to_string())?;Ok(conn)}
fn initialize(conn:&Connection)->Result<(),String>{
 conn.execute_batch(r#"
 CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY,description TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 INSERT OR IGNORE INTO schema_migrations(version,description) VALUES(1,'Initial accounting database schema');
 CREATE TABLE IF NOT EXISTS ledgers(id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL COLLATE NOCASE UNIQUE,group_name TEXT NOT NULL,opening_balance INTEGER NOT NULL DEFAULT 0 CHECK(opening_balance>=0),balance_type TEXT NOT NULL CHECK(balance_type IN ('Dr','Cr')),created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE TABLE IF NOT EXISTS vouchers(id INTEGER PRIMARY KEY AUTOINCREMENT,voucher_no TEXT NOT NULL UNIQUE,voucher_type TEXT NOT NULL,voucher_date TEXT NOT NULL,narration TEXT NOT NULL DEFAULT '',total INTEGER NOT NULL CHECK(total>0),created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE TABLE IF NOT EXISTS voucher_lines(id INTEGER PRIMARY KEY AUTOINCREMENT,voucher_id INTEGER NOT NULL REFERENCES vouchers(id),ledger_id INTEGER NOT NULL REFERENCES ledgers(id),debit INTEGER NOT NULL DEFAULT 0 CHECK(debit>=0),credit INTEGER NOT NULL DEFAULT 0 CHECK(credit>=0),CHECK((debit>0 AND credit=0) OR (credit>0 AND debit=0)));
 CREATE INDEX IF NOT EXISTS idx_vouchers_date ON vouchers(voucher_date,id);
 CREATE INDEX IF NOT EXISTS idx_lines_ledger ON voucher_lines(ledger_id);
 CREATE TABLE IF NOT EXISTS company_settings(id INTEGER PRIMARY KEY CHECK(id=1),name TEXT NOT NULL,financial_year_start TEXT NOT NULL,financial_year_end TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'Gujarat',gstin TEXT NOT NULL DEFAULT '');
 INSERT OR IGNORE INTO company_settings(id,name,financial_year_start,financial_year_end,state,gstin) VALUES(1,'My Company','2026-04-01','2027-03-31','Gujarat','');
 "#).map_err(|e|e.to_string())?;
 let count:i64=conn.query_row("SELECT COUNT(*) FROM ledgers",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if count==0{let seed=[("Cash","Cash-in-Hand",0_i64,"Dr"),("Bank Account","Bank Accounts",0,"Dr"),("Capital Account","Capital Account",0,"Cr"),("Sales","Sales Accounts",0,"Cr"),("Purchase","Purchase Accounts",0,"Dr"),("Output CGST","Duties & Taxes",0,"Cr"),("Output SGST","Duties & Taxes",0,"Cr"),("Output IGST","Duties & Taxes",0,"Cr"),("Input CGST","Duties & Taxes",0,"Dr"),("Input SGST","Duties & Taxes",0,"Dr"),("Input IGST","Duties & Taxes",0,"Dr"),("TDS Payable","Duties & Taxes",0,"Cr"),("TDS Receivable","Duties & Taxes",0,"Dr"),("Sundry Debtors","Sundry Debtors",0,"Dr"),("Sundry Creditors","Sundry Creditors",0,"Cr"),("Office Expenses","Indirect Expenses",0,"Dr")];for(name,group,balance,side)in seed{conn.execute("INSERT INTO ledgers(name,group_name,opening_balance,balance_type) VALUES(?1,?2,?3,?4)",params![name,group,balance,side]).map_err(|e|e.to_string())?;}}
 Ok(())
}
#[tauri::command] fn init_db(app:AppHandle)->Result<(),String>{initialize(&open_db(&app)?)}
#[tauri::command] fn get_company(app:AppHandle)->Result<CompanyInfo,String>{let c=open_db(&app)?;initialize(&c)?;c.query_row("SELECT name,financial_year_start,financial_year_end,state,gstin FROM company_settings WHERE id=1",[],|r|Ok(CompanyInfo{name:r.get(0)?,financial_year_start:r.get(1)?,financial_year_end:r.get(2)?,state:r.get(3)?,gstin:r.get(4)?})).map_err(|e|e.to_string())}
#[tauri::command] fn update_company(app:AppHandle,input:CompanyUpdate)->Result<(),String>{let name=input.name.trim();if name.is_empty(){return Err("Company name is required.".into())}if !valid_iso_date(&input.financial_year_start)||!valid_iso_date(&input.financial_year_end)||input.financial_year_start>=input.financial_year_end{return Err("Enter valid financial year dates; the end date must be after the start date.".into())}let c=open_db(&app)?;initialize(&c)?;c.execute("UPDATE company_settings SET name=?1,financial_year_start=?2,financial_year_end=?3,state=?4,gstin=?5 WHERE id=1",params![name,input.financial_year_start,input.financial_year_end,input.state.trim(),input.gstin.trim()]).map_err(|e|e.to_string())?;Ok(())}
#[tauri::command] fn list_ledgers(app:AppHandle)->Result<Vec<Ledger>,String>{let conn=open_db(&app)?;initialize(&conn)?;let mut s=conn.prepare("SELECT id,name,group_name,opening_balance,balance_type FROM ledgers ORDER BY name").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(Ledger{id:r.get(0)?,name:r.get(1)?,group_name:r.get(2)?,opening_balance:r.get(3)?,balance_type:r.get(4)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VoucherLineDetail { ledger_id:i64, ledger_name:String, debit:i64, credit:i64 }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VoucherDetail { id:i64, voucher_no:String, voucher_type:String, voucher_date:String, narration:String, total:i64, lines:Vec<VoucherLineDetail> }
#[tauri::command]
fn get_voucher_detail(app:AppHandle,voucher_id:i64)->Result<VoucherDetail,String>{
 let c=open_db(&app)?;initialize(&c)?;
 let (id,no,kind,date,narration,total)=c.query_row(
  "SELECT id,voucher_no,voucher_type,voucher_date,narration,total FROM vouchers WHERE id=?1",
  params![voucher_id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?))
 ).map_err(|e|format!("Could not find voucher: {e}"))?;
 let mut stmt=c.prepare("SELECT vl.ledger_id,l.name,vl.debit,vl.credit FROM voucher_lines vl JOIN ledgers l ON l.id=vl.ledger_id WHERE vl.voucher_id=?1 ORDER BY vl.id").map_err(|e|e.to_string())?;
 let rows=stmt.query_map(params![voucher_id],|r|Ok(VoucherLineDetail{ledger_id:r.get(0)?,ledger_name:r.get(1)?,debit:r.get(2)?,credit:r.get(3)?})).map_err(|e|e.to_string())?;
 let lines=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
 Ok(VoucherDetail{id,voucher_no:no,voucher_type:kind,voucher_date:date,narration,total,lines})
}

#[tauri::command] fn list_vouchers(app:AppHandle)->Result<Vec<Voucher>,String>{let conn=open_db(&app)?;initialize(&conn)?;let mut s=conn.prepare("SELECT id,voucher_no,voucher_type,voucher_date,narration,total FROM vouchers ORDER BY voucher_date DESC,id DESC").map_err(|e|e.to_string())?;let rows=s.query_map([],|r|Ok(Voucher{id:r.get(0)?,voucher_no:r.get(1)?,voucher_type:r.get(2)?,voucher_date:r.get(3)?,narration:r.get(4)?,total:r.get(5)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}
#[tauri::command] fn create_ledger(app:AppHandle,input:NewLedger)->Result<i64,String>{let name=input.name.trim();if name.is_empty(){return Err("Ledger name is required.".into())}if input.group_name.trim().is_empty(){return Err("Select a ledger group.".into())}if input.opening_balance<0{return Err("Opening balance cannot be negative.".into())}if input.balance_type!="Dr"&&input.balance_type!="Cr"{return Err("Select Dr or Cr.".into())}let conn=open_db(&app)?;initialize(&conn)?;conn.execute("INSERT INTO ledgers(name,group_name,opening_balance,balance_type) VALUES(?1,?2,?3,?4)",params![name,input.group_name.trim(),input.opening_balance,input.balance_type]).map_err(|e|format!("Could not create ledger: {e}"))?;Ok(conn.last_insert_rowid())}
#[tauri::command] fn post_voucher(app:AppHandle,input:NewVoucher)->Result<String,String>{let entries=validate_voucher_input(&input.voucher_type,&input.voucher_date,&input.entries)?;let debit:i64=entries.iter().map(|e|e.debit).sum();let mut conn=open_db(&app)?;initialize(&conn)?;let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e|e.to_string())?;let seq:i64=tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM vouchers",[],|r|r.get(0)).map_err(|e|e.to_string())?;let number=format!("VCH/{seq:06}");tx.execute("INSERT INTO vouchers(voucher_no,voucher_type,voucher_date,narration,total) VALUES(?1,?2,?3,?4,?5)",params![number,input.voucher_type,input.voucher_date,input.narration.trim(),debit]).map_err(|e|e.to_string())?;let id=tx.last_insert_rowid();for line in &entries{let exists:i64=tx.query_row("SELECT COUNT(*) FROM ledgers WHERE id=?1",params![line.ledger_id],|r|r.get(0)).map_err(|e|e.to_string())?;if exists==0{return Err(format!("Ledger {} does not exist.",line.ledger_id))}tx.execute("INSERT INTO voucher_lines(voucher_id,ledger_id,debit,credit) VALUES(?1,?2,?3,?4)",params![id,line.ledger_id,line.debit,line.credit]).map_err(|e|e.to_string())?;}tx.commit().map_err(|e|e.to_string())?;Ok(number)}
#[tauri::command]
fn trial_balance(app:AppHandle,as_of:String)->Result<Vec<TrialBalanceRow>,String>{
 if !valid_iso_date(&as_of){return Err("Enter a valid Trial Balance date.".into())}
 let conn=open_db(&app)?;initialize(&conn)?;
 let mut s=conn.prepare("SELECT l.id,l.name,l.group_name,MAX(0,(CASE WHEN l.balance_type='Dr' THEN l.opening_balance ELSE -l.opening_balance END)+COALESCE(SUM(CASE WHEN v.voucher_date<=?1 THEN vl.debit-vl.credit ELSE 0 END),0)),MAX(0,-((CASE WHEN l.balance_type='Dr' THEN l.opening_balance ELSE -l.opening_balance END)+COALESCE(SUM(CASE WHEN v.voucher_date<=?1 THEN vl.debit-vl.credit ELSE 0 END),0))) FROM ledgers l LEFT JOIN voucher_lines vl ON vl.ledger_id=l.id LEFT JOIN vouchers v ON v.id=vl.voucher_id GROUP BY l.id ORDER BY l.name").map_err(|e|e.to_string())?;
 let rows=s.query_map(params![as_of],|r|Ok(TrialBalanceRow{id:r.get(0)?,name:r.get(1)?,group_name:r.get(2)?,debit:r.get(3)?,credit:r.get(4)?})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command]
fn opening_balance_status(app:AppHandle)->Result<OpeningBalanceStatus,String>{
 let conn=open_db(&app)?;initialize(&conn)?;
 let(dr,cr):(i64,i64)=conn.query_row("SELECT COALESCE(SUM(CASE WHEN balance_type='Dr' THEN opening_balance ELSE 0 END),0),COALESCE(SUM(CASE WHEN balance_type='Cr' THEN opening_balance ELSE 0 END),0) FROM ledgers",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
 Ok(OpeningBalanceStatus{debit_total:dr,credit_total:cr,difference:dr-cr})
}
#[tauri::command] fn ledger_statement(app:AppHandle,ledger_id:i64,from_date:String,to_date:String)->Result<Vec<StatementRow>,String>{if !valid_date_range(&from_date,&to_date){return Err("Enter a valid statement date range.".into())}let c=open_db(&app)?;initialize(&c)?;let opening:i64=c.query_row("SELECT (CASE WHEN balance_type='Dr' THEN opening_balance ELSE -opening_balance END)+COALESCE((SELECT SUM(vl.debit-vl.credit) FROM voucher_lines vl JOIN vouchers v ON v.id=vl.voucher_id WHERE vl.ledger_id=?1 AND v.voucher_date<?2),0) FROM ledgers WHERE id=?1",params![ledger_id,from_date],|r|r.get(0)).map_err(|e|e.to_string())?;let mut stmt=c.prepare("SELECT v.id,v.voucher_no,v.voucher_type,v.voucher_date,v.narration,vl.debit,vl.credit FROM voucher_lines vl JOIN vouchers v ON v.id=vl.voucher_id WHERE vl.ledger_id=?1 AND v.voucher_date BETWEEN ?2 AND ?3 ORDER BY v.voucher_date,v.id").map_err(|e|e.to_string())?;let rows=stmt.query_map(params![ledger_id,from_date,to_date],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?))).map_err(|e|e.to_string())?;let mut balance=opening;let mut result=Vec::new();for row in rows{let(id,no,kind,date,narration,dr,cr)=row.map_err(|e|e.to_string())?;balance+=dr-cr;result.push(StatementRow{voucher_id:id,voucher_no:no,voucher_type:kind,voucher_date:date,narration,debit:dr,credit:cr,running_balance:balance});}Ok(result)}
#[tauri::command] fn profit_loss(app:AppHandle,from_date:String,to_date:String)->Result<Vec<GroupBalance>,String>{if !valid_iso_date(&from_date)||!valid_iso_date(&to_date)||from_date>to_date{return Err("Enter a valid Profit & Loss date range.".into())}let c=open_db(&app)?;initialize(&c)?;let mut s=c.prepare("SELECT l.group_name,SUM(vl.debit-vl.credit) FROM voucher_lines vl JOIN vouchers v ON v.id=vl.voucher_id JOIN ledgers l ON l.id=vl.ledger_id WHERE v.voucher_date BETWEEN ?1 AND ?2 AND l.group_name IN ('Sales Accounts','Purchase Accounts','Direct Expenses','Indirect Expenses','Direct Incomes','Indirect Incomes') GROUP BY l.group_name ORDER BY l.group_name").map_err(|e|e.to_string())?;let rows=s.query_map(params![from_date,to_date],|r|Ok(GroupBalance{group_name:r.get(0)?,amount:r.get(1)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}
#[tauri::command] fn balance_sheet(app:AppHandle,as_of:String)->Result<Vec<GroupBalance>,String>{if !valid_iso_date(&as_of){return Err("Enter a valid Balance Sheet date.".into())}let c=open_db(&app)?;initialize(&c)?;let mut s=c.prepare("SELECT l.group_name,SUM((CASE WHEN l.balance_type='Dr' THEN l.opening_balance ELSE -l.opening_balance END)+COALESCE(x.net,0)) FROM ledgers l LEFT JOIN (SELECT vl.ledger_id,SUM(vl.debit-vl.credit) net FROM voucher_lines vl JOIN vouchers v ON v.id=vl.voucher_id WHERE v.voucher_date<=?1 GROUP BY vl.ledger_id) x ON x.ledger_id=l.id WHERE l.group_name NOT IN ('Sales Accounts','Purchase Accounts','Direct Expenses','Indirect Expenses','Direct Incomes','Indirect Incomes') GROUP BY l.group_name ORDER BY l.group_name").map_err(|e|e.to_string())?;let rows=s.query_map(params![as_of],|r|Ok(GroupBalance{group_name:r.get(0)?,amount:r.get(1)?})).map_err(|e|e.to_string())?;rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())}

#[tauri::command]
fn create_backup(app:AppHandle)->Result<String,String>{
 let source_path=database_path(&app)?;
 if !source_path.exists(){return Err("The company database has not been created yet.".into())}
 let source=Connection::open(&source_path).map_err(|e|format!("Could not open company database: {e}"))?;
 let backup_dir=app.path().app_data_dir().map_err(|e|e.to_string())?.join("backups");
 fs::create_dir_all(&backup_dir).map_err(|e|format!("Could not create backup folder: {e}"))?;
 let stamp=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
 let backup_path=backup_dir.join(format!("ns_accounts_backup_{stamp}.sqlite"));
 let mut destination=Connection::open(&backup_path).map_err(|e|format!("Could not create backup file: {e}"))?;
 {
  let backup=Backup::new(&source,&mut destination).map_err(|e|format!("Could not start SQLite backup: {e}"))?;
  backup.run_to_completion(128,Duration::from_millis(50),None).map_err(|e|format!("SQLite backup did not complete: {e}"))?;
 }
 let integrity:String=destination.query_row("PRAGMA integrity_check",[],|r|r.get(0)).map_err(|e|format!("Could not verify backup: {e}"))?;
 if integrity!="ok"{
  drop(destination);
  let _=fs::remove_file(&backup_path);
  return Err(format!("Backup integrity check failed: {integrity}"));
 }
 Ok(backup_path.to_string_lossy().into_owned())
}

#[cfg_attr(mobile,tauri::mobile_entry_point)]
pub fn run(){tauri::Builder::default().invoke_handler(tauri::generate_handler![init_db,get_company,update_company,list_ledgers,list_vouchers,get_voucher_detail,create_ledger,post_voucher,trial_balance,opening_balance_status,ledger_statement,profit_loss,balance_sheet,create_backup]).run(tauri::generate_context!()).expect("error while running NS Accounts");}

#[cfg(test)]
mod accounting_validation_tests {
 use super::*;
 fn e(ledger_id:i64,debit:i64,credit:i64)->VoucherEntry{VoucherEntry{ledger_id,debit,credit}}
 #[test] fn fresh_database_has_a_versioned_zero_balance_chart(){
  let conn=Connection::open_in_memory().unwrap();
  initialize(&conn).unwrap();
  let(debit,credit):(i64,i64)=conn.query_row("SELECT COALESCE(SUM(CASE WHEN balance_type='Dr' THEN opening_balance ELSE 0 END),0),COALESCE(SUM(CASE WHEN balance_type='Cr' THEN opening_balance ELSE 0 END),0) FROM ledgers",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
  let version:i64=conn.query_row("SELECT MAX(version) FROM schema_migrations",[],|r|r.get(0)).unwrap();
  let tds_ledgers:i64=conn.query_row("SELECT COUNT(*) FROM ledgers WHERE name IN ('TDS Payable','TDS Receivable')",[],|r|r.get(0)).unwrap();
  assert_eq!((debit,credit),(0,0));
  assert_eq!(version,1);
  assert_eq!(tds_ledgers,2);
 }
 #[test] fn sqlite_backup_api_creates_a_consistent_copy(){
  let source=Connection::open_in_memory().unwrap();
  source.execute_batch("CREATE TABLE sample(id INTEGER PRIMARY KEY,value TEXT NOT NULL); INSERT INTO sample(value) VALUES('NS Accounts');").unwrap();
  let mut target=Connection::open_in_memory().unwrap();
  {
   let backup=Backup::new(&source,&mut target).unwrap();
   backup.run_to_completion(16,Duration::from_millis(1),None).unwrap();
  }
  let value:String=target.query_row("SELECT value FROM sample WHERE id=1",[],|r|r.get(0)).unwrap();
  let integrity:String=target.query_row("PRAGMA integrity_check",[],|r|r.get(0)).unwrap();
  assert_eq!(value,"NS Accounts");
  assert_eq!(integrity,"ok");
 }
 #[test] fn accepts_balanced_journal(){assert!(validate_voucher_input("Journal","2026-11-07",&[e(1,10000,0),e(2,0,10000)]).is_ok());}
 #[test] fn ignores_completely_blank_trailing_row(){let r=validate_voucher_input("Journal","2026-11-07",&[e(1,10000,0),e(2,0,10000),e(0,0,0)]).unwrap();assert_eq!(r.len(),2);}
 #[test] fn rejects_partially_completed_blank_row(){assert!(validate_voucher_input("Journal","2026-11-07",&[e(1,10000,0),e(0,0,10000)]).is_err());}
 #[test] fn rejects_unbalanced_voucher(){assert!(validate_voucher_input("Journal","2026-11-07",&[e(1,10000,0),e(2,0,9000)]).is_err());}
 #[test] fn rejects_same_line_debit_and_credit(){assert!(validate_voucher_input("Journal","2026-11-07",&[e(1,10000,10000),e(2,0,0)]).is_err());}
 #[test] fn rejects_unknown_voucher_type(){assert!(validate_voucher_input("Sales","2026-11-07",&[e(1,10000,0),e(2,0,10000)]).is_err());}
 #[test] fn validates_real_calendar_dates(){assert!(valid_iso_date("2024-02-29"));assert!(!valid_iso_date("2025-02-29"));assert!(!valid_iso_date("2026-13-01"));assert!(!valid_iso_date("2026-2-01"));assert!(valid_date_range("2026-04-01","2027-03-31"));assert!(!valid_date_range("2027-04-01","2027-03-31"));}
}
