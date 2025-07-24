use std::{future::Future, sync::Arc};

use anyhow::Result;
use quinn::rustls::server::danger::ClientCertVerifier;
use svalin_pki::Credential;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::{permissions::PermissionHandler, rpc::command::handler::HandlerCollection};

use super::{RpcServer, RpcServerConfig, Socket};

pub struct RpcServerConfigBuilder<A, B, C, D, E> {
    credentials: A,
    client_cert_verifier: B,
    command_builder: C,
    cancellation_token: D,
    task_tracker: E,
}

impl RpcServerConfigBuilder<(), (), (), (), ()> {
    pub fn new() -> RpcServerConfigBuilder<(), (), (), (), ()> {
        RpcServerConfigBuilder {
            credentials: (),
            client_cert_verifier: (),
            command_builder: (),
            cancellation_token: (),
            task_tracker: (),
        }
    }
}

impl<A, B, C, D, E> RpcServerConfigBuilder<A, B, C, D, E> {
    pub fn credentials(
        self,
        credentials: Credential,
    ) -> RpcServerConfigBuilder<Credential, B, C, D, E> {
        RpcServerConfigBuilder {
            credentials,
            client_cert_verifier: self.client_cert_verifier,
            command_builder: self.command_builder,
            cancellation_token: self.cancellation_token,
            task_tracker: self.task_tracker,
        }
    }

    pub fn client_cert_verifier(
        self,
        client_cert_verifier: Arc<dyn ClientCertVerifier>,
    ) -> RpcServerConfigBuilder<A, Arc<dyn ClientCertVerifier>, C, D, E> {
        RpcServerConfigBuilder {
            credentials: self.credentials,
            client_cert_verifier,
            command_builder: self.command_builder,
            cancellation_token: self.cancellation_token,
            task_tracker: self.task_tracker,
        }
    }

    pub fn commands<CB: RpcCommandBuilder>(
        self,
        command_builder: CB,
    ) -> RpcServerConfigBuilder<A, B, CB, D, E> {
        RpcServerConfigBuilder {
            credentials: self.credentials,
            client_cert_verifier: self.client_cert_verifier,
            command_builder,
            cancellation_token: self.cancellation_token,
            task_tracker: self.task_tracker,
        }
    }

    pub fn cancellation_token(
        self,
        cancellation_token: CancellationToken,
    ) -> RpcServerConfigBuilder<A, B, C, CancellationToken, E> {
        RpcServerConfigBuilder {
            credentials: self.credentials,
            client_cert_verifier: self.client_cert_verifier,
            command_builder: self.command_builder,
            cancellation_token,
            task_tracker: self.task_tracker,
        }
    }

    pub fn task_tracker(
        self,
        task_tracker: TaskTracker,
    ) -> RpcServerConfigBuilder<A, B, C, D, TaskTracker> {
        RpcServerConfigBuilder {
            credentials: self.credentials,
            client_cert_verifier: self.client_cert_verifier,
            command_builder: self.command_builder,
            cancellation_token: self.cancellation_token,
            task_tracker,
        }
    }
}

impl<CB>
    RpcServerConfigBuilder<
        Credential,
        Arc<dyn ClientCertVerifier>,
        CB,
        CancellationToken,
        TaskTracker,
    >
where
    CB: RpcCommandBuilder,
{
    pub async fn start_server(self, socket: Socket) -> Result<Arc<RpcServer>> {
        let config = RpcServerConfig {
            credentials: self.credentials,
            client_cert_verifier: self.client_cert_verifier,
            cancellation_token: self.cancellation_token,
        };

        RpcServer::run(socket, config, self.command_builder, self.task_tracker).await
    }
}

pub trait RpcCommandBuilder {
    type PH: PermissionHandler;

    fn build(
        self,
        server: &Arc<RpcServer>,
    ) -> impl Future<Output = Result<HandlerCollection<Self::PH>>>;
}

impl<PH: PermissionHandler> RpcCommandBuilder for HandlerCollection<PH> {
    type PH = PH;

    async fn build(self, _: &Arc<RpcServer>) -> Result<HandlerCollection<PH>> {
        Ok(self)
    }
}
