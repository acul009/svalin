use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use clap::{Parser, Subcommand};
use svalin::{agent, installer, server::Server};

use tokio::runtime;
use tokio_util::sync::CancellationToken;
use tracing_subscriber;
#[cfg(target_os = "windows")]
use windows_service::define_windows_service;

#[derive(Debug, Parser)]
#[clap(name = "svalin", version)]
pub struct App {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run in server mode
    Server { address: String },
    /// Commands for running the agent
    Agent {
        #[clap(subcommand)]
        action: AgentAction,
    },
    /// Get version information
    Version,
}

#[derive(Debug, Subcommand)]
enum AgentAction {
    /// Run the agent with the already initiallized config
    Run {
        #[cfg_attr(target_os = "windows", clap(long, action))]
        #[cfg(target_os = "windows")]
        /// Run the agent as a windows service
        service: bool,
    },
    /// Install the agent with default settings
    Install,
    /// Uninstall the agent and delete all data
    Uninstall,
    /// Initialize the agent by connecting to a server
    Init { address: String },
}

#[cfg(target_os = "windows")]
define_windows_service!(ffi_service_agent, service_agent);

#[cfg(target_os = "windows")]
fn service_agent(_arguments: Vec<std::ffi::OsString>) {
    if let Err(err) = run_service_agent() {
        tracing::error!("Error running service agent: {:#?}", err);
    }
}

#[cfg(target_os = "windows")]
fn run_service_agent() -> anyhow::Result<()> {
    use windows_service::{
        service::{
            ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
            ServiceType,
        },
        service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
    };
    const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

    let cancel = CancellationToken::new();
    let cancel2 = cancel.clone();
    let status_handle_arc = Arc::new(Mutex::new(None::<ServiceStatusHandle>));
    let status_handle_arc2 = status_handle_arc.clone();

    let event_handler = move |control_event: ServiceControl| -> ServiceControlHandlerResult {
        use windows_service::service::ServiceControl;
        match control_event {
            ServiceControl::Stop => {
                cancel2.cancel();

                {
                    let _ = status_handle_arc2
                        .lock()
                        .unwrap()
                        .unwrap()
                        .set_service_status(ServiceStatus {
                            service_type: SERVICE_TYPE,
                            current_state: ServiceState::StopPending,
                            controls_accepted: ServiceControlAccept::empty(),
                            exit_code: ServiceExitCode::Win32(0),
                            checkpoint: 0,
                            wait_hint: Duration::from_secs(20),
                            process_id: None,
                        });
                }

                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle =
        service_control_handler::register(installer::WINDOWS_SERVICE_NAME, event_handler)?;
    {
        let mut slot = status_handle_arc.lock().unwrap();
        *slot = Some(status_handle.clone());
    }

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let run_result = run_async(run_agent(cancel));

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    });

    run_result
}

fn main() {
    tracing_subscriber::fmt::init();

    let app = App::parse();
    match app.command {
        Command::Server { address } => run_async(start_server(address)).unwrap(),
        Command::Agent { action } => match action {
            AgentAction::Run {
                #[cfg(target_os = "windows")]
                service,
            } => {
                #[cfg(target_os = "windows")]
                if service {
                    use windows_service::service_dispatcher;

                    service_dispatcher::start(installer::WINDOWS_SERVICE_NAME, ffi_service_agent)
                        .unwrap();
                } else {
                    run_async(run_agent(CancellationToken::new())).unwrap()
                }
                #[cfg(not(target_os = "windows"))]
                run_async(run_agent(CancellationToken::new())).unwrap()
            }
            AgentAction::Install => run_async(installer::install_agent()).unwrap(),
            AgentAction::Uninstall => run_async(installer::uninstall_agent()).unwrap(),
            AgentAction::Init { address } => run_async(init_agent(address)).unwrap(),
        },
        Command::Version => {
            println!("Commit: {}", svalin::commit())
        }
    }
}

fn run_async(fut: impl Future<Output = anyhow::Result<()>>) -> anyhow::Result<()> {
    let rt = runtime::Builder::new_multi_thread().enable_all().build()?;
    rt.block_on(fut)?;

    Ok(())
}

async fn start_server(address: String) -> anyhow::Result<()> {
    tracing::trace!("User wants to run server");

    let address = address.parse()?;
    tracing::trace!("Server address parsed successfully");
    let mutex = Arc::new(Mutex::<Option<Server>>::new(None));
    let mutex2 = mutex.clone();

    let cancel = CancellationToken::new();
    let cancel2 = cancel.clone();
    let cancel3 = cancel.clone();

    tokio::spawn(async move {
        tracing::trace!("Starting server");
        // This needs to be in a seperate task since the init server will block on
        // start_server
        let server = Server::build()
            .addr(address)
            .cancel(cancel2)
            .start_server()
            .await
            .unwrap();

        *mutex2.lock().unwrap() = Some(server);
    });

    tokio::spawn(async move {
        // Wait for shutdown signal
        tokio::signal::ctrl_c().await.unwrap();

        cancel3.cancel();
    });

    cancel.cancelled().await;
    println!("Shutting down server...");

    let server = mutex.lock().unwrap().take();

    if let Some(server) = server {
        server.close(Duration::from_secs(5)).await?;
    } else {
        panic!("server Mutex was empty")
    }

    Ok(())
}

async fn init_agent(address: String) -> anyhow::Result<()> {
    let mut welcome_message = "-".repeat(40);
    welcome_message.push_str("Svalin Agent");
    welcome_message.push_str("-".repeat(40).as_str());
    println!("{welcome_message}");

    println!("connecting to {address}...");

    let waiting_for_init = agent::init(address).await?;

    let cancel = CancellationToken::new();

    println!("Successfully requested to join server.");
    println!("Join-Code: {}", waiting_for_init.join_code());
    let waiting_for_confirm = waiting_for_init.wait_for_init().await?;
    println!("Confirm-Code: {}", waiting_for_confirm.confirm_code());
    waiting_for_confirm.wait_for_confirm(cancel).await?;
    println!("initialisation complete!");
    Ok(())
}

async fn run_agent(cancel: CancellationToken) -> anyhow::Result<()> {
    let cancel2 = cancel.clone();
    tokio::spawn(async move {
        // Wait for shutdown signal
        tokio::signal::ctrl_c().await.unwrap();

        cancel2.cancel();
    });

    agent::run(cancel).await?;
    Ok(())
}
