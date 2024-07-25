use svalin_pki::Certificate;

#[derive(Debug, Clone)]
pub enum Peer {
    Anonymous,
    Certificate(Certificate),
}
