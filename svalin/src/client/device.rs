use anyhow::anyhow;
use std::{path::PathBuf, time::Duration};
use svalin_pki::{SpkiHash, Verifier, get_current_timestamp};
use svalin_rpc::{
    commands::{forward::ForwardConnection, ping::Ping},
    rpc::connection::{Connection, direct_connection::DirectConnection},
};
use svalin_store::client_store::persistent::{Message::UpdateMetaInfo, SvalinMetaInfo};
use tokio::sync::mpsc;

use crate::shared::commands::{
    request_system_report::RequestSystemReport, update_agent::UpdateAgent, update_mls::MlsUpdate,
};

pub struct DeviceHandle<'a>(&'a super::Client, SpkiHash);

impl<'a> DeviceHandle<'a> {
    pub(super) fn new(client: &'a super::Client, hash: SpkiHash) -> Self {
        Self(client, hash)
    }

    pub async fn ping(&self) -> anyhow::Result<Duration> {
        Ok(self.connection().await?.dispatch(Ping).await?)
    }

    pub async fn request_system_report(&self) -> anyhow::Result<()> {
        Ok(self
            .connection()
            .await?
            .dispatch(RequestSystemReport)
            .await
            .map_err(|err| anyhow!("{}", err))?)
    }

    pub async fn update_metainfo(&self, metainfo: SvalinMetaInfo) -> anyhow::Result<()> {
        self.0
            .mls_update_sender
            .send(MlsUpdate::UpdateMetaInfo(self.1.clone(), metainfo.clone()))
            .await
            .map_err(|_| anyhow!("error sending metainfo through channel"))?;

        self.0
            .state_handle
            .persistent_update(UpdateMetaInfo(self.1.clone(), metainfo))
            .await?;

        Ok(())
    }

    pub async fn update_agent(
        &self,
        file: PathBuf,
        progress: mpsc::Sender<f32>,
    ) -> anyhow::Result<()> {
        self.connection()
            .await?
            .dispatch(UpdateAgent { file, progress })
            .await
            .map_err(|err| anyhow!("{err}"))?;

        Ok(())
    }

    async fn connection(&self) -> anyhow::Result<ForwardConnection<DirectConnection>> {
        let cert = self
            .0
            .verifier
            .verify_spki_hash(&self.1, get_current_timestamp())
            .await?;

        let connection = ForwardConnection::new(
            self.0.rpc.upstream_connection(),
            self.0.device_credential.clone(),
            cert,
        );

        Ok(connection)
    }
}

// struct InstallInfoStarter {
//     connection: ForwardConnection<DirectConnection>,
// }

// impl SubscriberStarter for InstallInfoStarter {
//     type Item = RemoteData<InstallationInfo>;

//     fn default(&self) -> Self::Item {
//         RemoteData::Unavailable
//     }

//     fn start(
//         &self,
//         send: watch::Sender<Self::Item>,
//         _cancel: CancellationToken,
//     ) -> impl Future<Output = ()> + Send + 'static {
//         let connection = self.connection.clone();
//         let _ = send.send(RemoteData::Pending);

//         async move {
//             let send2 = send.clone();
//             if let Err(err) = connection
//                 .dispatch(InstallationInfoDispatcher { send })
//                 .await
//             {
//                 let _ = send2.send(RemoteData::Unavailable);
//                 error!("error while requesting InstallationInfo: {err}");
//             }
//         }
//     }
// }

// struct RealtimeStarter {
//     connection: ForwardConnection<DirectConnection>,
// }

// impl SubscriberStarter for RealtimeStarter {
//     type Item = RemoteData<RealtimeStatus>;

//     fn default(&self) -> Self::Item {
//         RemoteData::Unavailable
//     }

//     fn start(
//         &self,
//         send: watch::Sender<Self::Item>,
//         cancel: CancellationToken,
//     ) -> impl Future<Output = ()> + Send + 'static {
//         let connection = self.connection.clone();
//         let _ = send.send(RemoteData::Pending);

//         async move {
//             let send2 = send.clone();
//             if let Err(err) = connection
//                 .dispatch(SubscribeRealtimeStatus { cancel, send })
//                 .await
//             {
//                 let _ = send2.send(RemoteData::Unavailable);
//                 error!("error while requesting InstallationInfo: {err}");
//             }
//         }
//     }
// }
