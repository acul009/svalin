use anyhow::anyhow;
use openmls::{
    group::{GroupId, StagedCommit},
    prelude::{Credential, LeafNodeIndex, Proposal, ProtocolVersion, Sender, tls_codec},
};
use openmls_rust_crypto::RustCrypto;

use crate::{
    Certificate, CertificateType, SpkiHash, VerifyError, get_current_timestamp,
    mls::{
        SvalinGroupId,
        agent::CreateSvalinGroupError,
        key_package::{KeyPackage, KeyPackageError, UnverifiedKeyPackage},
        key_retriever::{self},
        processor::MlsProcessorHandle,
        transport_types::MessageToServerTransport,
    },
};

pub(crate) struct MlsHarness<KeyRetriever, Verifier, Processor> {
    key_retriever: KeyRetriever,
    verifier: Verifier,
    processor: Processor,
    protocol_version: ProtocolVersion,
    crypto: RustCrypto,
}

pub(crate) trait AnyMlsProcessor {
    fn get_member(
        &self,
        group_id: GroupId,
        index: LeafNodeIndex,
    ) -> impl Future<Output = anyhow::Result<SpkiHash>>;
}

impl<KeyRetriever, Verifier, Processor> MlsHarness<KeyRetriever, Verifier, Processor> {
    pub fn new(key_retriever: KeyRetriever, verifier: Verifier, processor: Processor) -> Self {
        Self {
            key_retriever,
            verifier,
            processor,
            protocol_version: ProtocolVersion::default(),
            crypto: RustCrypto::default(),
        }
    }

    pub fn key_retriever(&self) -> &KeyRetriever {
        &self.key_retriever
    }

    pub fn verifier(&self) -> &Verifier {
        &self.verifier
    }

