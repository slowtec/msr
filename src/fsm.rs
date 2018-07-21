//! Finit State Machine
use super::*;

/// Finit State Machine
#[derive(Debug, Clone)]
pub struct StateMachine {
    /// Transitions
    pub transitions: Vec<Transition>,
}

/// A State Transition
#[derive(Debug, Clone)]
pub struct Transition {
    pub condition: BooleanExpr<Comparison>,
    pub from: String,
    pub to: String,
}

impl<'a> PureController<(&'a str, &'a IoState), Option<String>> for StateMachine {
    fn next(&self, input: (&str, &IoState)) -> Option<String> {
        let (state, io) = input;

        for t in &self.transitions {
            if t.from == state {
                if let Ok(active) = t.condition.eval(io) {
                    if active {
                        return Some(t.to.clone());
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
        let mut io = IoState::default();
        let machine = StateMachine {
            transitions: vec![
                Transition {
                    condition: BooleanExpr::Eval(
                        Source::In("x".into()).cmp_gt(Source::Const(5.0.into())),
                    ),
                    from: "start".into(),
                    to: "step-one".into(),
                },
                Transition {
                    condition: BooleanExpr::Eval(
                        Source::In("y".into()).cmp_gt(Source::Const(7.0.into())),
                    ),
                    from: "step-one".into(),
                    to: "step-two".into(),
                },
            ],
        };
        assert_eq!(machine.next(("start", &io)), None);
        io.inputs.insert("x".into(), Value::Decimal(5.1));
        assert_eq!(machine.next(("start", &io)), Some("step-one".into()));
        assert_eq!(machine.next(("step-one", &io)), None);
        io.inputs.insert("y".into(), Value::Decimal(7.1));
        assert_eq!(machine.next(("step-one", &io)), Some("step-two".into()));
    }
}
