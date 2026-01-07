#[dyn_utils::dyn_object(crate = crate, remote = Future)]
trait Future {
    type Output;
    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output>;
}

#[dyn_utils::dyn_object(crate = crate, remote = Future, bounds = Send)]
trait Future {
    type Output;
    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output>;
}

#[dyn_utils::dyn_object(crate = crate, remote = Iterator)]
trait Iterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
    fn size_hint(&self) -> (usize, Option<usize>);
    fn nth(&mut self, n: usize) -> Option<Self::Item>;
}
