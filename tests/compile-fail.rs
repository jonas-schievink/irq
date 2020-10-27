#[test]
#[cfg(not(miri))]
fn ui() {
    if rustc_version::version_meta().unwrap().channel == rustc_version::Channel::Nightly {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile-fail/*.rs");
    } else {
        println!("not on nightly, skipping compile-fail tests");
    }
}
