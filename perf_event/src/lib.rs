extern "C" {
    fn pe_foo();
}
pub mod sampling {
    use super::*;
    pub fn safe_foo() {
        unsafe {
            pe_foo();
        }
    }
}
