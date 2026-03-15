use std::marker::PhantomData;

use crate::mls::processor::MlsProcessorHandle;

pub struct MlsClient {
    mls: MlsProcessorHandle,
}

impl MlsClient {
    pub async fn join_my_device_group(
        &self,
        group_info: DeviceGroupCreationInfo,
    ) -> Result<(), JoinDeviceGroupError> {
        let provider = self.provider.clone();
        let me = self.svalin_credential.get_certificate().spki_hash().clone();
        let my_parent = self.svalin_credential.get_certificate().issuer().clone();

        let welcome = group_info.welcome()?;

        let ratchet_tree = group_info.ratchet_tree()?;

        let join_config = MlsGroupJoinConfig::builder()
            .max_past_epochs(0)
            .use_ratchet_tree_extension(false)
            .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .sender_ratchet_configuration(SenderRatchetConfiguration::new(0, 0))
            .build();

        let welcome = StagedWelcome::new_from_welcome(
            provider.as_ref(),
            &join_config,
            welcome,
            Some(ratchet_tree),
        )?;

        if welcome.group_context().group_id().as_slice() != me.as_slice() {
            return Err(JoinDeviceGroupError::WrongGroupId);
        }

        let creator: UnverifiedCertificate =
            welcome.welcome_sender()?.credential().deserialized()?;

        // Ensure there are only sessions and myself in the group
        welcome
            .members()
            .map(|member| -> Result<(), JoinDeviceGroupError> {
                let certificate: UnverifiedCertificate = member.credential.deserialized()?;
                if certificate.spki_hash() == &me {
                    return Ok(());
                }

                if certificate.certificate_type() != CertificateType::UserDevice {
                    return Err(JoinDeviceGroupError::WrongMemberType);
                }

                Ok(())
            })
            .collect::<Result<(), JoinDeviceGroupError>>()?;

        // TODO: check that members contains root
        if creator.issuer() != &my_parent {
            return Err(JoinDeviceGroupError::WrongGroupCreator);
        }

        let _group = welcome.into_group(provider.as_ref())?;

        Ok(())
    }
}
