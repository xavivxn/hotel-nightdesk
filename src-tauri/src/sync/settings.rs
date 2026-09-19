/// Business keys mirrored in `public.business_settings` (N12). Device keys stay local.
#[allow(dead_code)]
pub const BUSINESS_SETTING_KEYS: &[&str] = &[
    "business_name",
    "address",
    "phone",
    "tax_percent",
    "currency_symbol",
    "receipt_footer",
    "require_guest_name",
    "ticket_header",
    "ticket_header_name",
];

#[allow(dead_code)]
const DEVICE_SETTING_KEYS: &[&str] = &[
    "theme",
    "printer_enabled",
    "printer_path",
    "printer_name",
    "paper_width",
    "auto_print_on_checkout",
    "pin_hash",
    "device_mode",
];

#[allow(dead_code)]
pub fn is_business_key(key: &str) -> bool {
    BUSINESS_SETTING_KEYS.contains(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_excludes_device_keys() {
        for key in DEVICE_SETTING_KEYS {
            assert!(
                !is_business_key(key),
                "{key} must stay on the reception device"
            );
        }
        assert!(!BUSINESS_SETTING_KEYS.iter().any(|key| key.starts_with("sync_")));
        assert!(!BUSINESS_SETTING_KEYS.iter().any(|key| key.starts_with("printer_")));
    }

    #[test]
    fn whitelist_matches_postgres_business_settings() {
        let sql = include_str!("../../../supabase/migrations/20260918232847_schema.sql");
        let start = sql
            .find("CREATE TABLE public.business_settings")
            .expect("business_settings table");
        let block = &sql[start..];
        let check_at = block.find("CHECK (key IN (").expect("key check");
        let after = &block[check_at + "CHECK (key IN (".len()..];
        let end = after.find(')').expect("end of check");
        let mut remote: Vec<String> = after[..end]
            .split(',')
            .map(|part| part.trim().trim_matches('\'').to_string())
            .filter(|part| !part.is_empty())
            .collect();
        remote.sort();
        let mut local: Vec<String> = BUSINESS_SETTING_KEYS
            .iter()
            .map(|key| (*key).to_string())
            .collect();
        local.sort();
        assert_eq!(local, remote);
    }
}
