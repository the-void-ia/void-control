#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Pending,
    Starting,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

impl RunState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        match (self, next) {
            (Self::Pending, Self::Starting) => true,
            (Self::Starting, Self::Running) => true,
            (Self::Running, Self::Succeeded | Self::Failed | Self::Canceled) => true,
            (a, b) if a == b => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RunState;

    #[test]
    fn permits_expected_lifecycle() {
        assert!(RunState::Pending.can_transition_to(RunState::Starting));
        assert!(RunState::Starting.can_transition_to(RunState::Running));
        assert!(RunState::Running.can_transition_to(RunState::Succeeded));
        assert!(RunState::Running.can_transition_to(RunState::Failed));
        assert!(RunState::Running.can_transition_to(RunState::Canceled));
    }

    #[test]
    fn rejects_invalid_lifecycle_edges() {
        assert!(!RunState::Pending.can_transition_to(RunState::Running));
        assert!(!RunState::Starting.can_transition_to(RunState::Succeeded));
        assert!(!RunState::Succeeded.can_transition_to(RunState::Running));
    }

    #[test]
    fn marks_terminal_states() {
        assert!(RunState::Succeeded.is_terminal());
        assert!(RunState::Failed.is_terminal());
        assert!(RunState::Canceled.is_terminal());
        assert!(!RunState::Running.is_terminal());
    }
}
