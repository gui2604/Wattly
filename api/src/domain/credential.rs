use crate::error::AppError;

pub const DRAFT: &str = "draft";
pub const SUBMITTED: &str = "submitted";
pub const UNDER_REVIEW: &str = "under_review";
pub const APPROVED: &str = "approved";
pub const REJECTED: &str = "rejected";
pub const SUSPENDED: &str = "suspended";
pub const REVOKED: &str = "revoked";

pub const REQUIRED_DOCUMENTS: [&str; 3] = ["identity", "address_proof", "charger_photo"];
pub const REQUIRED_CHECKS: [&str; 4] = ["identity", "address", "electrical", "connector"];

const TRANSITIONS: &[(&str, &str)] = &[
    (DRAFT, SUBMITTED),
    (SUBMITTED, UNDER_REVIEW),
    (UNDER_REVIEW, APPROVED),
    (UNDER_REVIEW, REJECTED),
    (REJECTED, DRAFT),
    (APPROVED, SUSPENDED),
    (SUSPENDED, APPROVED),
    (APPROVED, REVOKED),
    (SUSPENDED, REVOKED),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewAction {
    Start,
    Approve,
    Reject,
    Suspend,
    Reactivate,
    Revoke,
}

impl ReviewAction {
    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "start" => Ok(Self::Start),
            "approve" => Ok(Self::Approve),
            "reject" => Ok(Self::Reject),
            "suspend" => Ok(Self::Suspend),
            "reactivate" => Ok(Self::Reactivate),
            "revoke" => Ok(Self::Revoke),
            _ => Err(AppError::field(
                "action",
                "use start, approve, reject, suspend, reactivate ou revoke",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "review_started",
            Self::Approve => "approved",
            Self::Reject => "rejected",
            Self::Suspend => "suspended",
            Self::Reactivate => "reactivated",
            Self::Revoke => "revoked",
        }
    }

    pub fn target(self) -> &'static str {
        match self {
            Self::Start => UNDER_REVIEW,
            Self::Approve | Self::Reactivate => APPROVED,
            Self::Reject => REJECTED,
            Self::Suspend => SUSPENDED,
            Self::Revoke => REVOKED,
        }
    }
}

pub fn transition(from: &str, to: &str) -> Result<(), AppError> {
    if TRANSITIONS.contains(&(from, to)) {
        Ok(())
    } else {
        Err(AppError::Conflict(format!(
            "a transição de {from} para {to} não é permitida"
        )))
    }
}

pub fn validate_ready(location_count: usize, document_kinds: &[String]) -> Result<(), AppError> {
    let mut fields = Vec::new();
    if location_count == 0 {
        fields.push((
            "locations".into(),
            "cadastre pelo menos um ponto de recarga antes de enviar".into(),
        ));
    }
    for kind in REQUIRED_DOCUMENTS {
        if !document_kinds.iter().any(|item| item == kind) {
            fields.push(("documents".into(), format!("falta o documento {kind}")));
        }
    }
    if fields.is_empty() {
        Ok(())
    } else {
        Err(AppError::validation(
            "o credenciamento ainda não pode ser enviado",
            fields,
        ))
    }
}

pub fn validate_checks(checks: &[(String, bool)]) -> Result<(), AppError> {
    let mut fields = Vec::new();
    for item in REQUIRED_CHECKS {
        match checks.iter().find(|(code, _)| code == item) {
            Some((_, true)) => {}
            Some((_, false)) => fields.push((
                "checks".into(),
                format!("o item {item} precisa ser aprovado"),
            )),
            None => fields.push(("checks".into(), format!("falta o item {item}"))),
        }
    }
    if fields.is_empty() {
        Ok(())
    } else {
        Err(AppError::validation(
            "a aprovação exige o checklist completo",
            fields,
        ))
    }
}

pub fn location_status_for(client_status: &str) -> Option<&'static str> {
    match client_status {
        APPROVED => Some("active"),
        SUSPENDED => Some("suspended"),
        REVOKED => Some("inactive"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{transition, validate_checks, validate_ready, APPROVED, DRAFT};

    #[test]
    fn rascunho_pode_ser_enviado_e_nao_pode_ser_aprovado_direto() {
        assert!(transition(DRAFT, "submitted").is_ok());
        let err = transition(DRAFT, APPROVED).expect_err("transição inválida");
        let message = err.to_string();
        assert!(message.contains("não é permitida"), "{message}");
    }

    #[test]
    fn envio_exige_ponto_e_os_tres_documentos() {
        let err = validate_ready(0, &[]).expect_err("incompleto");
        assert!(err.to_string().contains("não pode ser enviado"));
        assert!(validate_ready(
            1,
            &[
                "identity".into(),
                "address_proof".into(),
                "charger_photo".into()
            ]
        )
        .is_ok());
    }

    #[test]
    fn aprovacao_exige_os_quatro_itens_do_checklist() {
        let checks = vec![
            ("identity".into(), true),
            ("address".into(), true),
            ("electrical".into(), false),
            ("connector".into(), true),
        ];
        assert!(validate_checks(&checks).is_err());
        let checks = vec![
            ("identity".into(), true),
            ("address".into(), true),
            ("electrical".into(), true),
            ("connector".into(), true),
        ];
        assert!(validate_checks(&checks).is_ok());
    }
}
