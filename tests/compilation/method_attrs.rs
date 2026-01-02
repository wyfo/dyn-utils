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
trait UnknownAttribute {
    #[dyn_utils(unknown)]
    fn method(&self);
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
    nothing!();
}

fn main() {}
