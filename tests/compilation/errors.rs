#[dyn_utils::dyn_trait(.)]
trait InvalidRename {
    fn method(&self);
}

#[dyn_utils::dyn_trait(path::Rename)]
trait RenamePath {
    fn method(&self);
}

#[dyn_utils::dyn_trait(remote(trait))]
trait RemoteWithoutEqual {
    fn method(&self);
}

#[dyn_utils::dyn_trait(remote = ?)]
trait InvalidRemote {
    fn method(&self);
}

#[dyn_utils::dyn_trait]
trait StorageOnNormalMethod {
    #[dyn_trait(storage = dyn_utils::DefaultStorage)]
    fn method(&self);
}

#[dyn_utils::dyn_trait]
trait StorageWithoutEqual {
    #[dyn_trait(storage(dyn_utils::DefaultStorage))]
    fn method(&self);
}

#[dyn_utils::dyn_trait]
trait InvalidStorage {
    #[dyn_trait(storage = ?)]
    fn method(&self);
}

#[dyn_utils::dyn_trait]
trait TrySyncOnNormalMethod {
    #[dyn_trait(try_sync)]
    fn method(&self);
}

#[dyn_utils::dyn_trait]
trait TrySyncOnNonAsyncMethod {
    #[dyn_trait(try_sync)]
    fn method(&self) -> impl Iterator<Item = ()>;
}

#[dyn_utils::dyn_trait]
trait TrySyncOnDummyFuture {
    #[dyn_trait(try_sync)]
    fn method(&self) -> impl Future;
}

#[dyn_utils::dyn_trait]
trait TrySyncOnDummyFuture2 {
    #[dyn_trait(try_sync)]
    fn method(&self) -> impl Future<Item = ()>;
}

#[dyn_utils::dyn_trait]
trait UnknownAttribute2 {
    #[dyn_trait(unknown)]
    fn method(&self);
}

trait SyncOnSyncMethod {
    fn method(&self);
}

impl SyncOnSyncMethod for () {
    #[dyn_utils::sync]
    fn method(&self) {}
}

macro_rules! nothing {
    () => {};
}

// TODO Only for coverage, and I don't know why
#[dyn_utils::dyn_trait(Dyn)]
trait ForCoverage {
    type Result;
    fn method(&self) -> Self::Result;
    async fn empty(&self);
    #[dyn_trait(try_sync)]
    async fn future(&self);
    nothing!();
}

impl ForCoverage for () {
    type Result = ();
    fn method(&self) -> Self::Result {}
    async fn empty(&self) {}
    #[dyn_utils::sync]
    async fn future(&self) {}
}

trait Remote {
    fn method(&self);
}

#[dyn_utils::dyn_trait(remote = crate::Remote)]
trait Remote {
    fn method(&self);
}

impl Remote for () {
    fn method(&self) {}
}

fn main() {}
