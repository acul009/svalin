use anyhow::anyhow;
use openmls::{
    group::{GroupId, StagedCommit},
    prelude::{LeafNodeIndex, ProtocolVersion, Sender},
};
use openmls_rust_crypto::RustCrypto;

use crate::{
    CertificateType, SpkiHash, get_current_timestamp,
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
    ) -> anyhow::Result<()> {
        for to_verify in commit.credentials_to_verify() {
            let spki_hash: SpkiHash = to_verify.deserialized()?;
            self.verifier()
                .verify_spki_hash(&spki_hash, get_current_timestamp())
                .await?;
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
            let key_package = key_package
                .verify(&self.crypto, self.protocol_version, &self.verifier)
                .await?;
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

        let members = self.get_required_key_packages(&group_id, me).await?;

        let new_group = self
            .processor()
            .create_group(members, group_id.to_group_id())
            .await?;

        Ok(Some(new_group))
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
