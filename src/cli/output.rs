/// Discord currently emits its command data inside the operation. This receipt carries only its explicit completion budget.
#[derive(Debug)]
pub struct CliOutput {
    pub elapsed_duration_warn_threshold: Option<std::time::Duration>,
}
impl CliOutput {
    #[must_use]
    pub const fn none(elapsed_duration_warn_threshold: Option<std::time::Duration>) -> Self {
        Self {
            elapsed_duration_warn_threshold,
        }
    }
}
