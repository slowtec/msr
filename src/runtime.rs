use super::*;
use fsm::*;
use std::{collections::HashMap, io::Result, time::Duration};

/// A simple synchronous closed-loop runtime.
#[derive(Debug)]
pub struct SyncRuntime {
    /// Loops grouped by interval IDs
    // TODO: separte intervals and loops
    pub loops: HashMap<String, Vec<Loop>>,
    /// Rules that will be evaluated on each step.
    pub rules: Vec<Rule>,
    /// Actions that can modify the state
    pub actions: Vec<Action>,
    /// Finite State Machines
    pub state_machines: HashMap<String, StateMachine>,
}

impl Default for SyncRuntime {
    fn default() -> Self {
        SyncRuntime {
            loops: HashMap::new(),
            rules: vec![],
            actions: vec![],
            state_machines: HashMap::new(),
        }
    }
}

//TODO: tidy up!
impl<'a> PureController<(&'a SystemState, &'a str, &'a Duration), Result<SystemState>>
    for SyncRuntime
{
    fn next(&self, input: (&SystemState, &str, &Duration)) -> Result<SystemState> {
        let (orig_state, interval, dt) = input;
        let mut state = orig_state.clone();

        if let Some(loops) = self.loops.get(interval) {
            for (id, s) in &orig_state.setpoints {
                if loops.iter().any(|l| l.id == *id) {
                    if let Some(c) = orig_state.controllers.get(id) {
                        if let Value::Decimal(v) = s {
                            match c {
                                ControllerState::Pid(pid) => {
                                    let mut pid = *pid;
                                    pid.target = *v;
                                    state
                                        .controllers
                                        .insert(id.clone(), ControllerState::Pid(pid));
                                }
                                ControllerState::BangBang(bb) => {
                                    let mut bb = *bb;
                                    bb.threshold = *v;
                                    state
                                        .controllers
                                        .insert(id.clone(), ControllerState::BangBang(bb));
                                }
                            }
                        }
                    }
                }
            }
            for l in loops.iter() {
                if state.controllers.get(&l.id).is_none() {
                    self.initialize_controller_state(l, &mut state);
                }
                let (new_controller, new_io) = l.next((
                    state
                        .controllers
                        .get(&l.id)
                        .expect("The controller state was not initialized"),
                    &state.io,
                    dt,
                ))?;
                state.io = new_io;
                state.controllers.insert(l.id.clone(), new_controller);
            }
        }

        state.rules = self.rules_state(&state)?;

        let rule_actions = state
            .rules
            .iter()
            .filter(|(_, active)| **active)
            .filter_map(|(r_id, _)| {
                if let Some(r) = self.rules.iter().find(|r| r.id == *r_id) {
                    Some(&r.actions)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for x in rule_actions {
            self.apply_actions(&x, orig_state, &mut state, interval);
        }

        let mut actions = vec![];

        for (m_id, machine) in &self.state_machines {
            if let Some((new_fsm_state, fsm_actions)) =
                machine.next((state.state_machines.get(m_id).map(|x| &**x), &state))
            {
                if !fsm_actions.is_empty() {
                    actions.push(fsm_actions);
                }
                state.state_machines.insert(m_id.clone(), new_fsm_state);
            }
        }

        for x in actions {
            self.apply_actions(&x, orig_state, &mut state, interval);
        }

        Ok(state)
    }
}

impl SyncRuntime {
    /// Check for active [Rule]s.
    fn rules_state(&self, state: &SystemState) -> Result<HashMap<String, bool>> {
        let mut rules_state = HashMap::new();
        for r in &self.rules {
            let r_state = r.condition.eval(state)?;
            rules_state.insert(r.id.clone(), r_state);
        }
        Ok(rules_state)
    }

    fn initialize_controller_state(&self, l: &Loop, state: &mut SystemState) {
        match l.controller {
            ControllerConfig::Pid(ref cfg) => {
                let mut s = pid::PidState::default();
                s.target = cfg.default_target;
                state
                    .controllers
                    .insert(l.id.clone(), ControllerState::Pid(s));
            }
            ControllerConfig::BangBang(ref cfg) => {
                let mut s = bang_bang::BangBangState::default();
                s.threshold = cfg.default_threshold;
                state
                    .controllers
                    .insert(l.id.clone(), ControllerState::BangBang(s));
            }
        }
    }

    fn apply_actions(
        &self,
        actions: &[String],
        orig_state: &SystemState,
        state: &mut SystemState,
        interval: &str,
    ) {
        use Source::*;

        for a_id in actions {
            if let Some(a) = self.actions.iter().find(|a| a.id == *a_id) {
                for (k, src) in &a.outputs {
                    match src {
                        In(id) => {
                            if let Some(v) = orig_state.io.inputs.get(id) {
                                state.io.outputs.insert(k.clone(), v.clone());
                            }
                        }
                        Out(id) => {
                            if let Some(v) = orig_state.io.outputs.get(id) {
                                state.io.outputs.insert(k.clone(), v.clone());
                            }
                        }
                        Mem(id) => {
                            if let Some(v) = orig_state.io.mem.get(id) {
                                state.io.outputs.insert(k.clone(), v.clone());
                            }
                        }
                        Const(v) => {
                            state.io.outputs.insert(k.clone(), v.clone());
                        }
                        Setpoint(id) => {
                            if let Some(v) = orig_state.setpoints.get(id) {
                                state.io.outputs.insert(k.clone(), v.clone());
                            }
                        }
                    }
                }
                for (k, src) in &a.setpoints {
                    match src {
                        In(id) => {
                            if let Some(v) = orig_state.io.inputs.get(id) {
                                state.setpoints.insert(k.clone(), v.clone());
                            }
                        }
                        Out(id) => {
                            if let Some(v) = orig_state.io.outputs.get(id) {
                                state.setpoints.insert(k.clone(), v.clone());
                            }
                        }
                        Mem(id) => {
                            if let Some(v) = orig_state.io.mem.get(id) {
                                state.setpoints.insert(k.clone(), v.clone());
                            }
                        }
                        Const(v) => {
                            state.setpoints.insert(k.clone(), v.clone());
                        }
                        Setpoint(id) => {
                            if let Some(v) = orig_state.setpoints.get(id) {
                                state.setpoints.insert(k.clone(), v.clone());
                            }
                        }
                    }
                }
                for id in &a.controller_resets {
                    if let Some(loops) = self.loops.get(interval) {
                        if let Some(l) = loops.iter().find(|l| l.id == *id) {
                            state.controllers.remove(id);
                            self.initialize_controller_state(l, state);
                        }
                    }
                }
            }
        }
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
            inputs: vec![],
            outputs: vec![],
            controller,
        };
        let mut rt = SyncRuntime::default();
        let mut s = SystemState::default();
        s.io.inputs.insert("input".into(), 0.0.into());
        assert!(rt.next((&s, "i", &dt)).is_ok());
        rt.loops.insert("i".into(), vec![loop0]);
        assert!(rt.next((&s, "i", &dt)).is_err());
        rt.loops.get_mut("i").unwrap()[0].inputs = vec!["input".into()];
        assert!(rt.next((&s, "i", &dt)).is_err());
        rt.loops.get_mut("i").unwrap()[0].outputs = vec!["output".into()];
        assert!(rt.next((&s, "i", &dt)).is_ok());
    }

    #[test]
    fn check_input_value_type() {
        let controller = ControllerConfig::Pid(PidConfig::default());
        let dt = Duration::from_millis(5);
        let loops = vec![Loop {
            id: "foo".into(),
            inputs: vec!["input".into()],
            outputs: vec!["output".into()],
            controller,
        }];
        let mut rt = SyncRuntime::default();
        rt.loops.insert("i".into(), loops);
        let mut s = SystemState::default();
        s.io.inputs.insert("input".into(), true.into());
        assert!(rt.next((&s, "i", &dt)).is_err());
        s.io.inputs.insert("input".into(), Value::Bin(vec![]));
        assert!(rt.next((&s, "i", &dt)).is_err());
        s.io.inputs.insert("input".into(), 0.0.into());
        assert!(rt.next((&s, "i", &dt)).is_ok());
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
            inputs: vec!["sensor".into()],
            outputs: vec!["actuator".into()],
            controller,
        }];
        let mut rt = SyncRuntime::default();
        rt.loops.insert("i".into(), loops);
        let mut s = SystemState::default();
        s.io.inputs.insert("sensor".into(), 0.0.into());
        let s = rt.next((&s, "i", &dt)).unwrap();
        assert_eq!(*s.io.outputs.get("actuator").unwrap(), Value::Decimal(20.0));
    }

    #[test]
    fn run_bang_bang_controllers() {
        let mut bb_cfg = BangBangConfig::default();
        bb_cfg.default_threshold = 2.0;
        let controller = ControllerConfig::BangBang(bb_cfg);
        let dt = Duration::from_secs(1);
        let sensor = "sensor".to_string();
        let actuator = "actuator".to_string();
        let loops = vec![Loop {
            id: "foo".into(),
            inputs: vec![sensor.clone()],
            outputs: vec![actuator.clone()],
            controller,
        }];
        let mut rt = SyncRuntime::default();
        rt.loops.insert("i".into(), loops);
        let mut s = SystemState::default();
        s.io.inputs.insert(sensor.clone(), 0.0.into());
        let mut s = rt.next((&s, "i", &dt)).unwrap();
        assert_eq!(*s.io.outputs.get(&actuator).unwrap(), Value::Bit(false));
        s.io.inputs.insert(sensor, 3.0.into());
        let s = rt.next((&s, "i", &dt)).unwrap();
        assert_eq!(*s.io.outputs.get(&actuator).unwrap(), Value::Bit(true));
    }

    #[test]
    fn check_active_rules() {
        let mut state = SystemState::default();
        let mut rt = SyncRuntime::default();
        assert_eq!(rt.rules_state(&mut state).unwrap().len(), 0);
        rt.rules = vec![Rule {
            id: "foo".into(),
            condition: BooleanExpr::Eval(Source::In("x".into()).cmp_ge(Source::Out("y".into()))),
            actions: vec!["a".into()],
        }];
        assert!(rt.rules_state(&mut state).is_err());
        state.io.inputs.insert("x".into(), 33.3.into());
        state.io.outputs.insert("y".into(), 33.3.into());
        assert_eq!(
            *rt.rules_state(&mut state).unwrap().get("foo").unwrap(),
            true
        );
    }

    #[test]
    fn apply_actions() {
        let mut rt = SyncRuntime::default();
        let mut state = SystemState::default();
        let dt = Duration::from_secs(1);
        rt.rules = vec![Rule {
            id: "foo".into(),
            condition: BooleanExpr::Eval(Source::In("x".into()).cmp_eq(Source::Const(10.0.into()))),
            actions: vec!["a".into()],
        }];
        let mut outputs = HashMap::new();
        let mut setpoints = HashMap::new();

        outputs.insert("z".into(), Source::Const(6.into()));
        outputs.insert("j".into(), Source::In("ref-in".into()));
        outputs.insert("k".into(), Source::Out("ref-out".into()));

        setpoints.insert("foo".into(), Source::Const(99.7.into()));
        setpoints.insert("bar".into(), Source::In("a".into()));
        setpoints.insert("baz".into(), Source::Out("b".into()));

        rt.actions = vec![Action {
            id: "a".into(),
            outputs,
            setpoints,
            controller_resets: vec![],
        }];
        state.io.inputs.insert("x".into(), 0.0.into());
        let mut state = rt.next((&state, "i", &dt)).unwrap();
        assert!(state.io.outputs.get("z").is_none());
        assert!(state.io.outputs.get("j").is_none());
        assert!(state.io.outputs.get("k").is_none());
        assert!(state.setpoints.get("foo").is_none());
        assert!(state.setpoints.get("bar").is_none());
        assert!(state.setpoints.get("baz").is_none());
        state.io.inputs.insert("x".into(), 10.0.into());
        state.io.inputs.insert("ref-in".into(), 33.0.into());
        state.io.inputs.insert("a".into(), true.into());
        state
            .io
            .outputs
            .insert("ref-out".into(), "bla".to_string().into());
        state.io.outputs.insert("b".into(), false.into());
        let state = rt.next((&state, "i", &dt)).unwrap();
        assert_eq!(*state.io.outputs.get("z").unwrap(), Value::Integer(6));
        assert_eq!(*state.io.outputs.get("j").unwrap(), Value::Decimal(33.0));
        assert_eq!(
            *state.io.outputs.get("k").unwrap(),
            Value::Text("bla".into())
        );
        assert_eq!(
            *state.setpoints.get("foo").unwrap(),
            Value::Decimal(99.7.into())
        );
        assert_eq!(*state.setpoints.get("bar").unwrap(), Value::Bit(true));
        assert_eq!(*state.setpoints.get("baz").unwrap(), Value::Bit(false));
    }

    #[test]
    fn apply_controller_reset_actions() {
        let mut rt = SyncRuntime::default();
        let mut state = SystemState::default();
        let dt = Duration::from_secs(1);
        let mut pid_cfg = PidConfig::default();
        pid_cfg.k_p = 2.0;
        pid_cfg.k_i = 100.0;
        pid_cfg.k_d = 1.0;
        pid_cfg.default_target = 10.0;
        let controller = ControllerConfig::Pid(pid_cfg);
        rt.loops.insert(
            "i".into(),
            vec![Loop {
                id: "pid".into(),
                inputs: vec!["sensor".into()],
                outputs: vec!["actuator".into()],
                controller,
            }],
        );
        rt.rules = vec![Rule {
            id: "foo".into(),
            condition: BooleanExpr::Eval(Source::In("x".into()).cmp_eq(Source::Const(10.0.into()))),
            actions: vec!["a".into()],
        }];
        rt.actions = vec![Action {
            id: "a".into(),
            outputs: HashMap::new(),
            setpoints: HashMap::new(),
            controller_resets: vec!["pid".into()],
        }];
        state.io.inputs.insert("x".into(), 0.0.into());
        state.io.inputs.insert("sensor".into(), 0.0.into());
        state.setpoints.insert("pid".into(), 20.0.into());
        let state = rt.next((&state, "i", &dt)).unwrap();
        assert_eq!(
            *state.io.outputs.get("actuator").unwrap(),
            Value::Decimal(1020.0)
        );
        let mut state = rt.next((&state, "i", &dt)).unwrap();
        assert_eq!(
            *state.io.outputs.get("actuator").unwrap(),
            Value::Decimal(3040.0)
        );
        assert_eq!(
            *state.controllers.get("pid").unwrap(),
            ControllerState::Pid(PidState {
                target: 20.0,
                prev_value: Some(0.0),
                p: 40.0,
                i: 3000.0,
                d: 0.0,
            })
        );
        // trigger the rule
        state.io.inputs.insert("x".into(), 10.0.into());
        // make sure k_d is not 0.0
        state.io.inputs.insert("sensor".into(), 1.0.into());
        let state = rt.next((&state, "i", &dt)).unwrap();
        assert_eq!(
            *state.controllers.get("pid").unwrap(),
            ControllerState::Pid(PidState {
                target: 10.0,
                prev_value: None,
                p: 0.0,
                i: 0.0,
                d: 0.0,
            })
        );
    }

    #[test]
    fn runtime_state() {
        let mut s = SystemState::default();
        //let mut io = IoState::default();
        let mut rt = SyncRuntime::default();
        let dt = Duration::from_secs(1);
        s.io.inputs.insert("a".into(), 8.0.into());
        s.io.inputs.insert("b".into(), false.into());
        s.io.inputs.insert("j".into(), 0.0.into());
        s.io.inputs.insert("k".into(), 0.0.into());
        s.io.inputs.insert("x".into(), 1.0.into());
        s.io.inputs.insert("z".into(), 3.0.into());
        s.io.outputs.insert("y".into(), 2.0.into());

        assert_eq!(rt.next((&s, "i", &dt)).unwrap(), s);

        rt.rules = vec![Rule {
            id: "foo".into(),
            condition: BooleanExpr::Eval(Source::In("x".into()).cmp_ge(Source::Out("y".into()))),
            actions: vec!["a".into()],
        }];
        let state = rt.next((&s, "default", &dt)).unwrap();
        assert_eq!(state.rules.len(), 1);
        assert_eq!(*state.rules.get("foo").unwrap(), false);
        assert_eq!(state.io.inputs.get("x").unwrap(), &Value::from(1.0));
        assert_eq!(state.io.outputs.get("y").unwrap(), &Value::from(2.0));

        let mut bb_cfg = BangBangConfig::default();
        bb_cfg.default_threshold = 2.0;
        let bb = ControllerConfig::BangBang(bb_cfg);

        let mut pid_cfg = PidConfig::default();
        pid_cfg.k_p = 2.0;
        pid_cfg.default_target = 10.0;
        let pid = ControllerConfig::Pid(pid_cfg);

        let loops = vec![
            Loop {
                id: "bb".into(),
                inputs: vec!["a".into()],
                outputs: vec!["b".into()],
                controller: bb,
            },
            Loop {
                id: "pid".into(),
                inputs: vec!["j".into()],
                outputs: vec!["k".into()],
                controller: pid,
            },
        ];
        rt.loops.insert("default".into(), loops);
        let state = rt.next((&s, "default", &dt)).unwrap();
        assert_eq!(state.io.outputs.get("b").unwrap(), &Value::from(true));
        assert_eq!(state.io.outputs.get("k").unwrap(), &Value::from(20.0));
        assert_eq!(
            state.controllers.get("bb").unwrap(),
            &ControllerState::BangBang(BangBangState {
                current: true,
                threshold: 2.0
            })
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
    fn only_run_loops_of_corresponding_interval_id() {
        let mut rt = SyncRuntime::default();
        let dt = Duration::from_secs(1);
        let mut s = SystemState::default();

        s.io.inputs.insert("a".into(), 3.0.into());
        s.io.inputs.insert("x".into(), 1.0.into());

        let mut bb_cfg = BangBangConfig::default();
        bb_cfg.default_threshold = 2.0;
        let bb = ControllerConfig::BangBang(bb_cfg);

        let mut pid_cfg = PidConfig::default();
        pid_cfg.k_p = 2.0;
        pid_cfg.default_target = 10.0;
        let pid = ControllerConfig::Pid(pid_cfg);

        rt.loops.insert(
            "bb".into(),
            vec![Loop {
                id: "bb".into(),
                inputs: vec!["a".into()],
                outputs: vec!["b".into()],
                controller: bb,
            }],
        );

        rt.loops.insert(
            "pid".into(),
            vec![Loop {
                id: "pid".into(),
                inputs: vec!["x".into()],
                outputs: vec!["y".into()],
                controller: pid,
            }],
        );

        let mut s = rt.next((&s, "bb", &dt)).unwrap();
        assert_eq!(*s.io.outputs.get("b").unwrap(), Value::from(true));
        assert!(s.io.outputs.get("y").is_none());

        s.io.inputs.insert("a".into(), 0.0.into());
        let s = rt.next((&s, "pid", &dt)).unwrap();
        assert_eq!(*s.io.outputs.get("b").unwrap(), Value::from(true));
        assert_eq!(*s.io.outputs.get("y").unwrap(), Value::from(18.0));

        let s = rt.next((&s, "bb", &dt)).unwrap();
        assert_eq!(*s.io.outputs.get("b").unwrap(), Value::from(false));
        assert_eq!(*s.io.outputs.get("y").unwrap(), Value::from(18.0));
    }

    #[test]
    fn apply_setpoints_to_controllers() {
        let mut pid_cfg = PidConfig::default();
        pid_cfg.k_p = 2.0;
        let pid_controller = ControllerConfig::Pid(pid_cfg);
        let mut bb_cfg = BangBangConfig::default();
        bb_cfg.default_threshold = 2.0;
        let bb = ControllerConfig::BangBang(bb_cfg);
        let dt = Duration::from_secs(1);
        let loops = vec![
            Loop {
                id: "pid".into(),
                inputs: vec!["sensor".into()],
                outputs: vec!["actuator".into()],
                controller: pid_controller,
            },
            Loop {
                id: "bb".into(),
                inputs: vec!["a".into()],
                outputs: vec!["b".into()],
                controller: bb,
            },
        ];
        let mut state = SystemState::default();
        let mut runtime = SyncRuntime::default();
        runtime.loops.insert("interval".into(), loops);
        state.io.inputs.insert("sensor".into(), 0.0.into());
        state.io.inputs.insert("a".into(), 0.0.into());
        let mut state = runtime.next((&state, "interval", &dt)).unwrap();
        let mut expected_pid_state = PidState::default();
        expected_pid_state.prev_value = Some(0.0.into());
        assert_eq!(
            *state.io.outputs.get("actuator").unwrap(),
            Value::Decimal(0.0)
        );
        assert_eq!(
            *state.controllers.get("pid").unwrap(),
            ControllerState::Pid(expected_pid_state)
        );
        state.setpoints.insert("pid".into(), Value::Decimal(100.0));
        let state = runtime.next((&state, "foo-interval", &dt)).unwrap();
        assert_eq!(
            *state.io.outputs.get("actuator").unwrap(),
            Value::Decimal(0.0)
        );
        assert_eq!(
            *state.controllers.get("pid").unwrap(),
            ControllerState::Pid(expected_pid_state)
        );
        let mut state = runtime.next((&state, "interval", &dt)).unwrap();
        expected_pid_state.target = 100.0;
        assert_eq!(
            *state.io.outputs.get("actuator").unwrap(),
            Value::Decimal(200.0)
        );
        state.setpoints.insert("bb".into(), Value::Decimal(-30.0));
        let state = runtime.next((&state, "interval", &dt)).unwrap();
        assert_eq!(*state.io.outputs.get("b").unwrap(), Value::Bit(true));
    }

    #[test]
    fn check_fsm_states() {
        let dt = Duration::from_secs(1);
        let sm = StateMachine {
            initial: "start".into(),
            transitions: vec![
                Transition {
                    condition: BooleanExpr::Eval(
                        Source::In("x".into()).cmp_gt(Source::Const(1.0.into())),
                    ),
                    from: "start".into(),
                    to: "step-one".into(),
                    actions: vec![],
                },
                Transition {
                    condition: BooleanExpr::Eval(
                        Source::In("y".into()).cmp_gt(Source::Const(2.0.into())),
                    ),
                    from: "step-one".into(),
                    to: "step-two".into(),
                    actions: vec![],
                },
            ],
        };
        let mut rt = SyncRuntime::default();
        rt.state_machines.insert("fsm".into(), sm);
        let mut state = SystemState::default();
        state.io.inputs.insert("x".into(), 0.0.into());
        let mut state = rt.next((&state, "i", &dt)).unwrap();
        assert!(state.state_machines.get("fsm").is_none());
        state.state_machines.insert("fsm".into(), "start".into());
        state.io.inputs.insert("x".into(), 1.5.into());
        let state = rt.next((&state, "i", &dt)).unwrap();
        assert_eq!(
            *state.state_machines.get("fsm").unwrap(),
            "step-one".to_string()
        );
    }

    #[test]
    fn apply_fsm_transition_actions() {
        let dt = Duration::from_secs(1);
        let sm = StateMachine {
            initial: "start".into(),
            transitions: vec![
                Transition {
                    condition: BooleanExpr::Eval(
                        Source::In("x".into()).cmp_eq(Source::Const(true.into())),
                    ),
                    from: "start".into(),
                    to: "step-one".into(),
                    actions: vec!["foo".into()],
                },
                Transition {
                    condition: BooleanExpr::Eval(
                        Source::In("y".into()).cmp_eq(Source::Const(123.into())),
                    ),
                    from: "step-one".into(),
                    to: "step-two".into(),
                    actions: vec!["bar".into()],
                },
            ],
        };
        let mut rt = SyncRuntime::default();
        let mut foo_outputs = HashMap::new();
        let mut bar_setpoints = HashMap::new();
        foo_outputs.insert("x".to_string(), Value::from(99.9).into());
        bar_setpoints.insert("y".to_string(), Value::from(-100).into());
        rt.actions = vec![
            Action {
                id: "foo".into(),
                outputs: foo_outputs,
                setpoints: HashMap::new(),
                controller_resets: vec![],
            },
            Action {
                id: "bar".into(),
                outputs: HashMap::new(),
                setpoints: bar_setpoints,
                controller_resets: vec![],
            },
        ];
        rt.state_machines.insert("fsm".into(), sm);
        let mut state = SystemState::default();
        state.io.inputs.insert("x".into(), false.into());
        let mut state = rt.next((&state, "i", &dt)).unwrap();
        state.state_machines.insert("fsm".into(), "start".into());
        assert!(state.io.outputs.get("x").is_none());
        assert!(state.setpoints.get("y").is_none());
        state.io.inputs.insert("x".into(), true.into());
        let mut state = rt.next((&state, "i", &dt)).unwrap();
        assert_eq!(*state.io.outputs.get("x").unwrap(), Value::from(99.9));
        assert!(state.setpoints.get("y").is_none());
        state.io.inputs.insert("y".into(), 123.into());
        let state = rt.next((&state, "i", &dt)).unwrap();
        assert_eq!(*state.setpoints.get("y").unwrap(), Value::from(-100));
    }
}
