use crate::error::AppError;

use super::models::{LocationRecord, WindowRecord};

#[derive(Debug)]
pub struct NewAccount {
    pub email: String,
    pub password: String,
    pub full_name: String,
    pub phone: String,
    pub document: String,
    pub role: String,
}

#[derive(Debug)]
pub struct NewClient {
    pub trade_name: String,
    pub legal_name: String,
    pub person_type: String,
    pub tax_id: String,
    pub pix_key_type: String,
    pub pix_key: String,
    pub bio: String,
}

#[derive(Debug)]
pub struct NewLocation {
    pub location: LocationRecord,
    pub windows: Vec<WindowRecord>,
}

#[derive(Default)]
pub struct FieldErrors {
    fields: Vec<(String, String)>,
}

impl FieldErrors {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    pub fn add(&mut self, field: &str, detail: impl Into<String>) {
        self.fields.push((field.to_string(), detail.into()));
    }

    pub fn finish(self, message: &str) -> Result<(), AppError> {
        if self.fields.is_empty() {
            Ok(())
        } else {
            Err(AppError::validation(message, self.fields))
        }
    }
}

pub fn validate_account(
    email: &str,
    password: &str,
    full_name: &str,
    phone: &str,
    document: &str,
    role: &str,
) -> Result<NewAccount, AppError> {
    let mut errors = FieldErrors::new();
    let email = normalize_email(email);
    if !is_email(&email) {
        errors.add("email", "informe um e-mail válido");
    }
    if let Err(detail) = validate_password(password) {
        errors.add("password", detail);
    }
    let full_name = full_name.trim();
    if full_name.chars().count() < 3 || full_name.chars().count() > 160 {
        errors.add("full_name", "use entre 3 e 160 caracteres");
    } else if !full_name.chars().any(|c| c.is_alphabetic()) {
        errors.add("full_name", "informe o nome de uma pessoa");
    }
    let phone = digits_only(phone);
    if !(10..=13).contains(&phone.len()) {
        errors.add("phone", "informe o telefone com DDD, só com números");
    }
    let document = digits_only(document);
    if !is_valid_cpf(&document) {
        errors.add("document", "CPF inválido");
    }
    if !matches!(role, "driver" | "host" | "both") {
        errors.add("role", "use driver, host ou both");
    }
    errors.finish("há campos inválidos no cadastro")?;
    Ok(NewAccount {
        email,
        password: password.to_string(),
        full_name: full_name.to_string(),
        phone,
        document,
        role: role.to_string(),
    })
}

pub fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < 8 || password.chars().count() > 72 {
        return Err("use entre 8 e 72 caracteres".into());
    }
    if !password.chars().any(|c| c.is_ascii_alphabetic())
        || !password.chars().any(|c| c.is_ascii_digit())
    {
        return Err("inclua pelo menos uma letra e um número".into());
    }
    Ok(())
}

pub fn validate_client(
    trade_name: &str,
    legal_name: Option<&str>,
    person_type: &str,
    tax_id: &str,
    pix_key_type: &str,
    pix_key: &str,
    bio: &str,
) -> Result<NewClient, AppError> {
    let mut errors = FieldErrors::new();
    let trade_name = trade_name.trim();
    if !(2..=160).contains(&trade_name.chars().count()) {
        errors.add("trade_name", "use entre 2 e 160 caracteres");
    }
    let legal_name = legal_name.unwrap_or(trade_name).trim();
    if !(2..=180).contains(&legal_name.chars().count()) {
        errors.add("legal_name", "use entre 2 e 180 caracteres");
    }
    if !matches!(person_type, "individual" | "mei" | "company") {
        errors.add("person_type", "use individual, mei ou company");
    }
    let tax_id = digits_only(tax_id);
    match person_type {
        "company" if !is_valid_cnpj(&tax_id) => errors.add("tax_id", "CNPJ inválido"),
        "individual" | "mei" if !is_valid_cpf(&tax_id) => errors.add("tax_id", "CPF inválido"),
        _ => {}
    }
    if !matches!(pix_key_type, "email" | "cpf" | "cnpj" | "phone" | "random") {
        errors.add("pix_key_type", "use email, cpf, cnpj, phone ou random");
    }
    let pix_key = pix_key.trim();
    if pix_key.is_empty() || pix_key.chars().count() > 180 {
        errors.add("pix_key", "informe a chave Pix");
    } else if pix_key_type == "email" && !is_email(pix_key) {
        errors.add("pix_key", "a chave Pix de e-mail está inválida");
    }
    if bio.chars().count() > 1000 {
        errors.add("bio", "a apresentação pode ter no máximo 1000 caracteres");
    }
    errors.finish("há campos inválidos no credenciamento")?;
    Ok(NewClient {
        trade_name: trade_name.to_string(),
        legal_name: legal_name.to_string(),
        person_type: person_type.to_string(),
        tax_id,
        pix_key_type: pix_key_type.to_string(),
        pix_key: if pix_key_type == "email" {
            pix_key.to_lowercase()
        } else {
            pix_key.to_string()
        },
        bio: bio.trim().to_string(),
    })
}

