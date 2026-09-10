use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use futures::{FutureExt, select};
use svalin_sysctl::system_report::SystemReporter;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::{
    client::state::persistent,
    message_streaming::{MessageFromAgent, agent::AgentMessageDispatcherHandle},
    mls::MlsAgent,
};

// Todo: repeat this periodically if it fails - also needs to account for dropping the welcome message to the server
// That basically means the server should send a signal to the agent upon connecting if this group is missing, causing a recreation
// The recreation will need to be checked though, so we're not accidentally deleting a modified group because of a malicious server
pub(super) async fn ensure_group_exists(
    mls: &MlsAgent,
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
    mls: Arc<MlsAgent>,
    messager_handle: AgentMessageDispatcherHandle,
    cancel: CancellationToken,
    notify: Arc<Notify>,
) {
    let mut reporter = SystemReporter::new();
    loop {
        if let Err(err) = send_system_report(&mls, &messager_handle, &mut reporter).await {
            tracing::error!("Failed to send system report: {}", err);
        }

        select! {
            _ = cancel.cancelled().fuse() => return,
            _ = notify.notified().fuse() => continue,
            _ = tokio::time::sleep(SYSTEM_REPORT_INTERVAL).fuse() => continue,
        }
    }
}

async fn send_system_report(
    mls: &MlsAgent,
    messager_handle: &AgentMessageDispatcherHandle,
    reporter: &mut SystemReporter,
) -> Result<(), anyhow::Error> {
    tracing::trace!("Generating and sending system report");
    let report = generate_system_report(reporter).await?;
    let message = mls.send_report(report).await?;
    messager_handle.send(MessageFromAgent::Mls(message)).await;
    tracing::trace!("System report sent");
    Ok(())
}

async fn generate_system_report(
    reporter: &mut SystemReporter,
) -> anyhow::Result<persistent::Report> {
    let system_report = reporter.create().await?;

    let report = persistent::Report {
        current_version_identifier: crate::commit().into(),
        system_report,
    };

    Ok(report)
}
