use std::{future::Future, sync::Arc};

use anyhow::Result;
use quinn::rustls::server::danger::ClientCertVerifier;
use svalin_pki::PermCredentials;
use tokio_util::sync::CancellationToken;

use crate::{permissions::PermissionHandler, rpc::command::handler::HandlerCollection};

use super::{RpcServer, RpcServerConfig, Socket};

pub struct RpcServerConfigBuilder<A, B, C, D> {
    credentials: A,
    client_cert_verifier: B,
    command_builder: C,
    cancellation_token: D,
}

impl RpcServerConfigBuilder<(), (), (), ()> {
    pub fn new() -> RpcServerConfigBuilder<(), (), (), ()> {
        RpcServerConfigBuilder {
            credentials: (),
            client_cert_verifier: (),
            command_builder: (),
            cancellation_token: (),
        }
    }
}

impl<A, B, C, D> RpcServerConfigBuilder<A, B, C, D> {
    pub fn credentials(
        self,
        credentials: PermCredentials,
    ) -> RpcServerConfigBuilder<PermCredentials, B, C, D> {
        RpcServerConfigBuilder {
            credentials,
            client_cert_verifier: self.client_cert_verifier,
            command_builder: self.command_builder,
            cancellation_token: self.cancellation_token,
        }
    }

    pub fn client_cert_verifier(
        self,
        client_cert_verifier: Arc<dyn ClientCertVerifier>,
    ) -> RpcServerConfigBuilder<A, Arc<dyn ClientCertVerifier>, C, D> {
        RpcServerConfigBuilder {
            credentials: self.credentials,
            client_cert_verifier,
            command_builder: self.command_builder,
            cancellation_token: self.cancellation_token,
        }
    }

    pub fn commands<CB: RpcCommandBuilder>(
        self,
        command_builder: CB,
    ) -> RpcServerConfigBuilder<A, B, CB, D> {
        RpcServerConfigBuilder {
            credentials: self.credentials,
            client_cert_verifier: self.client_cert_verifier,
            command_builder,
            cancellation_token: self.cancellation_token,
        }
    }

    pub fn cancellation_token(
        self,
        cancellation_token: CancellationToken,
    ) -> RpcServerConfigBuilder<A, B, C, CancellationToken> {
        RpcServerConfigBuilder {
            credentials: self.credentials,
            client_cert_verifier: self.client_cert_verifier,
            command_builder: self.command_builder,
            cancellation_token,
        }
    }
}

impl<CB> RpcServerConfigBuilder<PermCredentials, Arc<dyn ClientCertVerifier>, CB, CancellationToken>
where
    CB: RpcCommandBuilder,
{
    pub async fn start_server(self, socket: Socket) -> Result<Arc<RpcServer>> {
        let config = RpcServerConfig {
            credentials: self.credentials,
            client_cert_verifier: self.client_cert_verifier,
            cancellation_token: self.cancellation_token,
        };

        RpcServer::run(socket, config, self.command_builder).await
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
