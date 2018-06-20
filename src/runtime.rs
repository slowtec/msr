use super::*;
use std::{collections::HashMap, io::Result, time::Duration};

/// A simple synchronous closed-loop runtime.
#[derive(Debug)]
pub struct SyncRuntime {
    pub loops: Vec<Loop>,
    pub rules: Vec<Rule>,
    state: SyncRuntimeState,
}

impl Default for SyncRuntime {
    fn default() -> Self {
        SyncRuntime {
            loops: vec![],
            rules: vec![],
            state: SyncRuntimeState::default(),
        }
    }
}

/// The state structure of [SyncRuntime].
#[derive(Debug, Clone, PartialEq)]
pub struct SyncRuntimeState {
    pub controllers: HashMap<String, ControllerState>,
    pub rules: HashMap<String, bool>,
}

impl Default for SyncRuntimeState {
    fn default() -> Self {
        SyncRuntimeState {
            controllers: HashMap::new(),
            rules: HashMap::new(),
        }
    }
}

impl<'a>
    PureController<
        (&'a SyncRuntimeState, &'a IoState, &'a Duration),
        Result<(SyncRuntimeState, IoState)>,
    > for SyncRuntime
{
    fn next(
        &self,
        input: (&SyncRuntimeState, &IoState, &Duration),
    ) -> Result<(SyncRuntimeState, IoState)> {
        let (state, io, delta_t) = input;

        let mut io = io.clone();
        let mut state = state.clone();

        for l in self.loops.iter() {
            if state.controllers.get(&l.id).is_none() {
                match l.controller {
                    ControllerConfig::Pid(ref cfg) => {
                        let mut s = pid::PidState::default();
                        s.target = cfg.default_target;
                        state
                            .controllers
                            .insert(l.id.clone(), ControllerState::Pid(s));
                    }
                    ControllerConfig::BangBang(_) => {
                        state.controllers.insert(
                            l.id.clone(),
                            ControllerState::BangBang(bang_bang::BangBangState::default()),
                        );
                    }
                }
            }
            let (new_controller, new_io) = l.next((
                state
                    .controllers
                    .get(&l.id)
                    .expect("The controller state was not initialized"),
                &io,
                delta_t,
            ))?;
            io = new_io;
            state.controllers.insert(l.id.clone(), new_controller);
        }
        state.rules = self.rules_state(&io)?;
        Ok((state, io))
    }
}

impl SyncRuntime {
    /// Check for active [Rule]s.
    fn rules_state(&self, io: &IoState) -> Result<HashMap<String, bool>> {
        let mut rules_state = HashMap::new();
        for r in self.rules.iter() {
            let state = r.condition.eval(io)?;
            rules_state.insert(r.id.clone(), state);
        }
        Ok(rules_state)
    }
}

#[cfg(test)]
mod tests {

    use super::{super::*, bang_bang::*, pid::*, *};

    #[test]
    fn check_loops_inputs_and_outputs_len() {
        let controller = ControllerConfig::BangBang(BangBangConfig::default());
        let dt = Duration::from_millis(5);
        let loop0 = Loop {
            id: "foo".into(),
            desc: None,
            inputs: vec![],
            outputs: vec![],
            controller,
        };
        let mut rt = SyncRuntime::default();
        let mut io = IoState::default();
        io.inputs.insert("input".into(), 0.0.into());
        let s = SyncRuntimeState::default();
        assert!(rt.next((&s, &io, &dt)).is_ok());
        rt.loops = vec![loop0];
        assert!(rt.next((&s, &io, &dt)).is_err());
        rt.loops[0].inputs = vec!["input".into()];
        assert!(rt.next((&s, &io, &dt)).is_err());
        rt.loops[0].outputs = vec!["output".into()];
        assert!(rt.next((&s, &io, &dt)).is_ok());
    }

    #[test]
    fn check_input_value_type() {
        let controller = ControllerConfig::Pid(PidConfig::default());
        let dt = Duration::from_millis(5);
        let loops = vec![Loop {
            id: "foo".into(),
            desc: None,
            inputs: vec!["input".into()],
            outputs: vec!["output".into()],
            controller,
        }];
        let mut rt = SyncRuntime::default();
        rt.loops = loops;
        let s = SyncRuntimeState::default();
        let mut io = IoState::default();
        io.inputs.insert("input".into(), true.into());
        assert!(rt.next((&s, &io, &dt)).is_err());
        io.inputs.insert("input".into(), Value::Bin(vec![]));
        assert!(rt.next((&s, &io, &dt)).is_err());
        io.inputs.insert("input".into(), 0.0.into());
        assert!(rt.next((&s, &io, &dt)).is_ok());
    }

    #[test]
    fn run_pid_controllers() {
        let mut pid_cfg = PidConfig::default();
        pid_cfg.k_p = 2.0;
        pid_cfg.default_target = 10.0;
        let controller = ControllerConfig::Pid(pid_cfg);
        let dt = Duration::from_secs(1);
        let loops = vec![Loop {
            id: "foo".into(),
            desc: None,
            inputs: vec!["sensor".into()],
            outputs: vec!["actuator".into()],
            controller,
        }];
        let mut rt = SyncRuntime::default();
        rt.loops = loops;
        let s = SyncRuntimeState::default();
        let mut io = IoState::default();
        io.inputs.insert("sensor".into(), 0.0.into());
        let (_, io) = rt.next((&s, &io, &dt)).unwrap();
        assert_eq!(*io.outputs.get("actuator").unwrap(), Value::Decimal(20.0));
    }

    #[test]
    fn run_bang_bang_controllers() {
        let mut bb_cfg = BangBangConfig::default();
        bb_cfg.threshold = 2.0;
        let controller = ControllerConfig::BangBang(bb_cfg);
        let dt = Duration::from_secs(1);
        let sensor = "sensor".to_string();
        let actuator = "actuator".to_string();
        let loops = vec![Loop {
            id: "foo".into(),
            desc: None,
            inputs: vec![sensor.clone()],
            outputs: vec![actuator.clone()],
            controller,
        }];
        let mut rt = SyncRuntime::default();
        rt.loops = loops;
        let mut io = IoState::default();
        io.inputs.insert(sensor.clone(), 0.0.into());
        let s = SyncRuntimeState::default();
        let (_, mut io) = rt.next((&s, &io, &dt)).unwrap();
        assert_eq!(*io.outputs.get(&actuator).unwrap(), Value::Bit(false));
        io.inputs.insert(sensor, 3.0.into());
        let (_, io) = rt.next((&s, &io, &dt)).unwrap();
        assert_eq!(*io.outputs.get(&actuator).unwrap(), Value::Bit(true));
    }

    #[test]
    fn check_active_rules() {
        let mut io = IoState::default();
        let mut rt = SyncRuntime::default();
        assert_eq!(rt.rules_state(&mut io).unwrap().len(), 0);
        rt.rules = vec![Rule {
            id: "foo".into(),
            desc: None,
            condition: BooleanExpr::Eval(Source::In("x".into()).cmp_ge(Source::Out("y".into()))),
            actions: vec!["a".into()],
        }];
        assert!(rt.rules_state(&mut io).is_err());
        io.inputs.insert("x".into(), 33.3.into());
        io.outputs.insert("y".into(), 33.3.into());
        assert_eq!(*rt.rules_state(&mut io).unwrap().get("foo").unwrap(), true);
    }

    #[test]
    fn runtime_state() {
        let mut io = IoState::default();
        let mut rt = SyncRuntime::default();
        let dt = Duration::from_secs(1);
        io.inputs.insert("a".into(), 8.0.into());
        io.inputs.insert("b".into(), false.into());
        io.inputs.insert("j".into(), 0.0.into());
        io.inputs.insert("k".into(), 0.0.into());
        io.inputs.insert("x".into(), 1.0.into());
        io.inputs.insert("z".into(), 3.0.into());
        io.outputs.insert("y".into(), 2.0.into());

        let s = SyncRuntimeState::default();
        assert_eq!(
            rt.next((&s, &io, &dt)).unwrap().0,
            SyncRuntimeState::default()
        );

        rt.rules = vec![Rule {
            id: "foo".into(),
            desc: None,
            condition: BooleanExpr::Eval(Source::In("x".into()).cmp_ge(Source::Out("y".into()))),
            actions: vec!["a".into()],
        }];
        let (state, io) = rt.next((&s, &io, &dt)).unwrap();
        assert_eq!(state.rules.len(), 1);
        assert_eq!(*state.rules.get("foo").unwrap(), false);
        assert_eq!(io.inputs.get("x").unwrap(), &Value::from(1.0));
        assert_eq!(io.outputs.get("y").unwrap(), &Value::from(2.0));

        let mut bb_cfg = BangBangConfig::default();
        bb_cfg.threshold = 2.0;
        let bb = ControllerConfig::BangBang(bb_cfg);

        let mut pid_cfg = PidConfig::default();
        pid_cfg.k_p = 2.0;
        pid_cfg.default_target = 10.0;
        let pid = ControllerConfig::Pid(pid_cfg);

        let loops = vec![
            Loop {
                id: "bb".into(),
                desc: None,
                inputs: vec!["a".into()],
                outputs: vec!["b".into()],
                controller: bb,
            },
            Loop {
                id: "pid".into(),
                desc: None,
                inputs: vec!["j".into()],
                outputs: vec!["k".into()],
                controller: pid,
            },
        ];
        rt.loops = loops;
        let (state, io) = rt.next((&s, &io, &dt)).unwrap();
        assert_eq!(io.outputs.get("b").unwrap(), &Value::from(true));
        assert_eq!(io.outputs.get("k").unwrap(), &Value::from(20.0));
        assert_eq!(
            state.controllers.get("bb").unwrap(),
            &ControllerState::BangBang(true)
        );
        assert_eq!(
            state.controllers.get("pid").unwrap(),
            &ControllerState::Pid(PidState {
                p: 20.0,
                i: 0.0,
                d: 0.0,
                prev_value: Some(0.0),
                target: 10.0,
            })
        );
    }
}