    pub fn processor(&self) -> &Processor {
        &self.processor
    }

    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    pub fn crypto(&self) -> &RustCrypto {
        &self.crypto
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityVerificationError {
    #[error("invalid MLS credential: {0}")]
    Credential(#[from] tls_codec::Error),
    #[error("certificate verification failed: {0}")]
    Certificate(#[from] VerifyError),
    #[error("MLS signature key does not match the certificate for {0:?}")]
    SignatureKeyMismatch(SpkiHash),
}

impl<KeyRetriever, Verifier: crate::Verifier, Processor>
    MlsHarness<KeyRetriever, Verifier, Processor>
{
    pub async fn verify_identity(
        &self,
        credential: &Credential,
        signature_key: &[u8],
    ) -> Result<Certificate, IdentityVerificationError> {
        let identity: SpkiHash = credential.deserialized()?;
        let certificate = self
            .verifier
            .verify_spki_hash(&identity, get_current_timestamp())
            .await?;
        if signature_key != certificate.public_key() {
            return Err(IdentityVerificationError::SignatureKeyMismatch(identity));
        }
        Ok(certificate)
    }
}

// Shared between client, agent and server
impl<KeyRetriever, Verifier, Processor> MlsHarness<KeyRetriever, Verifier, Processor>
where
    KeyRetriever: key_retriever::KeyRetriever,
    Verifier: crate::Verifier,
    Processor: AnyMlsProcessor,
{
    pub async fn check_commit(
        &self,
        group_id: &SvalinGroupId,
        commit: &StagedCommit,
        sender: &SpkiHash,
    ) -> anyhow::Result<()> {
        // Add/update leaves are verified with their keys below. Keep the
        // credential-only check for external senders in context extensions:
        // OpenMLS doesn't expose their credential/key pairs through public APIs.
        if commit
            .queued_proposals()
            .any(|proposal| matches!(proposal.proposal(), Proposal::GroupContextExtensions(_)))
        {
            for credential in commit.credentials_to_verify() {
                let identity: SpkiHash = credential.deserialized()?;
                self.verifier()
                    .verify_spki_hash(&identity, get_current_timestamp())
                    .await?;
            }
        }

        for proposal in commit.update_proposals() {
            let leaf = proposal.update_proposal().leaf_node();
            let identity = self
                .verify_identity(leaf.credential(), leaf.signature_key().as_slice())
                .await?;
            let Sender::Member(index) = proposal.sender() else {
                anyhow::bail!("Only members can update their identity");
            };
            let previous = self
                .processor()
                .get_member(group_id.to_group_id(), *index)
                .await?;
            anyhow::ensure!(
                identity.spki_hash() == &previous,
                "Updates cannot change member identity"
            );
        }
        if let Some(leaf) = commit.update_path_leaf_node() {
            let identity = self
                .verify_identity(leaf.credential(), leaf.signature_key().as_slice())
                .await?;
            anyhow::ensure!(
                identity.spki_hash() == sender,
                "Updates cannot change member identity"
            );
        }

        let required_members = self
            .key_retriever()
            .get_required_group_members(group_id)
            .await
            .map_err(|err| anyhow!(err))?;

        for proposal in commit.remove_proposals() {
            let Sender::Member(sender) = proposal.sender() else {
                anyhow::bail!("Only members can remove members")
            };
            let sender = self
                .processor()
                .get_member(group_id.to_group_id(), sender.clone())
                .await?;
            let sender = self
                .verifier()
                .verify_spki_hash(&sender, get_current_timestamp())
                .await?;
            check_edit_allowed(group_id, sender.certificate_type())?;

            let index = proposal.remove_proposal().removed();
            let spki_hash = self
                .processor
                .get_member(group_id.to_group_id(), index)
                .await?;
            if required_members.contains(&spki_hash) {
                anyhow::bail!("Cannot remove required member: {spki_hash:?}")
            }
        }

        for proposal in commit.add_proposals() {
            let Sender::Member(sender) = proposal.sender() else {
                anyhow::bail!("Only members can remove members")
            };
            let sender = self
                .processor()
                .get_member(group_id.to_group_id(), sender.clone())
                .await?;
            let sender = self
                .verifier()
                .verify_spki_hash(&sender, get_current_timestamp())
                .await?;
            check_edit_allowed(group_id, sender.certificate_type())?;

            let raw_key_package = proposal.add_proposal().key_package();
            let key_package = UnverifiedKeyPackage::new(raw_key_package.clone().into());
            let key_package = self.verify_key_package(key_package).await?;
            match key_package.certificate().certificate_type() {
                CertificateType::User => (),
                CertificateType::UserSession => (),
                certificate_type => {
                    anyhow::bail!(
                        "Unexpected certificate type in key package: {certificate_type:?}"
                    )
                }
            }
        }

        Ok(())
    }

    pub async fn verify_key_package(
        &self,
        key_package: UnverifiedKeyPackage,
    ) -> Result<KeyPackage, KeyPackageError> {
        key_package
            .verify(self.crypto(), self.protocol_version(), self.verifier())
            .await
    }
}

// shared between client and agent only
impl<KeyRetriever, Verifier> MlsHarness<KeyRetriever, Verifier, MlsProcessorHandle>
where
    KeyRetriever: key_retriever::KeyRetriever,
    Verifier: crate::Verifier,
{
    pub(crate) async fn create_group_if_not_exists(
        &self,
        group_id: &SvalinGroupId,
        me: &SpkiHash,
    ) -> Result<Option<MessageToServerTransport>, CreateSvalinGroupError<KeyRetriever::Error>> {
        if self
            .processor()
            .group_exists(group_id.to_group_id())
            .await?
        {
            return Ok(None);
        }

        Ok(Some(self.recreate_group(group_id, me).await?))
    }

    pub(crate) async fn recreate_group(
        &self,
        group_id: &SvalinGroupId,
        me: &SpkiHash,
    ) -> Result<MessageToServerTransport, CreateSvalinGroupError<KeyRetriever::Error>> {
        let members = self.get_required_key_packages(group_id, me).await?;

        Ok(self
            .processor()
            .create_group(members, group_id.to_group_id())
            .await?)
    }

    async fn get_required_key_packages(
        &self,
        group_id: &SvalinGroupId,
        me: &SpkiHash,
    ) -> Result<Vec<KeyPackage>, CreateSvalinGroupError<KeyRetriever::Error>>
    where
        KeyRetriever: crate::mls::key_retriever::KeyRetriever,
        Verifier: crate::Verifier,
    {
        let required_members = self
            .key_retriever()
            .get_required_group_members(&group_id)
            .await
            .map_err(CreateSvalinGroupError::KeyRetrieverError)?
            .into_iter()
            .filter(|spki_hash| spki_hash != me)
            .collect::<Vec<_>>();

        if required_members.is_empty() {
            tracing::trace!("no required key packages for group {group_id:?}");
            return Ok(Vec::new());
        }

        tracing::trace!("retrieving key packages for group {group_id:?}: {required_members:?}");

        let unverified = self
            .key_retriever()
            .get_key_packages(&required_members)
            .await
            .map_err(CreateSvalinGroupError::KeyRetrieverError)?;

        let mut members = Vec::with_capacity(unverified.len());
        for member in unverified {
            let member = self.verify_key_package(member).await?;
            members.push(member);
        }

        Ok(members)
    }
}

fn check_edit_allowed(
    group_id: &SvalinGroupId,
    certificate_type: CertificateType,
) -> anyhow::Result<()> {
    match certificate_type {
        CertificateType::Root => Ok(()),
        CertificateType::User => Ok(()),
        CertificateType::UserSession => Ok(()),
        _ => anyhow::bail!(
            "Cannot edit group {group_id:?} with certificate type {certificate_type:?}"
        ),
    }
}
