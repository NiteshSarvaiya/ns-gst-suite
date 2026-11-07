import { useEffect, useMemo, useState } from "react";
import { Activity, ArrowDownLeft, ArrowUpRight, BookOpen, Building2, ChevronDown, FileText, LayoutDashboard, Plus, ReceiptIndianRupee, Search, Settings2, ShieldCheck, Wallet } from "lucide-react";
import { connectDb, getLedgers, getVouchers, postVoucher, type Ledger, type Voucher } from "./db";

const money = (paise:number) => new Intl.NumberFormat("en-IN",{style:"currency",currency:"INR",maximumFractionDigits:2}).format(paise/100);
const today = new Date().toISOString().slice(0,10);

export default function App() {
  const [ready,setReady] = useState(false);
  const [native,setNative] = useState(false);
  const [ledgers,setLedgers] = useState<Ledger[]>([]);
  const [vouchers,setVouchers] = useState<Voucher[]>([]);
  const [view,setView] = useState("Dashboard");
  const [showEntry,setShowEntry] = useState(false);
  const [search,setSearch] = useState("");
  const [form,setForm] = useState({type:"Journal",date:today,narration:"",debitLedger:1,creditLedger:3,amount:""});
  const [error,setError] = useState("");
  const refresh = async () => { setLedgers(await getLedgers()); setVouchers(await getVouchers()); };
  useEffect(()=>{ (async()=>{ try { setNative(await connectDb()); await refresh(); } catch(e) { setError(String(e)); } finally { setReady(true); } })(); },[]);
  const totalDr = ledgers.reduce((s,l)=>s+(l.balance_type==="Dr"?l.opening_balance:-l.opening_balance),0);
  const filtered = useMemo(()=>vouchers.filter(v=>(v.voucher_no+" "+v.voucher_type+" "+v.narration).toLowerCase().includes(search.toLowerCase())),[vouchers,search]);
  async function save() {
    setError("");
    try {
      const rupees = Number(form.amount);
      if (!Number.isFinite(rupees) || rupees<=0) throw new Error("Enter an amount greater than zero.");
      await postVoucher({type:form.type,date:form.date,narration:form.narration,debitLedger:form.debitLedger,creditLedger:form.creditLedger,amount:Math.round(rupees*100)});
      await refresh(); setShowEntry(false); setForm({...form,narration:"",amount:""});
    } catch(e) { setError(e instanceof Error?e.message:String(e)); }
  }
  const nav = [{name:"Dashboard",icon:LayoutDashboard},{name:"Vouchers",icon:ReceiptIndianRupee},{name:"Ledgers",icon:BookOpen},{name:"Reports",icon:FileText}];
  if(!ready) return <div className="loading">Opening your books…</div>;
  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark">NS</div><div><b>NS Accounts</b><small>DESKTOP EDITION</small></div></div>
      <div className="company-switch"><Building2 size={17}/><span><b>Demo Enterprises</b><small>FY 2026–27 · India</small></span><ChevronDown size={15}/></div>
      <div className="nav-label">WORKSPACE</div>
      {nav.map(n=><button key={n.name} className={view===n.name?"nav-item active":"nav-item"} onClick={()=>setView(n.name)}><n.icon size={18}/>{n.name}</button>)}
      <div className="sidebar-bottom"><div className="secure"><ShieldCheck size={17}/><span>Local data storage<small>Your books stay on this device</small></span></div><button className="nav-item"><Settings2 size={18}/>Settings</button></div>
    </aside>
    <main className="main">
      <header className="topbar"><div><span className="crumb">Workspace</span><span className="slash">/</span><b>{view}</b></div><div className="top-actions"><span className={native?"db-status":"db-status demo"}><i/>{native?"Offline database connected":"Preview mode · sample data"}</span><div className="avatar">NS</div></div></header>
      <div className="page">
        <div className="page-heading"><div><p className="eyebrow">SATURDAY, 7 NOVEMBER 2026</p><h1>{view==="Dashboard"?"Good afternoon":view}</h1><p className="subheading">{view==="Dashboard"?"Here’s your business at a glance.":view==="Vouchers"?"Record and review your accounting transactions.":view==="Ledgers"?"Your chart of accounts and opening balances.":"Financial summaries will appear here as transactions are posted."}</p></div><button className="primary" onClick={()=>{setError("");setShowEntry(true)}}><Plus size={17}/> New voucher</button></div>
        {error && <div className="error">{error}</div>}
        {view==="Dashboard" && <>
          <div className="stats">
            <div className="stat-card"><div className="stat-top"><span>Opening net position</span><Wallet size={18}/></div><strong>{money(Math.abs(totalDr)*100)}</strong><small>From configured opening balances</small></div>
            <div className="stat-card"><div className="stat-top"><span>Vouchers recorded</span><ReceiptIndianRupee size={18}/></div><strong>{vouchers.length}</strong><small>Posted transactions</small></div>
            <div className="stat-card"><div className="stat-top"><span>Accounts</span><BookOpen size={18}/></div><strong>{ledgers.length}</strong><small>In your chart of accounts</small></div>
            <div className="stat-card"><div className="stat-top"><span>Data status</span><Activity size={18}/></div><strong className="status-word">{native?"On device":"Preview"}</strong><small>{native?"SQLite local database":"Browser demonstration only"}</small></div>
          </div>
          <section className="panel"><div className="panel-head"><div><h2>Recent vouchers</h2><p>Your latest accounting entries</p></div><button className="text-button" onClick={()=>setView("Vouchers")}>View all</button></div><VoucherTable items={vouchers.slice(0,6)}/></section>
          <div className="note"><ShieldCheck size={18}/><div><b>Built for reliable books</b><p>V1 starts with a double-entry voucher foundation. GST invoices, inventory, statutory reports and backups are the next milestones.</p></div></div>
        </>}
        {view==="Vouchers" && <section className="panel"><div className="panel-head"><div><h2>Voucher register</h2><p>{vouchers.length} entries</p></div><div className="search"><Search size={16}/><input placeholder="Search vouchers…" value={search} onChange={e=>setSearch(e.target.value)}/></div></div><VoucherTable items={filtered}/></section>}
        {view==="Ledgers" && <section className="panel"><div className="panel-head"><div><h2>Chart of accounts</h2><p>{ledgers.length} ledgers · sample opening balances</p></div></div><table><thead><tr><th>LEDGER NAME</th><th>GROUP</th><th>OPENING BALANCE</th><th>TYPE</th></tr></thead><tbody>{ledgers.map(l=><tr key={l.id}><td className="strong">{l.name}</td><td>{l.group_name}</td><td>{money(l.opening_balance*100)}</td><td><span className="tag">{l.balance_type}</span></td></tr>)}</tbody></table></section>}
        {view==="Reports" && <section className="panel report-placeholder"><FileText size={30}/><h2>Financial reports</h2><p>Trial balance, profit & loss, balance sheet and GST summaries will be enabled after the ledger posting and opening-balance workflows are completed.</p></section>}
      </div>
    </main>
    {showEntry && <div className="modal-backdrop" onMouseDown={e=>{if(e.target===e.currentTarget)setShowEntry(false)}}><section className="modal"><div className="modal-head"><div><p className="eyebrow">ACCOUNTING ENTRY</p><h2>New voucher</h2></div><button className="close" onClick={()=>setShowEntry(false)}>×</button></div><label>Voucher type<select value={form.type} onChange={e=>setForm({...form,type:e.target.value})}>{["Journal","Payment","Receipt","Contra"].map(x=><option key={x}>{x}</option>)}</select></label><label>Date<input type="date" value={form.date} onChange={e=>setForm({...form,date:e.target.value})}/></label><label>Debit ledger<select value={form.debitLedger} onChange={e=>setForm({...form,debitLedger:Number(e.target.value)})}>{ledgers.map(l=><option key={l.id} value={l.id}>{l.name}</option>)}</select></label><label>Credit ledger<select value={form.creditLedger} onChange={e=>setForm({...form,creditLedger:Number(e.target.value)})}>{ledgers.map(l=><option key={l.id} value={l.id}>{l.name}</option>)}</select></label><label>Amount (₹)<input inputMode="decimal" placeholder="0.00" value={form.amount} onChange={e=>setForm({...form,amount:e.target.value})}/></label><label>Narration<input placeholder="Brief description of transaction" value={form.narration} onChange={e=>setForm({...form,narration:e.target.value})}/></label><div className="modal-foot"><span>Debit = Credit</span><button className="primary" onClick={save}>Post voucher <ArrowUpRight size={16}/></button></div></section></div>}
  </div>;
}
function VoucherTable({items}:{items:Voucher[]}) { return <table><thead><tr><th>VOUCHER NO.</th><th>TYPE</th><th>DATE</th><th>NARRATION</th><th className="right">AMOUNT</th></tr></thead><tbody>{items.length===0?<tr><td colSpan={5} className="empty">No vouchers yet. Create your first entry.</td></tr>:items.map(v=><tr key={v.id}><td className="strong">{v.voucher_no}</td><td><span className="tag">{v.voucher_type}</span></td><td>{v.voucher_date}</td><td>{v.narration||"—"}</td><td className="right amount">{money(v.total)}</td></tr>)}</tbody></table>}