pub fn mask_document(digits: &str) -> String {
    let last = digits.chars().rev().take(2).collect::<String>();
    let last: String = last.chars().rev().collect();
    if digits.len() == 14 {
        format!("**.***.***/****-{last}")
    } else {
        format!("***.***.***-{last}")
    }
}

pub fn mask_pix(kind: &str, value: &str) -> String {
    if kind == "email" {
        match value.split_once('@') {
            Some((name, domain)) => {
                let initial = name.chars().next().unwrap_or('*');
                format!("{initial}***@{domain}")
            }
            None => mask_tail(value),
        }
    } else {
        mask_tail(value)
    }
}

fn mask_tail(value: &str) -> String {
    let count = value.chars().count();
    if count <= 4 {
        return "*".repeat(count);
    }
    let hidden = count - 4;
    let tail: String = value.chars().skip(hidden).collect();
    format!("{}{tail}", "*".repeat(hidden))
}

#[cfg(test)]
mod tests {
    use super::{mask_document, mask_pix};

    #[test]
    fn mascara_de_documento_mostra_so_o_final() {
        assert_eq!(mask_document("52998224725"), "***.***.***-25");
        assert_eq!(mask_document("11222333000181"), "**.***.***/****-81");
        assert!(!mask_document("52998224725").contains("529982247"));
    }

    #[test]
    fn mascara_de_pix_esconde_o_meio() {
        assert_eq!(mask_pix("email", "ana@wattly.dev"), "a***@wattly.dev");
        assert_eq!(mask_pix("cpf", "52998224725"), "*******4725");
    }
}
