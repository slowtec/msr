//! Finit State Machine
use super::*;

/// Finit State Machine
#[derive(Debug, Clone)]
pub struct StateMachine {
    /// Initial state
    pub initial: String,
    /// Transitions
    pub transitions: Vec<Transition>,
}

/// A State Transition
#[derive(Debug, Clone)]
pub struct Transition {
    pub condition: BooleanExpr<Comparison>,
    pub from: String,
    pub to: String,
    pub actions: Vec<String>,
}

impl<'a> PureController<(Option<&'a str>, &'a SystemState), Option<(String, Vec<String>)>>
    for StateMachine
{
    fn next(&self, input: (Option<&str>, &SystemState)) -> Option<(String, Vec<String>)> {
        let (fsm_state, state) = input;

        for t in &self.transitions {
            if t.from == fsm_state.unwrap_or_else(|| &self.initial) {
                if let Ok(active) = t.condition.eval(state) {
                    if active {
                        return Some((t.to.clone(), t.actions.clone()));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn simple_fsm() {
        let mut state = SystemState::default();
        let machine = StateMachine {
            initial: "start".into(),
            transitions: vec![
                Transition {
                    condition: BooleanExpr::Eval(
                        Source::In("x".into()).cmp_gt(Source::Const(5.0.into())),
                    ),
                    from: "start".into(),
                    to: "step-one".into(),
                    actions: vec![],
                },
                Transition {
                    condition: BooleanExpr::Eval(
                        Source::In("y".into()).cmp_gt(Source::Const(7.0.into())),
                    ),
                    from: "step-one".into(),
                    to: "step-two".into(),
                    actions: vec![],
                },
            ],
        };
        assert_eq!(machine.next((None, &state)), None);
        state.io.inputs.insert("x".into(), Value::Decimal(5.1));
        assert_eq!(
            machine.next((None, &state)),
            Some(("step-one".into(), vec![]))
        );
        assert_eq!(machine.next((Some("step-one"), &state)), None);
        state.io.inputs.insert("y".into(), Value::Decimal(7.1));
        assert_eq!(
            machine.next((Some("step-one"), &state)),
            Some(("step-two".into(), vec![]))
        );
    }
}