pub struct LocationInput<'a> {
    pub label: &'a str,
    pub description: &'a str,
    pub address_line: &'a str,
    pub address_number: &'a str,
    pub complement: Option<&'a str>,
    pub district: &'a str,
    pub city: &'a str,
    pub state: &'a str,
    pub postal_code: &'a str,
    pub country: Option<&'a str>,
    pub latitude: f64,
    pub longitude: f64,
    pub connector: &'a str,
    pub power_kw: f64,
    pub voltage: i32,
    pub phases: i32,
    pub price_per_minute_cents: i64,
    pub price_per_hour_cents: i64,
    pub install_type: &'a str,
    pub has_roof: bool,
    pub well_lit: bool,
    pub access_note: Option<&'a str>,
    pub spot_code: Option<&'a str>,
    pub windows: &'a [WindowInput],
}

pub struct WindowInput {
    pub weekday: i32,
    pub start: String,
    pub end: String,
}

pub fn validate_location(
    input: LocationInput<'_>,
) -> Result<(LocationDraft, Vec<WindowDraft>), AppError> {
    let mut errors = FieldErrors::new();
    require_len(&mut errors, "label", input.label.trim(), 2, 120);
    if input.description.chars().count() > 1000 {
        errors.add("description", "use no máximo 1000 caracteres");
    }
    require_len(
        &mut errors,
        "address_line",
        input.address_line.trim(),
        2,
        180,
    );
    require_len(
        &mut errors,
        "address_number",
        input.address_number.trim(),
        1,
        20,
    );
    require_len(&mut errors, "district", input.district.trim(), 2, 80);
    require_len(&mut errors, "city", input.city.trim(), 2, 80);
    let state = input.state.trim().to_uppercase();
    if state.len() != 2 || !state.chars().all(|c| c.is_ascii_alphabetic()) {
        errors.add("state", "use a sigla do estado, como SP");
    }
    let postal_code = digits_only(input.postal_code);
    if postal_code.len() != 8 {
        errors.add("postal_code", "CEP precisa ter 8 dígitos");
    }
    let country = input.country.unwrap_or("BR").trim().to_uppercase();
    if country.len() != 2 {
        errors.add("country", "use a sigla do país, como BR");
    }
    if !input.latitude.is_finite() || !(-90.0..=90.0).contains(&input.latitude) {
        errors.add("latitude", "latitude precisa estar entre -90 e 90");
    }
    if !input.longitude.is_finite() || !(-180.0..=180.0).contains(&input.longitude) {
        errors.add("longitude", "longitude precisa estar entre -180 e 180");
    }
    if !matches!(
        input.connector,
        "home_outlet_220v" | "wallbox_type2" | "wallbox_ccs2"
    ) {
        errors.add(
            "connector",
            "use home_outlet_220v, wallbox_type2 ou wallbox_ccs2",
        );
    }
    if !input.power_kw.is_finite() || !(0.5..=350.0).contains(&input.power_kw) {
        errors.add("power_kw", "a potência precisa estar entre 0,5 e 350 kW");
    }
    if !matches!(input.voltage, 110 | 127 | 220 | 380) {
        errors.add("voltage", "use 110, 127, 220 ou 380");
    }
    if !matches!(input.phases, 1 | 3) {
        errors.add("phases", "use 1 ou 3");
    }
    if !(1..=100_000).contains(&input.price_per_minute_cents) {
        errors.add(
            "price_per_minute_cents",
            "o preço por minuto precisa ser maior que zero",
        );
    }
    if !(1..=1_000_000).contains(&input.price_per_hour_cents) {
        errors.add(
            "price_per_hour_cents",
            "o preço por hora precisa ser maior que zero",
        );
    }
    if !matches!(
        input.install_type,
        "home_outlet" | "wallbox" | "shared_garage"
    ) {
        errors.add("install_type", "use home_outlet, wallbox ou shared_garage");
    }
    let complement = clean_optional(input.complement, 80);
    let access_note = clean_optional(input.access_note, 500);
    let spot_code = clean_optional(input.spot_code, 40);
    if input.windows.is_empty() {
        errors.add("windows", "informe pelo menos um horário de funcionamento");
    }
    let mut windows = Vec::new();
    for (index, window) in input.windows.iter().enumerate() {
        if !(0..=6).contains(&window.weekday) {
            errors.add(
                "windows",
                format!("janela {index}: weekday vai de 0 (domingo) a 6 (sábado)"),
            );
            continue;
        }
        match (parse_clock(&window.start), parse_clock(&window.end)) {
            (Some(start), Some(end)) if end > start => windows.push(WindowDraft {
                weekday: window.weekday,
                start_min: start,
                end_min: end,
            }),
            (Some(_), Some(_)) => errors.add(
                "windows",
                format!("janela {index}: o horário final precisa ser depois do inicial"),
            ),
            _ => errors.add(
                "windows",
                format!("janela {index}: use horários no formato HH:MM"),
            ),
        }
    }
    errors.finish("há campos inválidos no ponto de recarga")?;
    Ok((
        LocationDraft {
            label: input.label.trim().to_string(),
            description: input.description.trim().to_string(),
            address_line: input.address_line.trim().to_string(),
            address_number: input.address_number.trim().to_string(),
            complement,
            district: input.district.trim().to_string(),
            city: input.city.trim().to_string(),
            state,
            postal_code,
            country,
            latitude: input.latitude,
            longitude: input.longitude,
            connector: input.connector.to_string(),
            power_kw: input.power_kw,
            voltage: input.voltage,
            phases: input.phases,
            price_per_minute_cents: input.price_per_minute_cents,
            price_per_hour_cents: input.price_per_hour_cents,
            install_type: input.install_type.to_string(),
            has_roof: input.has_roof,
            well_lit: input.well_lit,
            access_note,
            spot_code,
        },
        windows,
    ))
}

