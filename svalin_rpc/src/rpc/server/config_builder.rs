use std::sync::Arc;

use anyhow::Result;
use quinn::rustls::server::danger::ClientCertVerifier;
use svalin_pki::PermCredentials;
use tokio_util::sync::CancellationToken;

use crate::{permissions::PermissionHandler, rpc::command::handler::HandlerCollection};

use super::{RpcServer, RpcServerConfig, Socket};

pub struct RpcServerConfigBuilder<A, B, C, D> {
    credentials: A,
    client_cert_verifier: B,
    commands: C,
    cancellation_token: D,
}

impl RpcServerConfigBuilder<(), (), (), ()> {
    pub fn new() -> RpcServerConfigBuilder<(), (), (), ()> {
        RpcServerConfigBuilder {
            credentials: (),
            client_cert_verifier: (),
            commands: (),
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
            commands: self.commands,
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
            commands: self.commands,
            cancellation_token: self.cancellation_token,
        }
    }

    pub fn commands<PH: PermissionHandler>(
        self,
        commands: HandlerCollection<PH>,
    ) -> RpcServerConfigBuilder<A, B, HandlerCollection<PH>, D> {
        RpcServerConfigBuilder {
            credentials: self.credentials,
            client_cert_verifier: self.client_cert_verifier,
            commands,
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
            commands: self.commands,
            cancellation_token,
        }
    }
}

impl<PH: PermissionHandler>
    RpcServerConfigBuilder<
        PermCredentials,
        Arc<dyn ClientCertVerifier>,
        HandlerCollection<PH>,
        CancellationToken,
    >
{
    pub fn start_server(self, socket: Socket) -> Result<Arc<RpcServer<PH>>> {
        let config = self.to_config();

        RpcServer::run(socket, config)
    }

    fn to_config(self) -> RpcServerConfig<PH> {
        RpcServerConfig {
            credentials: self.credentials,
            client_cert_verifier: self.client_cert_verifier,
            cancellation_token: self.cancellation_token,
            commands: self.commands,
        }
    }
}
