use rusqlite::{backup::Backup, params, Connection, OptionalExtension, Transaction, TransactionBehavior};
mod gst;
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}, time::{Duration, SystemTime, UNIX_EPOCH}};
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
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanySummary { id:String, name:String, is_active:bool }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompanyUpdate { name:String, financial_year_start:String, financial_year_end:String, state:String, gstin:String }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewLedger { name:String, group_name:String, opening_balance:i64, balance_type:String }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpeningBalanceEntry { ledger_id:i64, amount:i64, side:String }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(PartialEq, Eq, Clone)]
struct VoucherEntry { ledger_id:i64, debit:i64, credit:i64 }


#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TdsRuleInput {
 id:Option<i64>,section_code:String,nature_of_payment:String,description:String,
 rate_bps:i64,no_pan_rate_bps:Option<i64>,threshold_paise:i64,threshold_basis:String,
 effective_from:String,effective_to:Option<String>,source_reference:String,notes:String,is_active:bool
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TdsRule {
 id:i64,section_code:String,nature_of_payment:String,description:String,
 rate_bps:i64,no_pan_rate_bps:Option<i64>,threshold_paise:i64,threshold_basis:String,
 effective_from:String,effective_to:Option<String>,source_reference:String,notes:String,is_active:bool
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GstInvoiceLineInput { description:String, hsn_sac:String, quantity_milli:i64, unit_price_paise:i64, discount_paise:i64, gst_rate_bps:i64, cess_rate_bps:i64 }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewGstSalesInvoice { invoice_date:String, party_ledger_id:i64, party_gstin:String, place_of_supply_state:String, lines:Vec<GstInvoiceLineInput> }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GstSalesInvoiceSummary { id:i64, invoice_no:String, invoice_date:String, party_name:String, party_gstin:String, place_of_supply_state:String, supply_type:String, taxable_total:i64, cgst_total:i64, sgst_total:i64, igst_total:i64, cess_total:i64, invoice_total:i64, voucher_id:i64 }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewVoucher { request_id:String, voucher_type:String, voucher_date:String, narration:String, entries:Vec<VoucherEntry> }


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
 if !["Contra","Payment","Receipt","Journal","Sales"].contains(&kind){return Err("Unsupported voucher type.".into());}
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

fn app_data_dir(app:&AppHandle)->Result<PathBuf,String>{let dir=app.path().app_data_dir().map_err(|e|e.to_string())?;fs::create_dir_all(&dir).map_err(|e|e.to_string())?;Ok(dir)}
fn registry_connection_in(dir:&Path)->Result<Connection,String>{
 fs::create_dir_all(dir).map_err(|e|e.to_string())?;
 let registry=Connection::open(dir.join("ns_accounts_registry.db")).map_err(|e|e.to_string())?;
 registry.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS companies(id TEXT PRIMARY KEY,name TEXT NOT NULL COLLATE NOCASE,db_file TEXT NOT NULL UNIQUE,created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP); CREATE TABLE IF NOT EXISTS app_state(key TEXT PRIMARY KEY,value TEXT NOT NULL);").map_err(|e|e.to_string())?;
 let count:i64=registry.query_row("SELECT COUNT(*) FROM companies",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if count==0{
  let legacy=dir.join("ns_accounts.db");
  let legacy_name=if legacy.exists(){Connection::open(&legacy).ok().and_then(|db|db.query_row("SELECT name FROM company_settings WHERE id=1",[],|r|r.get::<_,String>(0)).optional().ok().flatten()).filter(|n|!n.trim().is_empty()).unwrap_or_else(||"My Company".to_string())}else{"My Company".to_string()};
  registry.execute("INSERT OR IGNORE INTO companies(id,name,db_file) VALUES('default',?1,'ns_accounts.db')",params![legacy_name]).map_err(|e|e.to_string())?;
  registry.execute("INSERT OR IGNORE INTO app_state(key,value) VALUES('active_company_id','default')",[]).map_err(|e|e.to_string())?;
 }
 Ok(registry)
}
fn registry_connection(app:&AppHandle)->Result<Connection,String>{let dir=app_data_dir(app)?;registry_connection_in(&dir)}
fn database_path_in(dir:&Path)->Result<PathBuf,String>{
 let registry=registry_connection_in(dir)?;
 let active:String=registry.query_row("SELECT value FROM app_state WHERE key='active_company_id'",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?.unwrap_or_else(||"default".to_string());
 let filename:Option<String>=registry.query_row("SELECT db_file FROM companies WHERE id=?1",params![active],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
 let filename=filename.unwrap_or_else(||"ns_accounts.db".to_string());
 if PathBuf::from(&filename).file_name().and_then(|n|n.to_str())!=Some(filename.as_str()){return Err("Invalid company database registration.".into())}
 Ok(dir.join(filename))
}
fn database_path(app:&AppHandle)->Result<PathBuf,String>{let dir=app_data_dir(app)?;database_path_in(&dir)}
fn open_db(app:&AppHandle)->Result<Connection,String>{let conn=Connection::open(database_path(app)?).map_err(|e|e.to_string())?;conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;").map_err(|e|e.to_string())?;Ok(conn)}

fn has_column(conn:&Connection,table:&str,column:&str)->Result<bool,String>{
 let sql=format!("PRAGMA table_info({table})");
 let mut stmt=conn.prepare(&sql).map_err(|e|e.to_string())?;
 let rows=stmt.query_map([],|row|row.get::<_,String>(1)).map_err(|e|e.to_string())?;
 for row in rows{if row.map_err(|e|e.to_string())?==column{return Ok(true)}}
 Ok(false)
}
fn apply_core_migrations(conn:&Connection)->Result<(),String>{
 if !has_column(conn,"vouchers","request_id")?{
  conn.execute("ALTER TABLE vouchers ADD COLUMN request_id TEXT",[]).map_err(|e|e.to_string())?;
 }
 conn.execute_batch("
  CREATE UNIQUE INDEX IF NOT EXISTS idx_vouchers_request_id ON vouchers(request_id) WHERE request_id IS NOT NULL AND request_id<>'';
  CREATE TABLE IF NOT EXISTS audit_log(
   id INTEGER PRIMARY KEY AUTOINCREMENT,
   event_type TEXT NOT NULL,
   entity_type TEXT NOT NULL,
   entity_id TEXT NOT NULL,
   description TEXT NOT NULL,
   created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
  );
  CREATE INDEX IF NOT EXISTS idx_audit_entity ON audit_log(entity_type,entity_id,id);
  CREATE TRIGGER IF NOT EXISTS audit_log_no_update BEFORE UPDATE ON audit_log
   BEGIN SELECT RAISE(ABORT,'Audit log is append-only'); END;
  CREATE TRIGGER IF NOT EXISTS audit_log_no_delete BEFORE DELETE ON audit_log
   BEGIN SELECT RAISE(ABORT,'Audit log is append-only'); END;
 ").map_err(|e|e.to_string())?;

 conn.execute_batch("
  CREATE TABLE IF NOT EXISTS tds_rules(
   id INTEGER PRIMARY KEY AUTOINCREMENT,
   section_code TEXT NOT NULL,
   nature_of_payment TEXT NOT NULL,
   description TEXT NOT NULL,
   rate_bps INTEGER NOT NULL CHECK(rate_bps BETWEEN 0 AND 10000),
   no_pan_rate_bps INTEGER CHECK(no_pan_rate_bps BETWEEN 0 AND 10000),
   threshold_paise INTEGER NOT NULL DEFAULT 0 CHECK(threshold_paise>=0),
   threshold_basis TEXT NOT NULL CHECK(threshold_basis IN ('PerPayment','AnnualAggregate','NoThreshold')),
   effective_from TEXT NOT NULL,
   effective_to TEXT,
   source_reference TEXT NOT NULL,
   notes TEXT NOT NULL DEFAULT '',
   is_active INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)),
   created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
   updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
   UNIQUE(section_code,nature_of_payment,effective_from)
  );
  CREATE INDEX IF NOT EXISTS idx_tds_rules_lookup ON tds_rules(section_code,nature_of_payment,effective_from,effective_to,is_active);
 ").map_err(|e|e.to_string())?;
 conn.execute("INSERT OR IGNORE INTO schema_migrations(version,description) VALUES(4,'Effective-dated configurable TDS rule master')",[]).map_err(|e|e.to_string())?;
 Ok(())
}
fn write_audit_event(tx:&Transaction,event_type:&str,entity_type:&str,entity_id:&str,description:&str)->Result<(),String>{
 tx.execute("INSERT INTO audit_log(event_type,entity_type,entity_id,description) VALUES(?1,?2,?3,?4)",params![event_type,entity_type,entity_id,description]).map_err(|e|e.to_string())?;
 Ok(())
}
fn initialize(conn:&Connection)->Result<(),String>{
 conn.execute_batch(r#"
 CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY,description TEXT NOT NULL,applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 INSERT OR IGNORE INTO schema_migrations(version,description) VALUES(1,'Initial accounting database schema');
 INSERT OR IGNORE INTO schema_migrations(version,description) VALUES(2,'GST sales invoice documents and accounting links');
 CREATE TABLE IF NOT EXISTS ledgers(id INTEGER PRIMARY KEY AUTOINCREMENT,name TEXT NOT NULL COLLATE NOCASE UNIQUE,group_name TEXT NOT NULL,opening_balance INTEGER NOT NULL DEFAULT 0 CHECK(opening_balance>=0),balance_type TEXT NOT NULL CHECK(balance_type IN ('Dr','Cr')),created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE TABLE IF NOT EXISTS vouchers(id INTEGER PRIMARY KEY AUTOINCREMENT,voucher_no TEXT NOT NULL UNIQUE,voucher_type TEXT NOT NULL,voucher_date TEXT NOT NULL,narration TEXT NOT NULL DEFAULT '',total INTEGER NOT NULL CHECK(total>0),created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
 CREATE TABLE IF NOT EXISTS voucher_lines(id INTEGER PRIMARY KEY AUTOINCREMENT,voucher_id INTEGER NOT NULL REFERENCES vouchers(id),ledger_id INTEGER NOT NULL REFERENCES ledgers(id),debit INTEGER NOT NULL DEFAULT 0 CHECK(debit>=0),credit INTEGER NOT NULL DEFAULT 0 CHECK(credit>=0),CHECK((debit>0 AND credit=0) OR (credit>0 AND debit=0)));
 CREATE INDEX IF NOT EXISTS idx_vouchers_date ON vouchers(voucher_date,id);
 CREATE INDEX IF NOT EXISTS idx_lines_ledger ON voucher_lines(ledger_id);
 CREATE TABLE IF NOT EXISTS gst_sales_invoices(
  id INTEGER PRIMARY KEY AUTOINCREMENT,invoice_no TEXT NOT NULL UNIQUE,invoice_date TEXT NOT NULL,
  party_ledger_id INTEGER NOT NULL REFERENCES ledgers(id),party_name TEXT NOT NULL,party_gstin TEXT NOT NULL DEFAULT '',
  place_of_supply_state TEXT NOT NULL,supply_type TEXT NOT NULL CHECK(supply_type IN ('Intra-State','Inter-State')),
  taxable_total INTEGER NOT NULL CHECK(taxable_total>=0),cgst_total INTEGER NOT NULL CHECK(cgst_total>=0),
  sgst_total INTEGER NOT NULL CHECK(sgst_total>=0),igst_total INTEGER NOT NULL CHECK(igst_total>=0),
  cess_total INTEGER NOT NULL CHECK(cess_total>=0),invoice_total INTEGER NOT NULL CHECK(invoice_total>0),
  voucher_id INTEGER NOT NULL UNIQUE REFERENCES vouchers(id),created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
 );
 CREATE INDEX IF NOT EXISTS idx_gst_sales_invoice_date ON gst_sales_invoices(invoice_date,id);
 CREATE TABLE IF NOT EXISTS gst_sales_invoice_lines(
  id INTEGER PRIMARY KEY AUTOINCREMENT,invoice_id INTEGER NOT NULL REFERENCES gst_sales_invoices(id) ON DELETE RESTRICT,
  line_no INTEGER NOT NULL,description TEXT NOT NULL,hsn_sac TEXT NOT NULL,quantity_milli INTEGER NOT NULL CHECK(quantity_milli>0),
  unit_price_paise INTEGER NOT NULL CHECK(unit_price_paise>0),discount_paise INTEGER NOT NULL CHECK(discount_paise>=0),
  gst_rate_bps INTEGER NOT NULL CHECK(gst_rate_bps>=0),cess_rate_bps INTEGER NOT NULL CHECK(cess_rate_bps>=0),
  taxable_value INTEGER NOT NULL CHECK(taxable_value>=0),cgst INTEGER NOT NULL CHECK(cgst>=0),sgst INTEGER NOT NULL CHECK(sgst>=0),
  igst INTEGER NOT NULL CHECK(igst>=0),cess INTEGER NOT NULL CHECK(cess>=0),line_total INTEGER NOT NULL CHECK(line_total>0),
  UNIQUE(invoice_id,line_no)
 );
 CREATE TABLE IF NOT EXISTS company_settings(id INTEGER PRIMARY KEY CHECK(id=1),name TEXT NOT NULL,financial_year_start TEXT NOT NULL,financial_year_end TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'Gujarat',gstin TEXT NOT NULL DEFAULT '');
 INSERT OR IGNORE INTO company_settings(id,name,financial_year_start,financial_year_end,state,gstin) VALUES(1,'My Company','2026-04-01','2027-03-31','Gujarat','');
 "#).map_err(|e|e.to_string())?;
 let count:i64=conn.query_row("SELECT COUNT(*) FROM ledgers",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if count==0{let seed=[("Cash","Cash-in-Hand",0_i64,"Dr"),("Bank Account","Bank Accounts",0,"Dr"),("Capital Account","Capital Account",0,"Cr"),("Sales","Sales Accounts",0,"Cr"),("Purchase","Purchase Accounts",0,"Dr"),("Output CGST","Duties & Taxes",0,"Cr"),("Output SGST","Duties & Taxes",0,"Cr"),("Output IGST","Duties & Taxes",0,"Cr"),("Input CGST","Duties & Taxes",0,"Dr"),("Input SGST","Duties & Taxes",0,"Dr"),("Input IGST","Duties & Taxes",0,"Dr"),("TDS Payable","Duties & Taxes",0,"Cr"),("TDS Receivable","Duties & Taxes",0,"Dr"),("Sundry Debtors","Sundry Debtors",0,"Dr"),("Sundry Creditors","Sundry Creditors",0,"Cr"),("Office Expenses","Indirect Expenses",0,"Dr")];for(name,group,balance,side)in seed{conn.execute("INSERT INTO ledgers(name,group_name,opening_balance,balance_type) VALUES(?1,?2,?3,?4)",params![name,group,balance,side]).map_err(|e|e.to_string())?;}}
 conn.execute("INSERT OR IGNORE INTO ledgers(name,group_name,opening_balance,balance_type) VALUES('Output Cess','Duties & Taxes',0,'Cr')",[]).map_err(|e|e.to_string())?;
 apply_core_migrations(conn)?;
 Ok(())
}

#[tauri::command]
fn list_companies(app:AppHandle)->Result<Vec<CompanySummary>,String>{
 let registry=registry_connection(&app)?;
 let active:String=registry.query_row("SELECT value FROM app_state WHERE key='active_company_id'",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?.unwrap_or_else(||"default".to_string());
 let mut stmt=registry.prepare("SELECT id,name FROM companies ORDER BY name COLLATE NOCASE").map_err(|e|e.to_string())?;
 let rows=stmt.query_map([],|r|{let id:String=r.get(0)?;let name:String=r.get(1)?;Ok(CompanySummary{id:id.clone(),name,is_active:id==active})}).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
#[tauri::command]
fn switch_company(app:AppHandle,company_id:String)->Result<(),String>{
 let registry=registry_connection(&app)?;
 let changed=registry.execute("UPDATE app_state SET value=?1 WHERE key='active_company_id' AND EXISTS(SELECT 1 FROM companies WHERE id=?1)",params![company_id]).map_err(|e|e.to_string())?;
 if changed!=1{return Err("The selected company does not exist.".into())}Ok(())
}
#[tauri::command]
fn create_company(app:AppHandle,name:String)->Result<String,String>{
 let name=name.trim();if name.is_empty(){return Err("Enter a company name.".into())}if name.len()>120{return Err("Company name must be 120 characters or fewer.".into())}
 let _=database_path(&app)?;let dir=app_data_dir(&app)?;let mut registry=registry_connection(&app)?;
 let stamp=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
 let id=format!("company_{stamp}");let filename=format!("{id}.sqlite");let path=dir.join(&filename);
 let result=(||->Result<(),String>{
  let db=Connection::open(&path).map_err(|e|e.to_string())?;initialize(&db)?;
  db.execute("UPDATE company_settings SET name=?1 WHERE id=1",params![name]).map_err(|e|e.to_string())?;
  let tx=registry.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
  tx.execute("INSERT INTO companies(id,name,db_file) VALUES(?1,?2,?3)",params![id,name,filename]).map_err(|e|format!("Could not register company: {e}"))?;
  tx.execute("INSERT INTO app_state(key,value) VALUES('active_company_id',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![id]).map_err(|e|e.to_string())?;
  tx.commit().map_err(|e|e.to_string())?;Ok(())
 })();
 if let Err(e)=result{let _=fs::remove_file(&path);return Err(e)}Ok(id)
}

#[tauri::command] fn init_db(app:AppHandle)->Result<(),String>{initialize(&open_db(&app)?)}
#[tauri::command] fn get_company(app:AppHandle)->Result<CompanyInfo,String>{let c=open_db(&app)?;initialize(&c)?;c.query_row("SELECT name,financial_year_start,financial_year_end,state,gstin FROM company_settings WHERE id=1",[],|r|Ok(CompanyInfo{name:r.get(0)?,financial_year_start:r.get(1)?,financial_year_end:r.get(2)?,state:r.get(3)?,gstin:r.get(4)?})).map_err(|e|e.to_string())}
#[tauri::command] fn update_company(app:AppHandle,input:CompanyUpdate)->Result<(),String>{let name=input.name.trim();if name.is_empty(){return Err("Company name is required.".into())}if !valid_iso_date(&input.financial_year_start)||!valid_iso_date(&input.financial_year_end)||input.financial_year_start>=input.financial_year_end{return Err("Enter valid financial year dates; the end date must be after the start date.".into())}let active_path=database_path(&app)?;let c=open_db(&app)?;initialize(&c)?;c.execute("UPDATE company_settings SET name=?1,financial_year_start=?2,financial_year_end=?3,state=?4,gstin=?5 WHERE id=1",params![name,input.financial_year_start,input.financial_year_end,input.state.trim(),input.gstin.trim()]).map_err(|e|e.to_string())?;let registry=registry_connection(&app)?;let filename=active_path.file_name().and_then(|n|n.to_str()).ok_or("Invalid company database file.")?;registry.execute("UPDATE companies SET name=?1 WHERE db_file=?2",params![name,filename]).map_err(|e|e.to_string())?;Ok(())}
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

fn save_opening_balances_tx(tx:&Transaction,entries:&[OpeningBalanceEntry])->Result<(),String>{
 if entries.len()<2{return Err("Opening balances require at least two ledger entries.".into())}
 let mut seen=std::collections::HashSet::new();
 let(mut debit,mut credit)=(0_i64,0_i64);
 for entry in entries{
  if entry.ledger_id<=0||entry.amount<0{return Err("Select a valid ledger and enter a non-negative opening amount.".into())}
  if entry.side!="Dr"&&entry.side!="Cr"{return Err("Opening balance side must be Dr or Cr.".into())}
  if !seen.insert(entry.ledger_id){return Err("A ledger can appear only once in the opening balance batch.".into())}
  let exists:i64=tx.query_row("SELECT COUNT(*) FROM ledgers WHERE id=?1",params![entry.ledger_id],|r|r.get(0)).map_err(|e|e.to_string())?;
  if exists==0{return Err(format!("Ledger {} does not exist.",entry.ledger_id))}
  if entry.side=="Dr"{debit=debit.checked_add(entry.amount).ok_or("Opening debit total overflow.")?}
  else{credit=credit.checked_add(entry.amount).ok_or("Opening credit total overflow.")?}
 }
 let ledger_count:i64=tx.query_row("SELECT COUNT(*) FROM ledgers",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if ledger_count as usize!=entries.len(){return Err("Include every ledger in the opening balance batch; use zero for ledgers without an opening amount.".into())}
 if debit==0||debit!=credit{return Err(format!("Opening balances are not balanced. Debit {} paise and credit {} paise must be equal and greater than zero.",debit,credit))}
 let voucher_count:i64=tx.query_row("SELECT COUNT(*) FROM vouchers",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if voucher_count>0{return Err("Opening balances are locked after the first voucher. Use an opening adjustment journal instead of changing historical opening figures.".into())}
 for entry in entries{
  tx.execute("UPDATE ledgers SET opening_balance=?1,balance_type=?2 WHERE id=?3",params![entry.amount,entry.side,entry.ledger_id]).map_err(|e|e.to_string())?;
 }
 write_audit_event(tx,"SET_OPENING_BALANCES","COMPANY","1",&format!("Balanced opening batch posted: debit {} paise, credit {} paise",debit,credit))?;
 Ok(())
}
#[tauri::command]
fn save_opening_balances(app:AppHandle,entries:Vec<OpeningBalanceEntry>)->Result<(),String>{
 let mut conn=open_db(&app)?;initialize(&conn)?;
 let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
 save_opening_balances_tx(&tx,&entries)?;
 tx.commit().map_err(|e|e.to_string())?;
 Ok(())
}
#[tauri::command] fn create_ledger(app:AppHandle,input:NewLedger)->Result<i64,String>{let name=input.name.trim();if name.is_empty(){return Err("Ledger name is required.".into())}if input.group_name.trim().is_empty(){return Err("Select a ledger group.".into())}if input.opening_balance!=0{return Err("Independent opening balances are disabled. Use a balanced opening journal workflow once available.".into())}if input.balance_type!="Dr"&&input.balance_type!="Cr"{return Err("Select Dr or Cr.".into())}let conn=open_db(&app)?;initialize(&conn)?;conn.execute("INSERT INTO ledgers(name,group_name,opening_balance,balance_type) VALUES(?1,?2,0,?3)",params![name,input.group_name.trim(),input.balance_type]).map_err(|e|format!("Could not create ledger: {e}"))?;Ok(conn.last_insert_rowid())}

fn is_cash_or_bank_group(group:&str)->bool{
 matches!(group,"Cash-in-Hand"|"Bank Accounts"|"Bank OD A/c"|"Bank Overdraft")
}
fn validate_manual_voucher_accounts(tx:&Transaction,kind:&str,entries:&[VoucherEntry])->Result<(),String>{
 let(mut cash_or_bank_debit,mut cash_or_bank_credit,mut all_cash_or_bank)=(false,false,true);
 for entry in entries{
  let group:String=tx.query_row("SELECT group_name FROM ledgers WHERE id=?1",params![entry.ledger_id],|r|r.get(0)).optional().map_err(|e|e.to_string())?.ok_or_else(||format!("Ledger {} does not exist.",entry.ledger_id))?;
  let cash_or_bank=is_cash_or_bank_group(&group);
  all_cash_or_bank&=cash_or_bank;
  if cash_or_bank{cash_or_bank_debit|=entry.debit>0;cash_or_bank_credit|=entry.credit>0;}
 }
 match kind{
  "Contra" if !all_cash_or_bank=>Err("Contra vouchers can contain only cash and bank ledgers. Use Payment, Receipt or Journal for other accounts.".into()),
  "Payment" if !cash_or_bank_credit=>Err("A Payment voucher must credit at least one cash or bank ledger.".into()),
  "Receipt" if !cash_or_bank_debit=>Err("A Receipt voucher must debit at least one cash or bank ledger.".into()),
  _=>Ok(())
 }
}
fn post_voucher_tx(tx:&Transaction,input:&NewVoucher,financial_year_start:&str,financial_year_end:&str)->Result<String,String>{
 if !["Contra","Payment","Receipt","Journal"].contains(&input.voucher_type.as_str()){
  return Err("Manual vouchers support only Contra, Payment, Receipt and Journal. Sales invoices must be created through GST Sales.".into())
 }
 if input.request_id.trim().is_empty()||input.request_id.len()>128{return Err("A valid posting request ID is required.".into())}
 if !valid_iso_date(&input.voucher_date)||input.voucher_date.as_str()<financial_year_start||input.voucher_date.as_str()>financial_year_end{
  return Err(format!("Voucher date must be within the company's financial year ({} to {}).",financial_year_start,financial_year_end))
 }
 let entries=validate_voucher_input(&input.voucher_type,&input.voucher_date,&input.entries)?;
 let debit=entries.iter().try_fold(0_i64,|total,line|total.checked_add(line.debit).ok_or("Debit total overflow."))?;
 let narration=input.narration.trim();
 // A repeat with the same key and same payload returns the original voucher. Reusing
 // the key with a changed payload is rejected rather than silently returning old data.
 let existing:Option<(i64,String,String,String,i64)>=tx.query_row(
  "SELECT id,voucher_no,voucher_type,voucher_date,narration,total FROM vouchers WHERE request_id=?1",
  params![input.request_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(5)?))
 ).optional().map_err(|e|e.to_string())?;
 if let Some((id,number,kind,date,total))=existing{
  let stored_narration:String=tx.query_row("SELECT narration FROM vouchers WHERE id=?1",params![id],|r|r.get(0)).map_err(|e|e.to_string())?;
  let mut stmt=tx.prepare("SELECT ledger_id,debit,credit FROM voucher_lines WHERE voucher_id=?1 ORDER BY id").map_err(|e|e.to_string())?;
  let rows=stmt.query_map(params![id],|r|Ok(VoucherEntry{ledger_id:r.get(0)?,debit:r.get(1)?,credit:r.get(2)?})).map_err(|e|e.to_string())?;
  let stored=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
  if kind==input.voucher_type&&date==input.voucher_date&&stored_narration==narration&&total==debit&&stored==entries{
   return Ok(number)
  }
  return Err("This posting request ID was already used for a different voucher. Reopen the voucher to create a new request.".into())
 }
 validate_manual_voucher_accounts(tx,&input.voucher_type,&entries)?;
 let seq:i64=tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM vouchers",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 let number=format!("VCH/{seq:06}");
 tx.execute("INSERT INTO vouchers(voucher_no,voucher_type,voucher_date,narration,total,request_id) VALUES(?1,?2,?3,?4,?5,?6)",params![number,input.voucher_type,input.voucher_date,narration,debit,input.request_id]).map_err(|e|e.to_string())?;
 let id=tx.last_insert_rowid();
 for line in &entries{
  tx.execute("INSERT INTO voucher_lines(voucher_id,ledger_id,debit,credit) VALUES(?1,?2,?3,?4)",params![id,line.ledger_id,line.debit,line.credit]).map_err(|e|e.to_string())?;
 }
 write_audit_event(tx,"POST","VOUCHER",&id.to_string(),&format!("Posted {} voucher {}",input.voucher_type,number))?;
 Ok(number)
}
#[tauri::command]
fn post_voucher(app:AppHandle,input:NewVoucher)->Result<String,String>{
 let mut conn=open_db(&app)?;initialize(&conn)?;
 let (fy_start,fy_end):(String,String)=conn.query_row("SELECT financial_year_start,financial_year_end FROM company_settings WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
 let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
 let number=post_voucher_tx(&tx,&input,&fy_start,&fy_end)?;
 tx.commit().map_err(|e|e.to_string())?;
 Ok(number)
}

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


fn ledger_id_by_name(tx:&Transaction,name:&str)->Result<i64,String>{
 tx.query_row("SELECT id FROM ledgers WHERE name=?1 COLLATE NOCASE",params![name],|r|r.get(0)).optional().map_err(|e|e.to_string())?.ok_or_else(||format!("Required ledger '{name}' is missing. Create or restore it before posting this invoice."))
}
fn normalized_state(value:&str)->String{value.chars().filter(|c|!c.is_whitespace()).flat_map(char::to_lowercase).collect()}
fn validate_gstin(value:&str)->bool{
 let v=value.trim().to_ascii_uppercase();
 v.len()==15 && v.chars().take(2).all(|c|c.is_ascii_digit()) && v.chars().all(|c|c.is_ascii_alphanumeric())
}

fn validate_tds_rule(input:&TdsRuleInput)->Result<(String,String,String,String,String),String>{
 let section=input.section_code.trim().to_ascii_uppercase();
 if section.is_empty()||section.len()>10||!section.chars().all(|c|c.is_ascii_alphanumeric()){return Err("Enter a valid TDS section code (letters and numbers only, up to 10 characters).".into())}
 let nature=input.nature_of_payment.trim();
 if nature.is_empty(){return Err("Nature of payment is required.".into())}
 if input.description.trim().is_empty(){return Err("Rule description is required.".into())}
 if !(0..=10000).contains(&input.rate_bps){return Err("TDS rate must be between 0% and 100%.".into())}
 if input.no_pan_rate_bps.is_some_and(|rate|!(0..=10000).contains(&rate)){return Err("The configured no-PAN rate must be between 0% and 100%.".into())}
 if input.threshold_paise<0{return Err("Threshold cannot be negative.".into())}
 if !["PerPayment","AnnualAggregate","NoThreshold"].contains(&input.threshold_basis.as_str()){return Err("Select a supported threshold method.".into())}
 if !valid_iso_date(&input.effective_from){return Err("Enter a valid effective-from date.".into())}
 let end=input.effective_to.as_deref().unwrap_or("").trim().to_string();
 if !end.is_empty()&&(!valid_iso_date(&end)||end<input.effective_from){return Err("Effective-to date must be a valid date on or after effective-from.".into())}
 let source=input.source_reference.trim();
 if source.is_empty(){return Err("A legal or official source reference is required for every TDS rule.".into())}
 Ok((section,nature.to_string(),end,source.to_string(),input.notes.trim().to_string()))
}
#[tauri::command]
fn list_tds_rules(app:AppHandle)->Result<Vec<TdsRule>,String>{
 let conn=open_db(&app)?;initialize(&conn)?;
 let mut stmt=conn.prepare("SELECT id,section_code,nature_of_payment,description,rate_bps,no_pan_rate_bps,threshold_paise,threshold_basis,effective_from,effective_to,source_reference,notes,is_active FROM tds_rules ORDER BY section_code,nature_of_payment,effective_from DESC").map_err(|e|e.to_string())?;
 let rows=stmt.query_map([],|r|Ok(TdsRule{id:r.get(0)?,section_code:r.get(1)?,nature_of_payment:r.get(2)?,description:r.get(3)?,rate_bps:r.get(4)?,no_pan_rate_bps:r.get(5)?,threshold_paise:r.get(6)?,threshold_basis:r.get(7)?,effective_from:r.get(8)?,effective_to:r.get(9)?,source_reference:r.get(10)?,notes:r.get(11)?,is_active:r.get::<_,i64>(12)?!=0})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}
fn save_tds_rule_tx(tx:&Transaction,input:TdsRuleInput)->Result<i64,String>{
 let (section,nature,end,source,notes)=validate_tds_rule(&input)?;
 let overlaps:i64=tx.query_row(
  "SELECT COUNT(*) FROM tds_rules WHERE section_code=?1 COLLATE NOCASE AND nature_of_payment=?2 COLLATE NOCASE AND (effective_to IS NULL OR effective_to>=?4) AND (?3 IS NULL OR ?3>=effective_from) AND (?5 IS NULL OR id<>?5)",
  params![section,nature,if end.is_empty(){None::<String>}else{Some(end.clone())},input.effective_from,input.id],
  |r|r.get(0)
 ).map_err(|e|e.to_string())?;
 if overlaps>0{return Err("This section and nature already have a rule whose effective period overlaps. Close the earlier rule or choose non-overlapping dates.".into())}
 let end_value=if end.is_empty(){None::<String>}else{Some(end)};
 let id=if let Some(id)=input.id{
  let changed=tx.execute("UPDATE tds_rules SET section_code=?1,nature_of_payment=?2,description=?3,rate_bps=?4,no_pan_rate_bps=?5,threshold_paise=?6,threshold_basis=?7,effective_from=?8,effective_to=?9,source_reference=?10,notes=?11,is_active=?12,updated_at=CURRENT_TIMESTAMP WHERE id=?13",params![section,nature,input.description.trim(),input.rate_bps,input.no_pan_rate_bps,input.threshold_paise,input.threshold_basis,input.effective_from,end_value,source,notes,input.is_active as i64,id]).map_err(|e|e.to_string())?;
  if changed!=1{return Err("TDS rule not found.".into())}id
 }else{
  tx.execute("INSERT INTO tds_rules(section_code,nature_of_payment,description,rate_bps,no_pan_rate_bps,threshold_paise,threshold_basis,effective_from,effective_to,source_reference,notes,is_active) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![section,nature,input.description.trim(),input.rate_bps,input.no_pan_rate_bps,input.threshold_paise,input.threshold_basis,input.effective_from,end_value,source,notes,input.is_active as i64]).map_err(|e|format!("Could not save TDS rule: {e}"))?;
  tx.last_insert_rowid()
 };
 write_audit_event(tx,if input.id.is_some(){"UPDATE"}else{"CREATE"},"TDS_RULE",&id.to_string(),&format!("{} TDS rule {} for {}",if input.id.is_some(){"Updated"}else{"Created"},section,nature))?;
 Ok(id)
}
#[tauri::command]
fn save_tds_rule(app:AppHandle,input:TdsRuleInput)->Result<i64,String>{
 let mut conn=open_db(&app)?;initialize(&conn)?;
 let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
 let id=save_tds_rule_tx(&tx,input)?;
 tx.commit().map_err(|e|e.to_string())?;
 Ok(id)
}
#[tauri::command]
fn list_gst_sales_invoices(app:AppHandle)->Result<Vec<GstSalesInvoiceSummary>,String>{
 let conn=open_db(&app)?;initialize(&conn)?;
 let mut stmt=conn.prepare("SELECT id,invoice_no,invoice_date,party_name,party_gstin,place_of_supply_state,supply_type,taxable_total,cgst_total,sgst_total,igst_total,cess_total,invoice_total,voucher_id FROM gst_sales_invoices ORDER BY invoice_date DESC,id DESC").map_err(|e|e.to_string())?;
 let rows=stmt.query_map([],|r|Ok(GstSalesInvoiceSummary{id:r.get(0)?,invoice_no:r.get(1)?,invoice_date:r.get(2)?,party_name:r.get(3)?,party_gstin:r.get(4)?,place_of_supply_state:r.get(5)?,supply_type:r.get(6)?,taxable_total:r.get(7)?,cgst_total:r.get(8)?,sgst_total:r.get(9)?,igst_total:r.get(10)?,cess_total:r.get(11)?,invoice_total:r.get(12)?,voucher_id:r.get(13)?})).map_err(|e|e.to_string())?;
 rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}

fn post_gst_sales_invoice_tx(tx:&Transaction,input:NewGstSalesInvoice,company:(String,String,String))->Result<GstSalesInvoiceSummary,String>{
 if !valid_iso_date(&input.invoice_date){return Err("Enter a valid invoice date.".into())}
 if input.lines.is_empty()||input.lines.len()>100{return Err("An invoice must contain between 1 and 100 item lines.".into())}
 let buyer_gstin=input.party_gstin.trim().to_ascii_uppercase();
 if !buyer_gstin.is_empty()&&!validate_gstin(&buyer_gstin){return Err("Buyer GSTIN must contain 15 alphanumeric characters and begin with a two-digit state code.".into())}
 let place=input.place_of_supply_state.trim();
 if place.is_empty(){return Err("Place of supply is required.".into())}
 if input.invoice_date<company.0||input.invoice_date>company.1{return Err(format!("Invoice date must fall within the company's financial year ({} to {}).",company.0,company.1))}
 if company.2.trim().is_empty(){return Err("Set the supplier's state in Company Settings before creating GST invoices.".into())}
 let interstate=normalized_state(&company.2)!=normalized_state(place);
 let supply_type=if interstate{"Inter-State"}else{"Intra-State"};
 let mut calculations=Vec::with_capacity(input.lines.len());
 let(mut taxable_total,mut cgst_total,mut sgst_total,mut igst_total,mut cess_total,mut invoice_total)=(0_i64,0_i64,0_i64,0_i64,0_i64,0_i64);
 for line in &input.lines{
  if line.description.trim().is_empty(){return Err("Every invoice line needs an item or service description.".into())}
  if line.hsn_sac.trim().is_empty(){return Err("Enter an HSN or SAC for every invoice line.".into())}
  let calc=gst::calculate_line(line.quantity_milli,line.unit_price_paise,line.discount_paise,line.gst_rate_bps,line.cess_rate_bps,interstate)?;
  taxable_total=taxable_total.checked_add(calc.taxable_value).ok_or("Invoice taxable total overflow.")?;
  cgst_total=cgst_total.checked_add(calc.cgst).ok_or("Invoice CGST total overflow.")?;
  sgst_total=sgst_total.checked_add(calc.sgst).ok_or("Invoice SGST total overflow.")?;
  igst_total=igst_total.checked_add(calc.igst).ok_or("Invoice IGST total overflow.")?;
  cess_total=cess_total.checked_add(calc.cess).ok_or("Invoice cess total overflow.")?;
  invoice_total=invoice_total.checked_add(calc.line_total).ok_or("Invoice total overflow.")?;
  calculations.push(calc);
 }
 if invoice_total<=0{return Err("Invoice total must be greater than zero.".into())}
 let party_name:String=tx.query_row("SELECT name FROM ledgers WHERE id=?1 AND group_name='Sundry Debtors'",params![input.party_ledger_id],|r|r.get(0)).optional().map_err(|e|e.to_string())?.ok_or("Select a customer ledger under Sundry Debtors.")?;
 let sales_id=ledger_id_by_name(tx,"Sales")?;
 let cgst_id=if cgst_total>0{Some(ledger_id_by_name(tx,"Output CGST")?)}else{None};
 let sgst_id=if sgst_total>0{Some(ledger_id_by_name(tx,"Output SGST")?)}else{None};
 let igst_id=if igst_total>0{Some(ledger_id_by_name(tx,"Output IGST")?)}else{None};
 let cess_id=if cess_total>0{Some(ledger_id_by_name(tx,"Output Cess")?)}else{None};
 let mut entries=vec![VoucherEntry{ledger_id:input.party_ledger_id,debit:invoice_total,credit:0},VoucherEntry{ledger_id:sales_id,debit:0,credit:taxable_total}];
 if let Some(id)=cgst_id{entries.push(VoucherEntry{ledger_id:id,debit:0,credit:cgst_total})}
 if let Some(id)=sgst_id{entries.push(VoucherEntry{ledger_id:id,debit:0,credit:sgst_total})}
 if let Some(id)=igst_id{entries.push(VoucherEntry{ledger_id:id,debit:0,credit:igst_total})}
 if let Some(id)=cess_id{entries.push(VoucherEntry{ledger_id:id,debit:0,credit:cess_total})}
 let entries=validate_voucher_input("Sales",&input.invoice_date,&entries)?;
 let fy_label=format!("{}-{}",&company.0[0..4],&company.1[2..4]);
 let invoice_seq:i64=tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM gst_sales_invoices",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 let invoice_no=format!("SI/{fy_label}/{invoice_seq:06}");
 let voucher_seq:i64=tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM vouchers",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 let voucher_no=format!("VCH/{voucher_seq:06}");
 let narration=format!("GST Sales Invoice {invoice_no} · {party_name}");
 tx.execute("INSERT INTO vouchers(voucher_no,voucher_type,voucher_date,narration,total) VALUES(?1,'Sales',?2,?3,?4)",params![voucher_no,input.invoice_date,narration,invoice_total]).map_err(|e|e.to_string())?;
 let voucher_id=tx.last_insert_rowid();
 for entry in &entries{tx.execute("INSERT INTO voucher_lines(voucher_id,ledger_id,debit,credit) VALUES(?1,?2,?3,?4)",params![voucher_id,entry.ledger_id,entry.debit,entry.credit]).map_err(|e|e.to_string())?;}
 tx.execute("INSERT INTO gst_sales_invoices(invoice_no,invoice_date,party_ledger_id,party_name,party_gstin,place_of_supply_state,supply_type,taxable_total,cgst_total,sgst_total,igst_total,cess_total,invoice_total,voucher_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",params![invoice_no,input.invoice_date,input.party_ledger_id,party_name,buyer_gstin,place,supply_type,taxable_total,cgst_total,sgst_total,igst_total,cess_total,invoice_total,voucher_id]).map_err(|e|e.to_string())?;
 let invoice_id=tx.last_insert_rowid();
 write_audit_event(tx,"CREATE","GST_SALES_INVOICE",&invoice_id.to_string(),&format!("Created GST sales invoice {}",invoice_no))?;
 for (index,(line,calc)) in input.lines.iter().zip(calculations.iter()).enumerate(){
  tx.execute("INSERT INTO gst_sales_invoice_lines(invoice_id,line_no,description,hsn_sac,quantity_milli,unit_price_paise,discount_paise,gst_rate_bps,cess_rate_bps,taxable_value,cgst,sgst,igst,cess,line_total) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![invoice_id,(index+1) as i64,line.description.trim(),line.hsn_sac.trim(),line.quantity_milli,line.unit_price_paise,line.discount_paise,line.gst_rate_bps,line.cess_rate_bps,calc.taxable_value,calc.cgst,calc.sgst,calc.igst,calc.cess,calc.line_total]).map_err(|e|e.to_string())?;
 }
 Ok(GstSalesInvoiceSummary{id:invoice_id,invoice_no,invoice_date:input.invoice_date,party_name,party_gstin:buyer_gstin,place_of_supply_state:place.to_string(),supply_type:supply_type.to_string(),taxable_total,cgst_total,sgst_total,igst_total,cess_total,invoice_total,voucher_id})
}
#[tauri::command]
fn create_gst_sales_invoice(app:AppHandle,input:NewGstSalesInvoice)->Result<GstSalesInvoiceSummary,String>{
 let mut conn=open_db(&app)?;initialize(&conn)?;
 let company:(String,String,String)=conn.query_row("SELECT financial_year_start,financial_year_end,state FROM company_settings WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
 let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
 let invoice=post_gst_sales_invoice_tx(&tx,input,company)?;
 tx.commit().map_err(|e|e.to_string())?;
 Ok(invoice)
}
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
pub fn run(){tauri::Builder::default().invoke_handler(tauri::generate_handler![init_db,list_companies,switch_company,create_company,get_company,update_company,list_ledgers,save_opening_balances,list_vouchers,get_voucher_detail,create_ledger,post_voucher,trial_balance,opening_balance_status,ledger_statement,profit_loss,balance_sheet,list_tds_rules,save_tds_rule,list_gst_sales_invoices,create_gst_sales_invoice,create_backup]).run(tauri::generate_context!()).expect("error while running NS Accounts");}

#[cfg(test)]
mod accounting_validation_tests {
 use super::*;
 fn e(ledger_id:i64,debit:i64,credit:i64)->VoucherEntry{VoucherEntry{ledger_id,debit,credit}}
 #[test] fn gst_sales_invoice_posts_balanced_voucher_and_tax_ledgers(){
  let mut conn=Connection::open_in_memory().unwrap();
  initialize(&conn).unwrap();
  conn.execute("INSERT INTO ledgers(name,group_name,opening_balance,balance_type) VALUES('Customer A','Sundry Debtors',0,'Dr')",[]).unwrap();
  let party:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Customer A'",[],|r|r.get(0)).unwrap();
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
  let input=NewGstSalesInvoice{invoice_date:"2026-11-07".into(),party_ledger_id:party,party_gstin:"".into(),place_of_supply_state:"Gujarat".into(),lines:vec![GstInvoiceLineInput{description:"Consulting service".into(),hsn_sac:"9983".into(),quantity_milli:1000,unit_price_paise:100000,discount_paise:0,gst_rate_bps:1800,cess_rate_bps:0}]};
  let invoice=post_gst_sales_invoice_tx(&tx,input,("2026-04-01".into(),"2027-03-31".into(),"Gujarat".into())).unwrap();
  tx.commit().unwrap();
  assert_eq!((invoice.taxable_total,invoice.cgst_total,invoice.sgst_total,invoice.igst_total,invoice.invoice_total),(100000,9000,9000,0,118000));
  let totals:(i64,i64)=conn.query_row("SELECT SUM(debit),SUM(credit) FROM voucher_lines WHERE voucher_id=?1",params![invoice.voucher_id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
  assert_eq!(totals,(118000,118000));
  let kind:String=conn.query_row("SELECT voucher_type FROM vouchers WHERE id=?1",params![invoice.voucher_id],|r|r.get(0)).unwrap();
  assert_eq!(kind,"Sales");
 }
 #[test] fn gst_sales_invoice_rejects_inter_financial_year_dates_without_posting(){
  let mut conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  conn.execute("INSERT INTO ledgers(name,group_name,opening_balance,balance_type) VALUES('Customer B','Sundry Debtors',0,'Dr')",[]).unwrap();
  let party:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Customer B'",[],|r|r.get(0)).unwrap();
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
  let input=NewGstSalesInvoice{invoice_date:"2025-03-31".into(),party_ledger_id:party,party_gstin:"".into(),place_of_supply_state:"Gujarat".into(),lines:vec![GstInvoiceLineInput{description:"Service".into(),hsn_sac:"9983".into(),quantity_milli:1000,unit_price_paise:100000,discount_paise:0,gst_rate_bps:1800,cess_rate_bps:0}]};
  assert!(post_gst_sales_invoice_tx(&tx,input,("2026-04-01".into(),"2027-03-31".into(),"Gujarat".into())).is_err());
  drop(tx);
  let count:i64=conn.query_row("SELECT COUNT(*) FROM vouchers",[],|r|r.get(0)).unwrap();
  assert_eq!(count,0);
 }
 #[test] fn initialization_installs_gst_sales_invoice_schema(){
  let conn=Connection::open_in_memory().unwrap();
  initialize(&conn).unwrap();
  let invoice_table:i64=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='gst_sales_invoices'",[],|r|r.get(0)).unwrap();
  let line_table:i64=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='gst_sales_invoice_lines'",[],|r|r.get(0)).unwrap();
  let version:i64=conn.query_row("SELECT MAX(version) FROM schema_migrations",[],|r|r.get(0)).unwrap();
  let cess_ledger:i64=conn.query_row("SELECT COUNT(*) FROM ledgers WHERE name='Output Cess'",[],|r|r.get(0)).unwrap();
  assert_eq!((invoice_table,line_table,version,cess_ledger),(1,1,4,1));
 }
 #[test] fn company_registry_preserves_existing_database_and_separates_new_company_files(){
  let stamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
  let dir=std::env::temp_dir().join(format!("ns_accounts_registry_test_{stamp}"));
  fs::create_dir_all(&dir).unwrap();
  let legacy=Connection::open(dir.join("ns_accounts.db")).unwrap();
  legacy.execute_batch("CREATE TABLE company_settings(id INTEGER PRIMARY KEY,name TEXT NOT NULL); INSERT INTO company_settings(id,name) VALUES(1,'Legacy Company');").unwrap();
  drop(legacy);
  let registry=registry_connection_in(&dir).unwrap();
  let registered:(String,String)=registry.query_row("SELECT name,db_file FROM companies WHERE id='default'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
  assert_eq!(registered,("Legacy Company".to_string(),"ns_accounts.db".to_string()));
  assert_eq!(database_path_in(&dir).unwrap(),dir.join("ns_accounts.db"));
  registry.execute("INSERT INTO companies(id,name,db_file) VALUES('other','Other Company','company_other.sqlite')",[]).unwrap();
  registry.execute("UPDATE app_state SET value='other' WHERE key='active_company_id'",[]).unwrap();
  assert_eq!(database_path_in(&dir).unwrap(),dir.join("company_other.sqlite"));
  assert!(dir.join("ns_accounts.db").exists());
  let _=fs::remove_dir_all(&dir);
 }
 #[test] fn fresh_database_has_a_versioned_zero_balance_chart(){
  let conn=Connection::open_in_memory().unwrap();
  initialize(&conn).unwrap();
  let(debit,credit):(i64,i64)=conn.query_row("SELECT COALESCE(SUM(CASE WHEN balance_type='Dr' THEN opening_balance ELSE 0 END),0),COALESCE(SUM(CASE WHEN balance_type='Cr' THEN opening_balance ELSE 0 END),0) FROM ledgers",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
  let version:i64=conn.query_row("SELECT MAX(version) FROM schema_migrations",[],|r|r.get(0)).unwrap();
  let tds_ledgers:i64=conn.query_row("SELECT COUNT(*) FROM ledgers WHERE name IN ('TDS Payable','TDS Receivable')",[],|r|r.get(0)).unwrap();
  assert_eq!((debit,credit),(0,0));
  assert_eq!(version,4);
  assert_eq!(tds_ledgers,2);
 }


 #[test] fn opening_balance_batch_requires_balanced_all_ledger_input(){
  let mut conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let ids:Vec<(i64,String)>={
   let mut s=conn.prepare("SELECT id,name FROM ledgers ORDER BY id").unwrap();
   s.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap()
  };
  let cash=ids.iter().find(|(_,n)|n=="Cash").unwrap().0;
  let capital=ids.iter().find(|(_,n)|n=="Capital Account").unwrap().0;
  let entries:Vec<OpeningBalanceEntry>=ids.iter().map(|(id,name)|OpeningBalanceEntry{ledger_id:*id,amount:if *id==cash{100000}else if *id==capital{100000}else{0},side:if name=="Capital Account"{"Cr".into()}else{"Dr".into()}}).collect();
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
  save_opening_balances_tx(&tx,&entries).unwrap();tx.commit().unwrap();
  let (dr,cr):(i64,i64)=conn.query_row("SELECT SUM(CASE WHEN balance_type='Dr' THEN opening_balance ELSE 0 END),SUM(CASE WHEN balance_type='Cr' THEN opening_balance ELSE 0 END) FROM ledgers",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
  assert_eq!((dr,cr),(100000,100000));
  let audit:i64=conn.query_row("SELECT COUNT(*) FROM audit_log WHERE event_type='SET_OPENING_BALANCES'",[],|r|r.get(0)).unwrap();assert_eq!(audit,1);
 }
 #[test] fn opening_balance_batch_rejects_difference_and_rolls_back(){
  let mut conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let ids:Vec<(i64,String)>={
   let mut s=conn.prepare("SELECT id,name FROM ledgers ORDER BY id").unwrap();
   s.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap()
  };
  let cash=ids.iter().find(|(_,n)|n=="Cash").unwrap().0;
  let capital=ids.iter().find(|(_,n)|n=="Capital Account").unwrap().0;
  let entries:Vec<OpeningBalanceEntry>=ids.iter().map(|(id,name)|OpeningBalanceEntry{ledger_id:*id,amount:if *id==cash{100000}else if *id==capital{90000}else{0},side:if name=="Capital Account"{"Cr".into()}else{"Dr".into()}}).collect();
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
  assert!(save_opening_balances_tx(&tx,&entries).is_err());drop(tx);
  let sum:i64=conn.query_row("SELECT SUM(opening_balance) FROM ledgers",[],|r|r.get(0)).unwrap();assert_eq!(sum,0);
  let audit:i64=conn.query_row("SELECT COUNT(*) FROM audit_log WHERE event_type='SET_OPENING_BALANCES'",[],|r|r.get(0)).unwrap();assert_eq!(audit,0);
 }
 #[test] fn opening_balance_batch_is_locked_after_voucher_posting(){
  let mut conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let ids:Vec<(i64,String)>={
   let mut s=conn.prepare("SELECT id,name FROM ledgers ORDER BY id").unwrap();
   s.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<Result<Vec<_>,_>>().unwrap()
  };
  let cash=ids.iter().find(|(_,n)|n=="Cash").unwrap().0;
  let capital=ids.iter().find(|(_,n)|n=="Capital Account").unwrap().0;
  let input=manual_voucher("opening-lock-voucher","Journal","2026-11-07","seed",vec![e(cash,10000,0),e(capital,0,10000)]);
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();post_voucher_tx(&tx,&input,"2026-04-01","2027-03-31").unwrap();tx.commit().unwrap();
  let entries:Vec<OpeningBalanceEntry>=ids.iter().map(|(id,name)|OpeningBalanceEntry{ledger_id:*id,amount:if *id==cash{100000}else if *id==capital{100000}else{0},side:if name=="Capital Account"{"Cr".into()}else{"Dr".into()}}).collect();
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();assert!(save_opening_balances_tx(&tx,&entries).is_err());drop(tx);
 }

 #[test] fn tds_rule_schema_is_created_and_versioned(){
  let conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let count:i64=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='tds_rules'",[],|r|r.get(0)).unwrap();
  let version:i64=conn.query_row("SELECT MAX(version) FROM schema_migrations",[],|r|r.get(0)).unwrap();
  assert_eq!((count,version),(1,4));
 }
 fn tds_rule(id:Option<i64>,from:&str,to:Option<&str>)->TdsRuleInput{
  TdsRuleInput{id,section_code:"194C".into(),nature_of_payment:"Contract payments".into(),description:"User-maintained rule".into(),rate_bps:100,no_pan_rate_bps:None,threshold_paise:100000,threshold_basis:"AnnualAggregate".into(),effective_from:from.into(),effective_to:to.map(str::to_string),source_reference:"Verified official source reference".into(),notes:"Reviewed for this effective period".into(),is_active:true}
 }
 #[test] fn tds_rule_requires_source_and_valid_rates_and_dates(){
  let mut input=tds_rule(None,"2026-04-01",None);
  input.source_reference=" ".into();assert!(validate_tds_rule(&input).is_err());
  let mut input=tds_rule(None,"2026-04-01",None);input.rate_bps=10001;assert!(validate_tds_rule(&input).is_err());
  let input=tds_rule(None,"2026-04-01",Some("2026-03-31"));assert!(validate_tds_rule(&input).is_err());
  let input=tds_rule(None,"2026-04-01",Some("2027-03-31"));assert!(validate_tds_rule(&input).is_ok());
 }
 #[test] fn tds_rule_effective_periods_reject_overlap_but_allow_adjacent_ranges(){
  let mut conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let first={let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();let id=save_tds_rule_tx(&tx,tds_rule(None,"2026-04-01",Some("2026-12-31"))).unwrap();tx.commit().unwrap();id};
  {
   let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
   assert!(save_tds_rule_tx(&tx,tds_rule(None,"2026-10-01",None)).is_err());drop(tx);
  }
  let second={let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();let id=save_tds_rule_tx(&tx,tds_rule(None,"2027-01-01",None)).unwrap();tx.commit().unwrap();id};
  assert!(second>first);
  let count:i64=conn.query_row("SELECT COUNT(*) FROM tds_rules WHERE section_code='194C'",[],|r|r.get(0)).unwrap();assert_eq!(count,2);
  let audit:i64=conn.query_row("SELECT COUNT(*) FROM audit_log WHERE entity_type='TDS_RULE'",[],|r|r.get(0)).unwrap();assert_eq!(audit,2);
 }
 #[test] fn core_migration_adds_idempotency_and_append_only_audit_log(){
  let conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let has_request=has_column(&conn,"vouchers","request_id").unwrap();
  let audit_table:i64=conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='audit_log'",[],|r|r.get(0)).unwrap();
  let version:i64=conn.query_row("SELECT MAX(version) FROM schema_migrations",[],|r|r.get(0)).unwrap();
  assert!(has_request);assert_eq!(audit_table,1);assert_eq!(version,3);
  conn.execute("INSERT INTO audit_log(event_type,entity_type,entity_id,description) VALUES('TEST','VOUCHER','1','test')",[]).unwrap();
  assert!(conn.execute("UPDATE audit_log SET description='changed' WHERE id=1",[]).is_err());
  assert!(conn.execute("DELETE FROM audit_log WHERE id=1",[]).is_err());
 }
 fn manual_voucher(request_id:&str,kind:&str,date:&str,narration:&str,entries:Vec<VoucherEntry>)->NewVoucher{
  NewVoucher{request_id:request_id.into(),voucher_type:kind.into(),voucher_date:date.into(),narration:narration.into(),entries}
 }
 #[test] fn manual_voucher_rejects_internal_sales_type(){
  let mut conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
  let input=manual_voucher("manual-sales","Sales","2026-11-07","not an invoice",vec![e(1,10000,0),e(2,0,10000)]);
  let result=post_voucher_tx(&tx,&input,"2026-04-01","2027-03-31");
  assert!(result.is_err());drop(tx);
  let count:i64=conn.query_row("SELECT COUNT(*) FROM vouchers",[],|r|r.get(0)).unwrap();assert_eq!(count,0);
 }
 #[test] fn manual_voucher_rejects_dates_outside_company_financial_year(){
  let mut conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let cash:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Cash'",[],|r|r.get(0)).unwrap();
  let capital:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Capital Account'",[],|r|r.get(0)).unwrap();
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
  let input=manual_voucher("out-of-year","Journal","2025-03-31","old year",vec![e(cash,10000,0),e(capital,0,10000)]);
  assert!(post_voucher_tx(&tx,&input,"2026-04-01","2027-03-31").is_err());drop(tx);
  let count:i64=conn.query_row("SELECT COUNT(*) FROM vouchers",[],|r|r.get(0)).unwrap();assert_eq!(count,0);
 }
 #[test] fn manual_voucher_posting_is_durably_idempotent_and_audited(){
  let mut conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let cash:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Cash'",[],|r|r.get(0)).unwrap();
  let capital:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Capital Account'",[],|r|r.get(0)).unwrap();
  let input=manual_voucher("stable-request-001","Journal","2026-11-07","Capital introduced",vec![e(cash,100000,0),e(capital,0,100000)]);
  let first={let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();let n=post_voucher_tx(&tx,&input,"2026-04-01","2027-03-31").unwrap();tx.commit().unwrap();n};
  let second={let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();let n=post_voucher_tx(&tx,&input,"2026-04-01","2027-03-31").unwrap();tx.commit().unwrap();n};
  assert_eq!(first,second);
  let count:i64=conn.query_row("SELECT COUNT(*) FROM vouchers",[],|r|r.get(0)).unwrap();
  let audit:i64=conn.query_row("SELECT COUNT(*) FROM audit_log WHERE entity_type='VOUCHER'",[],|r|r.get(0)).unwrap();
  assert_eq!((count,audit),(1,1));
  let changed=manual_voucher("stable-request-001","Journal","2026-11-07","Different narration",vec![e(cash,100000,0),e(capital,0,100000)]);
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
  assert!(post_voucher_tx(&tx,&changed,"2026-04-01","2027-03-31").is_err());drop(tx);
  let final_count:i64=conn.query_row("SELECT COUNT(*) FROM vouchers",[],|r|r.get(0)).unwrap();assert_eq!(final_count,1);
 }
 #[test] fn payment_receipt_and_contra_enforce_cash_bank_sides(){
  let mut conn=Connection::open_in_memory().unwrap();initialize(&conn).unwrap();
  let cash:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Cash'",[],|r|r.get(0)).unwrap();
  let bank:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Bank Account'",[],|r|r.get(0)).unwrap();
  let office:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Office Expenses'",[],|r|r.get(0)).unwrap();
  let sales:i64=conn.query_row("SELECT id FROM ledgers WHERE name='Sales'",[],|r|r.get(0)).unwrap();
  let valid_payment=manual_voucher("payment-1","Payment","2026-11-07","Office paid",vec![e(office,10000,0),e(cash,0,10000)]);
  let valid_receipt=manual_voucher("receipt-1","Receipt","2026-11-07","Cash sale",vec![e(cash,10000,0),e(sales,0,10000)]);
  let valid_contra=manual_voucher("contra-1","Contra","2026-11-07","Cash deposited",vec![e(bank,10000,0),e(cash,0,10000)]);
  let invalid_payment=manual_voucher("payment-2","Payment","2026-11-07","Wrong side",vec![e(cash,10000,0),e(office,0,10000)]);
  let invalid_contra=manual_voucher("contra-2","Contra","2026-11-07","Expense line",vec![e(bank,10000,0),e(office,0,10000)]);
  for input in [valid_payment,valid_receipt,valid_contra]{
   let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
   post_voucher_tx(&tx,&input,"2026-04-01","2027-03-31").unwrap();tx.commit().unwrap();
  }
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
  assert!(post_voucher_tx(&tx,&invalid_payment,"2026-04-01","2027-03-31").is_err());drop(tx);
  let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate).unwrap();
  assert!(post_voucher_tx(&tx,&invalid_contra,"2026-04-01","2027-03-31").is_err());drop(tx);
  let count:i64=conn.query_row("SELECT COUNT(*) FROM vouchers",[],|r|r.get(0)).unwrap();assert_eq!(count,3);
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
 #[test] fn rejects_unknown_voucher_type(){assert!(validate_voucher_input("Unknown","2026-11-07",&[e(1,10000,0),e(2,0,10000)]).is_err());}
 #[test] fn validates_real_calendar_dates(){assert!(valid_iso_date("2024-02-29"));assert!(!valid_iso_date("2025-02-29"));assert!(!valid_iso_date("2026-13-01"));assert!(!valid_iso_date("2026-2-01"));assert!(valid_date_range("2026-04-01","2027-03-31"));assert!(!valid_date_range("2027-04-01","2027-03-31"));}
}