#[derive(Debug)]
pub struct LocationDraft {
    pub label: String,
    pub description: String,
    pub address_line: String,
    pub address_number: String,
    pub complement: Option<String>,
    pub district: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub country: String,
    pub latitude: f64,
    pub longitude: f64,
    pub connector: String,
    pub power_kw: f64,
    pub voltage: i32,
    pub phases: i32,
    pub price_per_minute_cents: i64,
    pub price_per_hour_cents: i64,
    pub install_type: String,
    pub has_roof: bool,
    pub well_lit: bool,
    pub access_note: Option<String>,
    pub spot_code: Option<String>,
}

#[derive(Debug)]
pub struct WindowDraft {
    pub weekday: i32,
    pub start_min: i32,
    pub end_min: i32,
}

pub fn parse_clock(value: &str) -> Option<i32> {
    let (hour, minute) = value.split_once(':')?;
    if hour.len() != 2 || minute.len() != 2 {
        return None;
    }
    let hour: i32 = hour.parse().ok()?;
    let minute: i32 = minute.parse().ok()?;
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
        return None;
    }
    Some(hour * 60 + minute)
}

fn require_len(errors: &mut FieldErrors, field: &str, value: &str, min: usize, max: usize) {
    let size = value.chars().count();
    if !(min..=max).contains(&size) {
        errors.add(field, format!("use entre {min} e {max} caracteres"));
    }
}

fn clean_optional(value: Option<&str>, max: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.chars().take(max).collect())
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn is_email(email: &str) -> bool {
    let Some((name, domain)) = email.split_once('@') else {
        return false;
    };
    !name.is_empty()
        && !name.contains(' ')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && email.len() <= 180
}

pub fn digits_only(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_digit()).collect()
}

