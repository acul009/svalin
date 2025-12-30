use openmls::{
    group::{MlsGroupJoinConfig, PURE_CIPHERTEXT_WIRE_FORMAT_POLICY, WireFormatPolicy},
    prelude::SenderRatchetConfiguration,
};

impl super::Group {
    pub fn join_config() -> MlsGroupJoinConfig {
        MlsGroupJoinConfig::builder()
            .sender_ratchet_configuration(Self::ratchet_config())
            .wire_format_policy(Self::wire_format_policy())
            .build()
    }

    fn wire_format_policy() -> WireFormatPolicy {
        PURE_CIPHERTEXT_WIRE_FORMAT_POLICY
    }

    pub fn ratchet_config() -> SenderRatchetConfiguration {
        SenderRatchetConfiguration::new(0, 0)
    }
}
