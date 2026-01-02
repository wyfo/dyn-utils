#[dyn_utils::dyn_compatible]
trait StorageOnNormalMethod {
    #[dyn_utils(storage = dyn_utils::DefaultStorage)]
    fn method(&self);
}

#[dyn_utils::dyn_compatible]
trait StorageWithoutEqual {
    #[dyn_utils(storage(dyn_utils::DefaultStorage))]
    fn method(&self);
}

#[dyn_utils::dyn_compatible]
trait InvalidStorage {
    #[dyn_utils(storage = ..)]
    fn method(&self);
}

#[dyn_utils::dyn_compatible]
trait TrySyncOnNormalMethod {
    #[dyn_utils(try_sync)]
    fn method(&self);
}

#[dyn_utils::dyn_compatible]
trait TrySyncOnNonAsyncMethod {
    #[dyn_utils(try_sync)]
    fn method(&self) -> impl Iterator<Item = ()>;
}

#[dyn_utils::dyn_compatible]
trait TrySyncOnDummyFuture {
    #[dyn_utils(try_sync)]
    fn method(&self) -> impl Future;
}

#[dyn_utils::dyn_compatible]
trait TrySyncOnDummyFuture2 {
    #[dyn_utils(try_sync)]
    fn method(&self) -> impl Future<Item = ()>;
}

#[dyn_utils::dyn_compatible]
trait UnknownAttribute {
    #[dyn_utils(unknown)]
    fn method(&self);
}

trait Trait {
    fn method(&self);
}

impl Trait for () {
    #[dyn_utils::sync]
    fn method(&self) {}
}

macro_rules! nothing {
    () => {};
}

// // TODO Only for coverage, and I don't know why
#[dyn_utils::dyn_compatible]
trait ForCoverage {
    type Result;
    fn method(&self) -> Self::Result;
    async fn empty(&self);
    #[dyn_utils(try_sync)]
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

fn main() {}
