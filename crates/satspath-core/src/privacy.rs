use sha2::{Digest, Sha256};

pub fn canonical_identifier(identifier: &str) -> String {
    identifier.trim().to_ascii_lowercase()
}

pub fn identifier_hash(identifier: &str) -> String {
    // Hashed emails are not strong anonymity because emails are guessable.
    let digest = Sha256::digest(canonical_identifier(identifier).as_bytes());
    hex::encode(digest)
}

pub fn mask_identifier(identifier: &str) -> String {
    let trimmed = identifier.trim();
    if let Some((local, domain)) = trimmed.split_once('@') {
        let first = local.chars().next().unwrap_or('*');
        return format!("{first}***@{domain}");
    }
    mask_middle(trimmed, 3, 3)
}

pub fn mask_address(address: &str) -> String {
    mask_middle(address, 6, 3)
}

pub fn mask_pubkey(pubkey: &str) -> String {
    mask_middle(pubkey, 6, 4)
}

pub fn mask_invoice(invoice: &str) -> String {
    mask_middle(invoice, 8, 6)
}

fn mask_middle(value: &str, head: usize, tail: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= head + tail + 3 {
        return "***".into();
    }
    let start: String = chars.iter().take(head).collect();
    let end: String = chars.iter().skip(chars.len() - tail).collect();
    format!("{start}...{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_output_does_not_reveal_full_email_or_pubkey() {
        let email = "rodrigo@gmail.com";
        let pubkey = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let masked_email = mask_identifier(email);
        let masked_pubkey = mask_pubkey(pubkey);

        assert_eq!(masked_email, "r***@gmail.com");
        assert!(!masked_email.contains("rodrigo"));
        assert_eq!(masked_pubkey, "0279be...1798");
        assert!(!masked_pubkey.contains("667ef9dcbbac55"));
    }

    #[test]
    fn identifier_hash_uses_canonical_identifier() {
        assert_eq!(
            identifier_hash(" Rodrigo@Gmail.com "),
            identifier_hash("rodrigo@gmail.com")
        );
    }
}
