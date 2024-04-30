use std::{fmt::Display, future::Future, sync::Arc};

use anyhow::Ok;
use rand::random;

struct Permission {}

#[derive(Debug)]
struct PermissionError {}

impl std::error::Error for PermissionError {}

impl Display for PermissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Permission denied")
    }
}

type PermissionCheckResult = Result<(), PermissionError>;

trait PermissionEntity {
    fn may(&self, p: Permission) -> impl Future<Output = PermissionCheckResult>;
}

struct Root {}

impl PermissionEntity for Root {
    async fn may(&self, _: Permission) -> PermissionCheckResult {
        Ok(())
    }
}

struct Anonymous {}

impl PermissionEntity for Anonymous {
    async fn may(&self, p: Permission) -> PermissionCheckResult {
        return Err(PermissionError {});
    }
}

// The entity resolver is given a uuid and resolves it to a PermissionEntity
trait EntityResolver {
    fn resolve(uuid: uuid::Uuid) -> anyhow::Result<Box<dyn PermissionEntity>>;
}

struct TestResolver {}

impl EntityResolver for TestResolver {
    async fn resolve(uuid: uuid::Uuid) -> anyhow::Result<Box<dyn PermissionEntity>> {
        if random::<bool>() {
            Ok(Box::new(Root {}))
        } else {
            Ok(Box::new(Anonymous {}))
        }
    }
}
