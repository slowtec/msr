use super::*;
use std::{
    collections::HashMap, io::{Error, ErrorKind, Result}, time::Duration,
};

/// A simple synchronous closed-loop runtime.
#[derive(Debug)]
pub struct SyncRuntime {
    pub loops: Vec<Loop>,
    pub rules: Vec<Rule>,
    pub controllers: HashMap<String, ControllerType>,
}

impl Default for SyncRuntime {
    fn default() -> Self {
        SyncRuntime {
            loops: vec![],
            rules: vec![],
            controllers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncRuntimeState {
    pub io: IoState,
    pub controllers: HashMap<String, ControllerState>,
    pub rules: HashMap<String, bool>,
}

impl Default for SyncRuntimeState {
    fn default() -> Self {
        SyncRuntimeState {
            io: IoState::default(),
            controllers: HashMap::new(),
            rules: HashMap::new(),
        }
    }
}

impl SyncRuntime {
    /// Trigger the calculation for the next step.
    pub fn tick(&mut self, io: &mut SyncIoSystem, delta_t: &Duration) -> Result<SyncRuntimeState> {
        let mut state = SyncRuntimeState::default();
        for l in self.loops.iter_mut() {
            if l.inputs.len() != 1 || l.outputs.len() != 1 {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Loop has invalid length of inputs/outputs",
                ));
            }

            if state.io.inputs.get(&l.inputs[0]).is_none() {
                let input = io.read(&l.inputs[0])?;
                state.io.inputs.insert(l.inputs[0].clone(), input.clone());
            }

            if let Some(Value::Decimal(v)) = state.io.inputs.get(&l.inputs[0]) {
                //NOTE: this will be more elegant as soon the compiler feature "nll" is stable
                if self.controllers.get(&l.id).is_none() {
                    match l.controller {
                        ControllerConfig::Pid(ref cfg) => {
                            self.controllers.insert(
                                l.id.clone(),
                                ControllerType::Pid(pid::Pid::new(cfg.clone())),
                            );
                        }
                        ControllerConfig::BangBang(ref cfg) => {
                            self.controllers.insert(
                                l.id.clone(),
                                ControllerType::BangBang(bang_bang::BangBang::new(cfg.clone())),
                            );
                        }
                    }
                }
                if let Some(ref mut c) = self.controllers.get_mut(&l.id) {
                    let out = match c {
                        ControllerType::Pid(ref mut pid) => {
                            let next = (pid as &mut TimeStepController<f64, f64>)
                                .next(*v, &delta_t)
                                .into();
                            state
                                .controllers
                                .insert(l.id.clone(), ControllerState::Pid(pid.state.clone()));
                            next
                        }
                        ControllerType::BangBang(ref mut bb) => {
                            let next = (bb as &mut Controller<f64, bool>).next(*v).into();
                            state
                                .controllers
                                .insert(l.id.clone(), ControllerState::BangBang(bb.state.clone()));
                            next
                        }
                    };
                    if state.io.outputs.get(&l.outputs[0]).is_some() {
                        // warn!("You should not write multiple times to an output");
                    }
                    state.io.outputs.insert(l.outputs[0].clone(), out);
                } else {
                    //NOTE: this will be removed as soon the compiler feature "nll" is stable
                    panic!("The controller of loop '{}' was not initialized", l.id);
                }
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Invalid input data type: a decimal value is required",
                ));
            }
        }
        for (id, v) in state.io.outputs.iter() {
            io.write(&id, &v)?;
        }
        let (inputs, outputs) = self.get_rule_sources();
        for x in inputs {
            if state.io.inputs.get(&x).is_none() {
                let i = io.read(&x)?;
                state.io.inputs.insert(x.clone(), i);
            }
        }
        for x in outputs {
            if state.io.outputs.get(&x).is_none() {
                if let Some(o) = io.read_output(&x)? {
                    state.io.outputs.insert(x.clone(), o);
                }
            }
        }
        state.rules = self.rules_state(&mut state.io)?;
        Ok(state)
    }

    fn get_rule_sources(&self) -> (Vec<String>, Vec<String>) {
        let (inputs, outputs): (Vec<_>, Vec<_>) = self
            .rules
            .iter()
            .flat_map(|r| r.condition.sources())
            .filter_map(|s| match s {
                Source::In(id) => Some((Some(id), None)),
                Source::Out(id) => Some((None, Some(id))),
                Source::Const(_) => None,
            })
            .unzip();
        let mut inputs: Vec<_> = inputs.into_iter().filter_map(|x| x).collect();
        let mut outputs: Vec<_> = outputs.into_iter().filter_map(|x| x).collect();
        inputs.sort();
        inputs.dedup();
        outputs.sort();
        outputs.dedup();
        (inputs, outputs)
    }
    /// Check for active [Rule]s.
    fn rules_state(&self, io: &mut SyncIoSystem) -> Result<HashMap<String, bool>> {
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

        assert!(rt.tick(&mut io, &dt).is_ok());
        rt.loops = vec![loop0];
        assert!(rt.tick(&mut io, &dt).is_err());
        rt.loops[0].inputs = vec!["input".into()];
        assert!(rt.tick(&mut io, &dt).is_err());
        rt.loops[0].outputs = vec!["output".into()];
        assert!(rt.tick(&mut io, &dt).is_ok());
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
        let mut io = IoState::default();
        io.inputs.insert("input".into(), true.into());
        assert!(rt.tick(&mut io, &dt).is_err());
        io.inputs.insert("input".into(), Value::Bin(vec![]));
        assert!(rt.tick(&mut io, &dt).is_err());
        io.inputs.insert("input".into(), 0.0.into());
        assert!(rt.tick(&mut io, &dt).is_ok());
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
        let mut io = IoState::default();
        io.inputs.insert("sensor".into(), 0.0.into());
        rt.tick(&mut io, &dt).unwrap();
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
        rt.tick(&mut io, &dt).unwrap();
        assert_eq!(*io.outputs.get(&actuator).unwrap(), Value::Bit(false));
        io.inputs.insert(sensor, 3.0.into());
        rt.tick(&mut io, &dt).unwrap();
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

        assert_eq!(rt.tick(&mut io, &dt).unwrap(), SyncRuntimeState::default());

        rt.rules = vec![Rule {
            id: "foo".into(),
            desc: None,
            condition: BooleanExpr::Eval(Source::In("x".into()).cmp_ge(Source::Out("y".into()))),
            actions: vec!["a".into()],
        }];
        let state = rt.tick(&mut io, &dt).unwrap();
        assert_eq!(state.rules.len(), 1);
        assert_eq!(*state.rules.get("foo").unwrap(), false);
        assert_eq!(state.io.inputs.len(), 1);
        assert_eq!(state.io.inputs.get("x").unwrap(), &Value::from(1.0));
        assert_eq!(state.io.outputs.len(), 1);
        assert_eq!(state.io.outputs.get("y").unwrap(), &Value::from(2.0));

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
        let state = rt.tick(&mut io, &dt).unwrap();
        assert_eq!(state.io.inputs.len(), 3);
        assert_eq!(state.io.outputs.len(), 3);
        assert_eq!(state.io.outputs.get("b").unwrap(), &Value::from(true));
        assert_eq!(state.io.outputs.get("k").unwrap(), &Value::from(20.0));
        assert_eq!(state.controllers.len(), 2);
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

    #[test]
    fn only_read_or_write_if_required() {
        struct DummyIo {
            io: IoState,
            reads: HashMap<String, usize>,
            writes: HashMap<String, usize>,
        }

        impl SyncIoSystem for DummyIo {
            fn read(&mut self, id: &str) -> Result<Value> {
                let cnt = *self.reads.get(id).unwrap_or(&0) + 1;
                self.reads.insert(id.into(), cnt);
                self.io.read(id)
            }
            fn read_output(&mut self, id: &str) -> Result<Option<Value>> {
                let cnt = *self.reads.get(id).unwrap_or(&0) + 1;
                self.reads.insert(id.into(), cnt);
                self.io.read_output(id)
            }
            fn write(&mut self, id: &str, value: &Value) -> Result<()> {
                let cnt = *self.writes.get(id).unwrap_or(&0) + 1;
                self.writes.insert(id.into(), cnt);
                self.io.write(id, value)
            }
        }
        let mut io = DummyIo {
            io: IoState::default(),
            reads: HashMap::new(),
            writes: HashMap::new(),
        };
        let mut rt = SyncRuntime::default();
        let dt = Duration::from_secs(1);
        io.io.inputs.insert("x".into(), 1.0.into());
        io.io.outputs.insert("y".into(), 2.0.into());

        rt.tick(&mut io, &dt).unwrap();
        assert_eq!(io.reads.len(), 0);
        assert_eq!(io.writes.len(), 0);
        let rule = Rule {
            id: "foo".into(),
            desc: None,
            condition: BooleanExpr::Eval(Source::In("x".into()).cmp_ge(Source::Out("y".into()))),
            actions: vec!["a".into()],
        };

        rt.rules = vec![rule.clone(), rule.clone()];
        rt.tick(&mut io, &dt).unwrap();
        assert_eq!(io.reads.len(), 2);
        assert_eq!(io.writes.len(), 0);
        assert_eq!(*io.reads.get("x").unwrap(), 1);
        assert_eq!(*io.reads.get("y").unwrap(), 1);

        let mut pid_cfg = PidConfig::default();
        pid_cfg.k_p = 2.0;
        pid_cfg.default_target = 10.0;
        let pid = ControllerConfig::Pid(pid_cfg);
        let l = Loop {
            id: "pid".into(),
            desc: None,
            inputs: vec!["x".into()],
            outputs: vec!["y".into()],
            controller: pid,
        };
        rt.loops = vec![l.clone(), l.clone()];
        rt.tick(&mut io, &dt).unwrap();
        assert_eq!(io.reads.len(), 2);
        assert_eq!(io.writes.len(), 1);
        assert_eq!(*io.reads.get("x").unwrap(), 2);
        assert_eq!(*io.writes.get("y").unwrap(), 1);
    }
}
