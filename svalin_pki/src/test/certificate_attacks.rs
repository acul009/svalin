#[test]
fn malformed_issuer_is_rejected_without_panicking() {
    use crate::{CertificateType, KeyPair, UnverifiedCertificate};
    use rcgen::PublicKeyData;
    let key = KeyPair::generate();
    let mut params = rcgen::CertificateParams::default();
    params.not_before = time::OffsetDateTime::now_utc();
    params.not_after = params.not_before + CertificateType::Agent.validity_duration();
    params.distinguished_name.push(
        rcgen::DnType::OrganizationalUnitName,
        CertificateType::Agent.to_string(),
    );
    params.distinguished_name.push(
        rcgen::DnType::CommonName,
        UnverifiedCertificate::compute_spki_hash(
            &key.export_public_key().subject_public_key_info(),
        )
        .to_string(),
    );
    let mut issuer_params = rcgen::CertificateParams::default();
    issuer_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, format!("a€{}", "a".repeat(60)));
    let issuer_key = KeyPair::generate();
    let issuer = rcgen::Issuer::new(issuer_params, issuer_key.rcgen());
    let cert = params.signed_by(key.rcgen(), &issuer).unwrap();
    let result = std::panic::catch_unwind(|| UnverifiedCertificate::from_der(cert.der().to_vec()));
    assert!(result.is_ok(), "untrusted certificate input must not panic");
    assert!(
        matches!(
            result.unwrap(),
            Err(crate::CertificateParseError::InvalidIssuer(_))
        ),
        "malformed issuer must be rejected"
    );
}
