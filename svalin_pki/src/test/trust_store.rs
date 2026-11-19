use super::*;
use crate::{CertificateType, KeyPair, secure_chain::State as _};
use rcgen::PublicKeyData;
use time::{Duration, OffsetDateTime};

#[test]
fn child_devices_includes_indirect_agents_and_excludes_unrelated_certificates() {
    let start = OffsetDateTime::now_utc() - Duration::minutes(1);
    let root = credential_at(CertificateType::Root, start, None);
    let user = credential_at(CertificateType::User, start, Some(&root));
    let nested_user = credential_at(CertificateType::User, start, Some(&user));
    let direct = credential_at(CertificateType::Agent, start, Some(&user));
    let indirect = credential_at(CertificateType::Agent, start, Some(&nested_user));
    let unrelated = credential_at(CertificateType::Agent, start, Some(&root));
    let session = credential_at(CertificateType::UserSession, start, Some(&user));
    let mut store = TrustStore::initialize(
        root.certificate()
            .clone()
            .to_unverified()
            .use_as_root()
            .unwrap(),
    );
    for (child, parent) in [
        (&user, &root),
        (&nested_user, &user),
        (&direct, &user),
        (&indirect, &nested_user),
        (&unrelated, &root),
        (&session, &user),
    ] {
        let block = store.add(child.certificate().clone(), parent).unwrap();
        store.apply(block);
    }

    let devices: HashSet<_> = store
        .child_devices(user.certificate().spki_hash())
        .map(|cert| cert.spki_hash().clone())
        .collect();
    assert_eq!(
        devices,
        HashSet::from([
            direct.certificate().spki_hash().clone(),
            indirect.certificate().spki_hash().clone(),
        ])
    );
    assert_eq!(
        store.child_devices(root.certificate().spki_hash()).count(),
        3
    );
    assert_eq!(
        store
            .child_devices(direct.certificate().spki_hash())
            .count(),
        0
    );
    assert_eq!(
        store
            .child_devices(&SpkiHash::from_slice(&[0; 32]).unwrap())
            .count(),
        0
    );
}

fn credential_at(
    certificate_type: CertificateType,
    start: OffsetDateTime,
    issuer: Option<&Credential>,
) -> Credential {
    let key = KeyPair::generate();
    let spki = UnverifiedCertificate::compute_spki_hash(
        &key.export_public_key().subject_public_key_info(),
    );
    let mut params = rcgen::CertificateParams::default();
    params.not_before = start;
    params.not_after = start + certificate_type.validity_duration();
    params.is_ca = if matches!(
        certificate_type,
        CertificateType::Root | CertificateType::User
    ) {
        rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained)
    } else {
        rcgen::IsCa::NoCa
    };
    params.distinguished_name.push(
        rcgen::DnType::OrganizationalUnitName,
        certificate_type.to_string(),
    );
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, spki.to_string());
    let certificate = match issuer {
        Some(issuer) => {
            let issuer = rcgen::Issuer::from_ca_cert_der(
                &issuer.certificate().as_der().into(),
                issuer.keypair().rcgen(),
            )
            .unwrap();
            params.signed_by(key.rcgen(), &issuer).unwrap()
        }
        None => params.self_signed(key.rcgen()).unwrap(),
    };
    key.upgrade(UnverifiedCertificate::from_der(certificate.der().to_vec()).unwrap())
        .unwrap()
}

#[test]
fn expired_signer_cannot_add_certificate() {
    let root = credential_at(
        CertificateType::Root,
        OffsetDateTime::now_utc() - CertificateType::Root.validity_duration() - Duration::days(1),
        None,
    );
    let mut store = TrustStore::initialize(
        root.certificate()
            .clone()
            .to_unverified()
            .use_as_root()
            .unwrap(),
    );
    let certificate = root
        .create_agent_certificate_for_key(&KeyPair::generate().export_public_key())
        .unwrap();
    let digest = store.digest();
    assert!(matches!(
        store.add(certificate, &root),
        Err(CreateBlockError::InvalidTransaction(Error::SignerInvalid(
            ValidityError::Expired
        )))
    ));
    assert_eq!(store.digest(), digest);
}

#[test]
fn future_signer_cannot_add_certificate() {
    let root = credential_at(
        CertificateType::Root,
        OffsetDateTime::now_utc() + Duration::days(1),
        None,
    );
    let mut store = TrustStore::initialize(
        root.certificate()
            .clone()
            .to_unverified()
            .use_as_root()
            .unwrap(),
    );
    let certificate = root
        .create_agent_certificate_for_key(&KeyPair::generate().export_public_key())
        .unwrap();
    assert!(matches!(
        store.add(certificate, &root),
        Err(CreateBlockError::InvalidTransaction(Error::SignerInvalid(
            ValidityError::NotYetValid
        )))
    ));
}

#[test]
fn historical_transaction_uses_signer_validity_at_transaction_time() {
    let now = OffsetDateTime::now_utc();
    let root = credential_at(
        CertificateType::Root,
        now - CertificateType::Root.validity_duration() - Duration::days(1),
        None,
    );
    let historical_time = now - Duration::days(2);
    let agent = credential_at(CertificateType::Agent, historical_time, Some(&root));
    let state = State::initialize(
        root.certificate()
            .clone()
            .to_unverified()
            .use_as_root()
            .unwrap(),
    );
    let transaction = Transaction::Add(agent.certificate().clone().to_unverified());
    state
        .check(
            root.certificate(),
            historical_time.unix_timestamp() as u64,
            &transaction,
        )
        .unwrap();
    assert!(matches!(
        state.check(
            root.certificate(),
            now.unix_timestamp() as u64,
            &transaction
        ),
        Err(Error::SignerInvalid(ValidityError::Expired))
    ));
}
