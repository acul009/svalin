use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use svalin_pki::mls::agent::MlsAgent;
use svalin_sysctl::sytem_report::SystemReport;
use tokio_util::sync::CancellationToken;

use crate::{
    message_streaming::{MessageFromAgent, agent::AgentMessageDispatcherHandle},
    remote_key_retriever::RemoteKeyRetriever,
    verifier::remote_verifier::RemoteVerifier,
};

// Todo: repeat this periodically if it fails - also needs to account for dropping the welcome message to the server
// That basically means the server should send a signal to the agent upon connecting if this group is missing, causing a recreation
// The recreation will need to be checked though, so we're not accidentally deleting a modified group because of a malicious server
pub(super) async fn ensure_group_exists(
    mls: &MlsAgent<RemoteKeyRetriever, RemoteVerifier>,
    messager_handle: &AgentMessageDispatcherHandle,
) -> Result<(), anyhow::Error> {
    if let Some(message) = mls
        .create_device_group_if_missing()
        .await
        .map_err(|err| anyhow!(err))?
    {
        messager_handle.send(MessageFromAgent::Mls(message)).await;
    }
    Ok(())
}

const SYSTEM_REPORT_INTERVAL: Duration = Duration::from_secs(60 * 30); // 24 hours
pub(super) async fn schedule_system_reports(
    mls: Arc<MlsAgent<RemoteKeyRetriever, RemoteVerifier>>,
    messager_handle: AgentMessageDispatcherHandle,
    cancel: CancellationToken,
) {
    loop {
        if let Err(err) = send_system_report(&mls, &messager_handle).await {
            tracing::error!("Failed to send system report: {}", err);
        }

        if cancel
            .run_until_cancelled(tokio::time::sleep(SYSTEM_REPORT_INTERVAL))
            .await
            .is_none()
        {
            return;
        }
    }
}

async fn send_system_report(
    mls: &MlsAgent<RemoteKeyRetriever, RemoteVerifier>,
    messager_handle: &AgentMessageDispatcherHandle,
) -> Result<(), anyhow::Error> {
    let report = SystemReport::create().await?;
    let message = mls.send_report(report).await?;
    messager_handle.send(MessageFromAgent::Mls(message)).await;
    Ok(())
}
