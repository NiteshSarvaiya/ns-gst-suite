import { invoke } from "@tauri-apps/api/core";

export type Ledger={id:number;name:string;group_name:string;opening_balance:number;balance_type:"Dr"|"Cr"};
export type Voucher={id:number;voucher_no:string;voucher_type:string;voucher_date:string;narration:string;total:number};
export type TrialBalanceRow={id:number;name:string;group_name:string;debit:number;credit:number};
export type Entry={ledgerId:number;debit:number;credit:number};
export type NewVoucher={voucherType:string;voucherDate:string;narration:string;entries:Entry[]};

export async function connectDb(){await invoke("init_db");}
export const getLedgers=()=>invoke<Ledger[]>("list_ledgers");
export const getVouchers=()=>invoke<Voucher[]>("list_vouchers");
export const getTrialBalance=()=>invoke<TrialBalanceRow[]>("trial_balance");
export const postVoucher=(input:NewVoucher)=>invoke<string>("post_voucher",{input});
export const createLedger=(input:{name:string;groupName:string;openingBalance:number;balanceType:"Dr"|"Cr"})=>invoke<number>("create_ledger",{input});
