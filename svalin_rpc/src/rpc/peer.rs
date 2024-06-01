use std::sync::Arc;

use svalin_pki::Certificate;

pub enum Peer {
    Anonymous,
    Certificate(Arc<Certificate>),
}
