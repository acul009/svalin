use openmls::{group::MlsGroupJoinConfig, prelude::SenderRatchetConfiguration};

impl super::Group {
    pub fn join_config() -> MlsGroupJoinConfig {
        MlsGroupJoinConfig::builder()
            .use_ratchet_tree_extension(true)
            .sender_ratchet_configuration(Self::ratchet_config())
            .build()
    }

    pub fn ratchet_config() -> SenderRatchetConfiguration {
        SenderRatchetConfiguration::new(0, 0)
    }
}
