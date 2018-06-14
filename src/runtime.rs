use super::*;
use std::{
    collections::HashMap, io::{Error, ErrorKind, Result}, time::Duration,
};

/// A simple synchronous closed-loop runtime.
#[derive(Debug)]
pub struct SyncRuntime {
    pub loops: Vec<Loop>,
    pub rules: Vec<Rule>,
    controllers: HashMap<String, ControllerType>,
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

impl SyncRuntime {
    /// Trigger the calculation for the next step.
    pub fn tick(&mut self, io: &mut SyncIoSystem, delta_t: &Duration) -> Result<()> {
        for l in self.loops.iter_mut() {
            if l.inputs.len() != 1 || l.outputs.len() != 1 {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Loop has invalid length of inputs/outputs",
                ));
            }
            let input = io.read(&l.inputs[0])?;
            if let Value::Decimal(v) = input {
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
                    match c {
                        ControllerType::Pid(ref mut pid) => {
                            let out = (pid as &mut TimeStepController<f64, f64>).next(v, &delta_t);
                            io.write(&l.outputs[0], &Value::Decimal(out))?;
                        }
                        ControllerType::BangBang(ref mut bb) => {
                            let out = (bb as &mut Controller<f64, bool>).next(v);
                            io.write(&l.outputs[0], &Value::Bit(out))?;
                        }
                    }
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
        Ok(())
    }
    /// Check for active [Rule]s.
    pub fn active_rules(&mut self, io: &mut SyncIoSystem) -> Result<Vec<String>> {
        let mut active_rules = vec![];
        for r in self.rules.iter() {
            if r.condition.eval(io)? {
                active_rules.push(r.id.clone());
            }
        }
        Ok(active_rules)
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
        assert_eq!(rt.active_rules(&mut io).unwrap().len(), 0);
        rt.rules = vec![Rule {
            id: "foo".into(),
            desc: None,
            condition: BooleanExpr::Eval(Source::In("x".into()).cmp_ge(Source::Out("y".into()))),
            actions: vec!["a".into()],
        }];
        assert!(rt.active_rules(&mut io).is_err());
        io.inputs.insert("x".into(), 33.3.into());
        io.outputs.insert("y".into(), 33.3.into());
        assert_eq!(rt.active_rules(&mut io).unwrap()[0], "foo");
    }
}
