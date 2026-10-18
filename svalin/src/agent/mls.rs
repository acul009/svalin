use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use svalin_sysctl::system_report::SystemReporter;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::{
    client::state::persistent,
    message_streaming::{MessageFromAgent, agent::AgentMessageDispatcherHandle},
    mls::MlsAgent,
};

pub enum MlsAction {
    SendReport,
    RecreateGroup(oneshot::Sender<Result<(), String>>),
}

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

const SYSTEM_REPORT_INTERVAL: Duration = Duration::from_secs(60 * 30);
pub(super) async fn run_mls_worker(
    mls: Arc<MlsAgent>,
    messager_handle: AgentMessageDispatcherHandle,
    cancel: CancellationToken,
    mut actions: mpsc::Receiver<MlsAction>,
) {
    let mut reporter = SystemReporter::new();
    let mut action = MlsAction::SendReport;
    loop {
        match action {
            MlsAction::SendReport => {
                if let Err(err) = send_system_report(&mls, &messager_handle, &mut reporter).await {
                    tracing::error!("Failed to send system report: {err}");
                }
            }
            MlsAction::RecreateGroup(response) => {
                let result = recreate_group(&mls, &messager_handle).await;
                let recreated = result.is_ok();
                if let Err(err) = &result {
                    tracing::error!("Failed to recreate device group: {err}");
                }
                let _ = response.send(result.map_err(|err| err.to_string()));
                if recreated {
                    action = MlsAction::SendReport;
                    continue;
                }
            }
        }

        action = tokio::select! {
            _ = cancel.cancelled() => return,
            action = actions.recv() => match action {
                Some(action) => action,
                None => return,
            },
            _ = tokio::time::sleep(SYSTEM_REPORT_INTERVAL) => MlsAction::SendReport,
        };
    }
}

async fn recreate_group(
    mls: &MlsAgent,
    messager_handle: &AgentMessageDispatcherHandle,
) -> anyhow::Result<()> {
    let message = mls
        .recreate_device_group()
        .await
        .map_err(|err| anyhow!(err))?;
    messager_handle
        .try_send(MessageFromAgent::Mls(message))
        .await
        .map_err(|_| anyhow!("Failed to submit recreated device group"))
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
