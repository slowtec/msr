use super::*;
use std::{
    io::{Error, ErrorKind, Result}, time::Duration,
};

/// A simple synchronous closed-loop runtime.
#[derive(Debug)]
pub struct SyncRuntime {
    pub loops: Vec<Loop>,
}

impl Default for SyncRuntime {
    fn default() -> Self {
        SyncRuntime { loops: vec![] }
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
                match l.controller {
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
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Invalid input data type: a decimal value is required",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::{super::*, bang_bang::*, pid::*, *};

    #[test]
    fn check_loops_inputs_and_outputs_len() {
        let controller = ControllerType::BangBang(BangBang::new(BangBangConfig::default()));
        let dt = Duration::from_millis(5);
        let loop0 = Loop {
            inputs: vec![],
            outputs: vec![],
            controller,
        };
        let mut rt = SyncRuntime { loops: vec![] };
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
        let controller = ControllerType::Pid(Pid::new(PidConfig::default()));
        let dt = Duration::from_millis(5);
        let loops = vec![Loop {
            inputs: vec!["input".into()],
            outputs: vec!["output".into()],
            controller,
        }];
        let mut rt = SyncRuntime { loops };
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
        let mut pid = Pid::new(pid_cfg);
        pid.set_target(10.0);
        let controller = ControllerType::Pid(pid);
        let dt = Duration::from_secs(1);
        let loops = vec![Loop {
            inputs: vec!["sensor".into()],
            outputs: vec!["actuator".into()],
            controller,
        }];
        let mut rt = SyncRuntime { loops };
        let mut io = IoState::default();
        io.inputs.insert("sensor".into(), 0.0.into());
        rt.tick(&mut io, &dt).unwrap();
        assert_eq!(*io.outputs.get("actuator").unwrap(), Value::Decimal(20.0));
    }

    #[test]
    fn run_bang_bang_controllers() {
        let mut bb_cfg = BangBangConfig::default();
        bb_cfg.threshold = 2.0;
        let bb = BangBang::new(bb_cfg);
        let controller = ControllerType::BangBang(bb);
        let dt = Duration::from_secs(1);
        let sensor = "sensor".to_string();
        let actuator = "actuator".to_string();
        let loops = vec![Loop {
            inputs: vec![sensor.clone()],
            outputs: vec![actuator.clone()],
            controller,
        }];
        let mut rt = SyncRuntime { loops };
        let mut io = IoState::default();
        io.inputs.insert(sensor.clone(), 0.0.into());
        rt.tick(&mut io, &dt).unwrap();
        assert_eq!(*io.outputs.get(&actuator).unwrap(), Value::Bit(false));
        io.inputs.insert(sensor, 3.0.into());
        rt.tick(&mut io, &dt).unwrap();
        assert_eq!(*io.outputs.get(&actuator).unwrap(), Value::Bit(true));
    }
}
