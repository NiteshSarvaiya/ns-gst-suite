
pub struct GstLineCalculation { pub taxable_value:i64,pub cgst:i64,pub sgst:i64,pub igst:i64,pub cess:i64,pub line_total:i64 }
fn round_rate(value:i64,basis_points:i64)->Result<i64,String>{
 if value<0||basis_points<0||basis_points>10000{return Err("Taxable value and tax rates must be non-negative; percentage rates cannot exceed 100%.".into())}
 let numerator=(value as i128).checked_mul(basis_points as i128).ok_or("Tax calculation overflow.")?+5000;
 i64::try_from(numerator/10000).map_err(|_|"Tax calculation overflow.".to_string())
}
pub fn calculate_line(quantity_milli:i64,unit_price_paise:i64,discount_paise:i64,gst_rate_bps:i64,cess_rate_bps:i64,interstate:bool)->Result<GstLineCalculation,String>{
 if quantity_milli<=0{return Err("Quantity must be greater than zero (up to three decimal places).".into())}
 if unit_price_paise<=0{return Err("Unit price must be greater than zero.".into())}
 if discount_paise<0{return Err("Discount cannot be negative.".into())}
 if gst_rate_bps>10000||cess_rate_bps>10000||gst_rate_bps<0||cess_rate_bps<0{return Err("GST and cess rates must be between 0% and 100%.".into())}
 let gross_num=(quantity_milli as i128).checked_mul(unit_price_paise as i128).ok_or("Line value overflow.")?+500;
 let gross=i64::try_from(gross_num/1000).map_err(|_|"Line value overflow.".to_string())?;
 if discount_paise>gross{return Err("Line discount cannot exceed the gross line value.".into())}
 let taxable_value=gross.checked_sub(discount_paise).ok_or("Taxable value overflow.")?;
 if taxable_value<=0{return Err("Taxable value must be greater than zero after discount.".into())}
 let total_gst=round_rate(taxable_value,gst_rate_bps)?;
 let (cgst,sgst,igst)=if interstate{(0,0,total_gst)}else{let half=total_gst/2;(half,total_gst-half,0)};
 let cess=round_rate(taxable_value,cess_rate_bps)?;
 let line_total=taxable_value.checked_add(total_gst).and_then(|v|v.checked_add(cess)).ok_or("Invoice line total overflow.")?;
 Ok(GstLineCalculation{taxable_value,cgst,sgst,igst,cess,line_total})
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test] fn intra_state_gst_splits_into_cgst_and_sgst(){let r=calculate_line(1000,100000,0,1800,0,false).unwrap();assert_eq!((r.taxable_value,r.cgst,r.sgst,r.igst,r.line_total),(100000,9000,9000,0,118000));}
 #[test] fn inter_state_gst_uses_igst_only(){let r=calculate_line(1000,100000,0,1800,0,true).unwrap();assert_eq!((r.cgst,r.sgst,r.igst,r.line_total),(0,0,18000,118000));}
 #[test] fn discounts_reduce_taxable_value_before_tax(){let r=calculate_line(1000,100000,10000,500,0,false).unwrap();assert_eq!((r.taxable_value,r.cgst,r.sgst,r.line_total),(90000,2250,2250,94500));}
 #[test] fn cess_is_calculated_separately_on_taxable_value(){let r=calculate_line(1000,100000,0,1800,100, true).unwrap();assert_eq!((r.igst,r.cess,r.line_total),(18000,1000,119000));}
 #[test] fn line_amount_rounding_is_to_nearest_paise(){let r=calculate_line(1000,10,0,500,0,true).unwrap();assert_eq!(r.igst,1);}
 #[test] fn rejects_invalid_quantity_discount_and_rate(){assert!(calculate_line(0,100,0,1800,0,false).is_err());assert!(calculate_line(1000,100,101,1800,0,false).is_err());assert!(calculate_line(1000,100,0,10001,0,false).is_err());}
}