pub fn is_valid_cpf(digits: &str) -> bool {
    if digits.len() != 11
        || digits
            .chars()
            .all(|c| c == digits.chars().next().unwrap_or('0'))
    {
        return false;
    }
    check_digit(digits, 9) && check_digit(digits, 10)
}

pub fn is_valid_cnpj(digits: &str) -> bool {
    if digits.len() != 14
        || digits
            .chars()
            .all(|c| c == digits.chars().next().unwrap_or('0'))
    {
        return false;
    }
    let first = cnpj_digit(&digits[..12], &[5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]);
    let second = cnpj_digit(&digits[..13], &[6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]);
    digits.as_bytes()[12] == first && digits.as_bytes()[13] == second
}

fn check_digit(digits: &str, length: usize) -> bool {
    let sum: u32 = digits
        .bytes()
        .take(length)
        .enumerate()
        .map(|(index, byte)| u32::from(byte - b'0') * (length as u32 + 1 - index as u32))
        .sum();
    let rest = sum % 11;
    let expected = if rest < 2 { 0 } else { 11 - rest };
    digits.as_bytes()[length] - b'0' == expected as u8
}

fn cnpj_digit(digits: &str, weights: &[u32]) -> u8 {
    let sum: u32 = digits
        .bytes()
        .zip(weights)
        .map(|(byte, weight)| u32::from(byte - b'0') * weight)
        .sum();
    let rest = sum % 11;
    let digit = if rest < 2 { 0 } else { 11 - rest };
    b'0' + digit as u8
}

pub fn document_kind(kind: &str) -> Result<&'static str, AppError> {
    match kind {
        "identity" => Ok("identity"),
        "address_proof" => Ok("address_proof"),
        "charger_photo" => Ok("charger_photo"),
        "electrical_safety" => Ok("electrical_safety"),
        _ => Err(AppError::field(
            "kind",
            "use identity, address_proof, charger_photo ou electrical_safety",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_valid_cnpj, is_valid_cpf, validate_account, validate_location, LocationInput,
        WindowInput,
    };

    #[test]
    fn cpf_e_cnpj_conhecidos_passam_na_validacao() {
        assert!(is_valid_cpf("52998224725"));
        assert!(!is_valid_cpf("11111111111"));
        assert!(!is_valid_cpf("52998224726"));
        assert!(is_valid_cnpj("11222333000181"));
        assert!(!is_valid_cnpj("11222333000180"));
    }

    #[test]
    fn cadastro_rejeita_senha_fraca_e_cpf_invalido() {
        let weak = validate_account(
            "ana@wattly.dev",
            "curta",
            "Ana Souza",
            "11987654321",
            "52998224725",
            "host",
        );
        assert!(weak.is_err());
        let bad_cpf = validate_account(
            "ana@wattly.dev",
            "Recarga-2026",
            "Ana Souza",
            "11987654321",
            "12345678901",
            "host",
        );
        assert!(bad_cpf.is_err());
        assert!(validate_account(
            "ana@wattly.dev",
            "Recarga-2026",
            "Ana Souza",
            "11987654321",
            "529.982.247-25",
            "host",
        )
        .is_ok());
    }

    #[test]
    fn preco_negativo_e_rejeitado() {
        let window = WindowInput {
            weekday: 1,
            start: "08:00".into(),
            end: "18:00".into(),
        };
        let mut input = sample_location(&window);
        input.price_per_minute_cents = 0;
        assert!(validate_location(input).is_err());
    }

    fn sample_location(window: &WindowInput) -> LocationInput<'_> {
        LocationInput {
            label: "Garagem da Ana",
            description: "Vaga coberta",
            address_line: "Rua das Acacias",
            address_number: "120",
            complement: None,
            district: "Vila Mariana",
            city: "Sao Paulo",
            state: "sp",
            postal_code: "04101-000",
            country: None,
            latitude: -23.561414,
            longitude: -46.655881,
            connector: "home_outlet_220v",
            power_kw: 7.4,
            voltage: 220,
            phases: 1,
            price_per_minute_cents: 40,
            price_per_hour_cents: 1800,
            install_type: "home_outlet",
            has_roof: true,
            well_lit: true,
            access_note: Some("Interfone 12"),
            spot_code: Some("Vaga 2"),
            windows: std::slice::from_ref(window),
        }
    }
}
