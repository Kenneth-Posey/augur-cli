use augur_domain::domain::StepStatus;

fn non_exhaustive(status: StepStatus) -> bool {
    match status {
        StepStatus::Pending => true,
    }
}

fn main() {
    let _ = non_exhaustive(StepStatus::Pending);
}
