// Regression case for #4521 / #4523: precise-capture syntax (`+ use<T>`)
// stabilised in Rust 1.82 could not be parsed by syn v1, which made
// `CrateContext::parse` fail and broke IDL generation. Keeping this in
// the generics program exercises the IDL-build code path in CI.

#[allow(dead_code)]
pub fn identity<T: Clone>(x: T) -> impl Clone + use<T> {
    x
}
