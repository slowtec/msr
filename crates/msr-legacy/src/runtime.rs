use super::*;
use crate::fsm::*;
use std::{collections::HashMap, io, result, time::Duration};

/// A simple synchronous closed-loop runtime.
#[derive(Debug)]
pub struct SyncRuntime {
    /// Loops
    pub loops: Vec<Loop>,
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
            loops: vec![],
            rules: vec![],
            actions: vec![],
            state_machines: HashMap::new(),
        }
    }
}

/// A runtime error
#[derive(Debug)]
pub struct Error<T> {
    pub state: T,
    pub causes: Vec<io::Error>,
}

type Result<T> = result::Result<T, Error<T>>;

//TODO: tidy up!
impl<'a> PureController<(&'a SystemState, &'a Duration), Result<SystemState>> for SyncRuntime {
    fn next(&self, input: (&SystemState, &Duration)) -> Result<SystemState> {
        let (orig_state, dt) = input;
        let mut state = orig_state.clone();
        let mut errors = vec![];

        for (id, s) in &orig_state.setpoints {
            if self.loops.iter().any(|l| l.id == *id) {
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

        //TODO: don't clone
        let ignore = state.inactive_loops.clone();
        for l in self
            .loops
            .iter()
            .filter(|l| !ignore.iter().any(|x| *x == l.id))
        {
            if state.controllers.get(&l.id).is_none() {
                self.initialize_controller_state(l, &mut state);
            }

            let res = l.next((
                state
                    .controllers
                    .get(&l.id)
                    .expect("The controller state was not initialized"),
                &state.io,
                dt,
            ));
            match res {
                Ok(x) => {
                    let (new_controller, new_io) = x;
                    state.io = new_io;
                    state.controllers.insert(l.id.clone(), new_controller);
                }
                Err(err) => {
                    errors.push(err);
                }
            }
        }

        for (id, t) in &orig_state.timeouts {
            if let Value::Timeout(t) = t {
                match t.checked_sub(*dt) {
                    Some(x) => {
                        state.timeouts.insert(id.clone(), x.into());
                    }
                    None => {
                        state
                            .timeouts
                            .insert(id.clone(), Duration::new(0, 0).into());
                    }
                }
            }
        }
        match self.rules_state(&state) {
            Ok(rules) => {
                state.rules = rules;
            }
            Err(err) => {
                state.rules = err.state;
                for e in err.causes {
                    errors.push(e);
                }
            }
        }

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
            self.apply_actions(&x, orig_state, &mut state);
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
            self.apply_actions(&x, orig_state, &mut state);
        }

        if !errors.is_empty() {
            return Err(Error {
                state,
                causes: errors,
            });
        }
        Ok(state)
    }
}

impl SyncRuntime {
    /// Check for active [Rule]s.
    fn rules_state(&self, state: &SystemState) -> Result<HashMap<String, bool>> {
        let mut rules_state = HashMap::new();
        let mut errors = vec![];
        for r in &self.rules {
            match r.condition.eval(state) {
                Ok(r_state) => {
                    rules_state.insert(r.id.clone(), r_state);
                }
                Err(e) => {
                    errors.push(e);
                }
            }
        }
        if !errors.is_empty() {
            return Err(Error {
                state: rules_state,
                causes: errors,
            });
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

    fn apply_actions(&self, actions: &[String], orig_state: &SystemState, state: &mut SystemState) {
        for a_id in actions {
            if let Some(a) = self.actions.iter().find(|a| a.id == *a_id) {
                for (k, src) in &a.outputs {
                    if let Some(v) = orig_state.get(&src) {
                        state.io.outputs.insert(k.clone(), v.clone());
                    }
                }
                for (k, src) in &a.setpoints {
                    if let Some(v) = orig_state.get(&src) {
                        state.setpoints.insert(k.clone(), v.clone());
                    }
                }
                for (k, src) in &a.memory {
                    if let Some(v) = orig_state.get(&src) {
                        state.io.mem.insert(k.clone(), v.clone());
                    }
                }
                for (id, ctl) in &a.controllers {
                    if ctl.reset {
                        state.controllers.remove(id);
                        if let Some(l) = self.loops.iter().find(|l| l.id == *id) {
                            self.initialize_controller_state(l, state);
                        }
                    }
                    if let Some(act) = ctl.active {
                        if act {
                            if let Some(idx) = state.inactive_loops.iter().position(|x| x == id) {
                                state.inactive_loops.remove(idx);
                            }
                        } else if !state.inactive_loops.iter().any(|l| l == id) {
                            state.inactive_loops.push(id.to_string());
                        }
                    }
                }
                for (id, t) in &a.timeouts {
                    match t {
                        Some(t) => {
                            if state.timeouts.get(id).is_none() {
                                state.timeouts.insert(id.clone(), (*t).into());
                            }
                        }
                        None => {
                            state.timeouts.remove(id);
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
        assert!(rt.next((&s, &dt)).is_ok());
        rt.loops.push(loop0);
        assert!(rt.next((&s, &dt)).is_err());
        rt.loops[0].inputs = vec!["input".into()];
        assert!(rt.next((&s, &dt)).is_err());
        rt.loops[0].outputs = vec!["output".into()];
        assert!(rt.next((&s, &dt)).is_ok());
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
        rt.loops = loops;
        let mut s = SystemState::default();
        s.io.inputs.insert("input".into(), true.into());
        assert!(rt.next((&s, &dt)).is_err());
        s.io.inputs.insert("input".into(), Value::Bin(vec![]));
        assert!(rt.next((&s, &dt)).is_err());
        s.io.inputs.insert("input".into(), 0.0.into());
        assert!(rt.next((&s, &dt)).is_ok());
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
        rt.loops = loops;
        let mut s = SystemState::default();
        s.io.inputs.insert("sensor".into(), 0.0.into());
        let s = rt.next((&s, &dt)).unwrap();
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
        rt.loops = loops;
        let mut s = SystemState::default();
        s.io.inputs.insert(sensor.clone(), 0.0.into());
        let mut s = rt.next((&s, &dt)).unwrap();
        assert_eq!(*s.io.outputs.get(&actuator).unwrap(), Value::Bit(false));
        s.io.inputs.insert(sensor, 3.0.into());
        let s = rt.next((&s, &dt)).unwrap();
        assert_eq!(*s.io.outputs.get(&actuator).unwrap(), Value::Bit(true));
    }

    #[test]
    fn check_active_rules() {
        let mut state = SystemState::default();
        let mut rt = SyncRuntime::default();
        assert_eq!(rt.rules_state(&mut state).unwrap().len(), 0);
        rt.rules = vec![Rule {
            id: "foo".into(),
            condition: BoolExpr::Eval(Source::In("x".into()).cmp_ge(Source::Out("y".into()))),
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
        let dt = Duration::from_millis(1);
        rt.rules = vec![Rule {
            id: "foo".into(),
            condition: BoolExpr::Eval(Source::In("x".into()).cmp_eq(Source::Const(10.0.into()))),
            actions: vec!["a".into()],
        }];
        let mut outputs = HashMap::new();
        let mut setpoints = HashMap::new();
        let mut memory = HashMap::new();
        let mut timeouts = HashMap::new();

        outputs.insert("z".into(), Source::Const(6.into()));
        outputs.insert("j".into(), Source::In("ref-in".into()));
        outputs.insert("k".into(), Source::Out("ref-out".into()));

        setpoints.insert("foo".into(), Source::Const(99.7.into()));
        setpoints.insert("bar".into(), Source::In("a".into()));
        setpoints.insert("baz".into(), Source::Out("b".into()));

        memory.insert(
            "a-message".into(),
            Source::Const("hello memory".to_string().into()),
        );

        timeouts.insert("a-timeout".into(), Some(Duration::from_millis(100).into()));
        timeouts.insert("an-other-timeout".into(), None);
        let controllers = HashMap::new();

        rt.actions = vec![Action {
            id: "a".into(),
            outputs,
            setpoints,
            timeouts,
            memory,
            controllers,
        }];
        state.io.inputs.insert("x".into(), 0.0.into());
        state
            .timeouts
            .insert("an-other-timeout".into(), Duration::from_millis(100).into());
        let mut state = rt.next((&state, &dt)).unwrap();
        assert!(state.io.outputs.get("z").is_none());
        assert!(state.io.outputs.get("j").is_none());
        assert!(state.io.outputs.get("k").is_none());
        assert!(state.setpoints.get("foo").is_none());
        assert!(state.setpoints.get("bar").is_none());
        assert!(state.setpoints.get("baz").is_none());
        assert!(state.io.mem.get("a-massage").is_none());
        assert!(state.timeouts.get("a-timeout").is_none());
        assert_eq!(
            *state.timeouts.get("an-other-timeout").unwrap(),
            Value::Timeout(Duration::from_millis(99).into())
        );
        state.io.inputs.insert("x".into(), 10.0.into());
        state.io.inputs.insert("ref-in".into(), 33.0.into());
        state.io.inputs.insert("a".into(), true.into());
        state
            .io
            .outputs
            .insert("ref-out".into(), "bla".to_string().into());
        state.io.outputs.insert("b".into(), false.into());
        let state = rt.next((&state, &dt)).unwrap();
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
        assert_eq!(
            *state.io.mem.get("a-message").unwrap(),
            Value::Text("hello memory".into())
        );
        assert_eq!(
            *state.timeouts.get("a-timeout").unwrap(),
            Value::Timeout(Duration::from_millis(100).into())
        );
        assert!(state.timeouts.get("an-other-timeout").is_none());
        let state = rt.next((&state, &dt)).unwrap();
        assert_eq!(
            *state.timeouts.get("a-timeout").unwrap(),
            Value::Timeout(Duration::from_millis(99).into())
        );
        let state = rt.next((&state, &Duration::from_millis(200))).unwrap();
        assert_eq!(
            *state.timeouts.get("a-timeout").unwrap(),
            Value::Timeout(Duration::from_millis(0).into())
        );
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
        rt.loops.push(Loop {
            id: "pid".into(),
            inputs: vec!["sensor".into()],
            outputs: vec!["actuator".into()],
            controller,
        });
        rt.rules = vec![Rule {
            id: "foo".into(),
            condition: BoolExpr::Eval(Source::In("x".into()).cmp_eq(Source::Const(10.0.into()))),
            actions: vec!["a".into()],
        }];

        let mut controllers = HashMap::new();
        controllers.insert(
            "pid".into(),
            ControllerAction {
                reset: true,
                active: None,
            },
        );

        rt.actions = vec![Action {
            id: "a".into(),
            outputs: HashMap::new(),
            setpoints: HashMap::new(),
            memory: HashMap::new(),
            timeouts: HashMap::new(),
            controllers,
        }];
        state.io.inputs.insert("x".into(), 0.0.into());
        state.io.inputs.insert("sensor".into(), 0.0.into());
        state.setpoints.insert("pid".into(), 20.0.into());
        let state = rt.next((&state, &dt)).unwrap();
        assert_eq!(
            *state.io.outputs.get("actuator").unwrap(),
            Value::Decimal(1020.0)
        );
        let mut state = rt.next((&state, &dt)).unwrap();
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
        let state = rt.next((&state, &dt)).unwrap();
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
    fn apply_controller_start_and_stop_actions() {
        let mut rt = SyncRuntime::default();
        let mut state = SystemState::default();
        let dt = Duration::from_secs(1);

        let mut pid_cfg = PidConfig::default();
        pid_cfg.k_i = 1.0;
        pid_cfg.default_target = 10.0;

        let controller_0 = ControllerConfig::Pid(pid_cfg.clone());
        let controller_1 = ControllerConfig::Pid(pid_cfg);

        rt.loops.push(Loop {
            id: "pid_0".into(),
            inputs: vec!["sensor".into()],
            outputs: vec!["actuator_0".into()],
            controller: controller_0,
        });
        rt.loops.push(Loop {
            id: "pid_1".into(),
            inputs: vec!["sensor".into()],
            outputs: vec!["actuator_1".into()],
            controller: controller_1,
        });

        rt.rules = vec![
            Rule {
                id: "foo".into(),
                condition: BoolExpr::Eval(
                    Source::In("x".into()).cmp_eq(Source::Const(10.0.into())),
                ),
                actions: vec!["a".into()],
            },
            Rule {
                id: "bar".into(),
                condition: BoolExpr::Eval(
                    Source::In("x".into()).cmp_eq(Source::Const(20.0.into())),
                ),
                actions: vec!["b".into()],
            },
        ];

        let mut controllers_a = HashMap::new();
        let mut controllers_b = HashMap::new();

        controllers_a.insert(
            "pid_1".into(),
            ControllerAction {
                reset: false,
                active: Some(false),
            },
        );

        controllers_b.insert(
            "pid_1".into(),
            ControllerAction {
                reset: false,
                active: Some(true),
            },
        );

        controllers_b.insert(
            "pid_0".into(),
            ControllerAction {
                reset: false,
                active: Some(true), // start it, even if it's already running
            },
        );

        rt.actions = vec![
            Action {
                id: "a".into(),
                outputs: HashMap::new(),
                setpoints: HashMap::new(),
                memory: HashMap::new(),
                timeouts: HashMap::new(),
                controllers: controllers_a,
            },
            Action {
                id: "b".into(),
                outputs: HashMap::new(),
                setpoints: HashMap::new(),
                memory: HashMap::new(),
                timeouts: HashMap::new(),
                controllers: controllers_b,
            },
        ];

        state.io.inputs.insert("x".into(), 0.0.into());
        state.io.inputs.insert("sensor".into(), 0.0.into());
        let state = rt.next((&state, &dt)).unwrap();
        assert_eq!(
            *state.io.outputs.get("actuator_0").unwrap(),
            Value::Decimal(20.0)
        );
        assert_eq!(
            *state.io.outputs.get("actuator_1").unwrap(),
            Value::Decimal(20.0)
        );

        let mut state = rt.next((&state, &dt)).unwrap();
        assert_eq!(
            *state.io.outputs.get("actuator_0").unwrap(),
            Value::Decimal(30.0)
        );
        assert_eq!(
            *state.io.outputs.get("actuator_1").unwrap(),
            Value::Decimal(30.0)
        );
        // trigger the rule "a"
        state.io.inputs.insert("x".into(), 10.0.into());
        let state = rt.next((&state, &dt)).unwrap();
        let mut state = rt.next((&state, &dt)).unwrap();
        assert_eq!(state.inactive_loops, vec!["pid_1"]);
        assert_eq!(state.io.outputs.get("actuator_0"), Some(&Value::from(50.0)));
        assert_eq!(state.io.outputs.get("actuator_1"), Some(&Value::from(40.0)));

        // rule "a" is no longer active
        state.io.inputs.insert("x".into(), 0.0.into());
        let state = rt.next((&state, &dt)).unwrap();
        let mut state = rt.next((&state, &dt)).unwrap();
        assert_eq!(state.inactive_loops, vec!["pid_1"]);
        assert_eq!(state.io.outputs.get("actuator_0"), Some(&Value::from(70.0)));
        assert_eq!(state.io.outputs.get("actuator_1"), Some(&Value::from(40.0)));

        // trigger the rule "b"
        state.io.inputs.insert("x".into(), 20.0.into());
        let state = rt.next((&state, &dt)).unwrap();
        assert!(state.inactive_loops.is_empty());
        assert_eq!(state.io.outputs.get("actuator_0"), Some(&Value::from(80.0)));
        assert_eq!(state.io.outputs.get("actuator_1"), Some(&Value::from(40.0)));

        let state = rt.next((&state, &dt)).unwrap();
        assert!(state.inactive_loops.is_empty());
        assert_eq!(state.io.outputs.get("actuator_0"), Some(&Value::from(90.0)));
        assert_eq!(state.io.outputs.get("actuator_1"), Some(&Value::from(50.0)));
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

        assert_eq!(rt.next((&s, &dt)).unwrap(), s);

        rt.rules = vec![Rule {
            id: "foo".into(),
            condition: BoolExpr::Eval(Source::In("x".into()).cmp_ge(Source::Out("y".into()))),
            actions: vec!["a".into()],
        }];
        let state = rt.next((&s, &dt)).unwrap();
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
        rt.loops = loops;
        let state = rt.next((&s, &dt)).unwrap();
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
        runtime.loops = loops;
        state.io.inputs.insert("sensor".into(), 0.0.into());
        state.io.inputs.insert("a".into(), 0.0.into());
        let mut state = runtime.next((&state, &dt)).unwrap();
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
        let mut state = runtime.next((&state, &dt)).unwrap();
        expected_pid_state.target = 100.0;
        assert_eq!(
            *state.io.outputs.get("actuator").unwrap(),
            Value::Decimal(200.0)
        );
        state.setpoints.insert("bb".into(), Value::Decimal(-30.0));
        let state = runtime.next((&state, &dt)).unwrap();
        assert_eq!(*state.io.outputs.get("b").unwrap(), Value::Bit(true));
    }

    #[test]
    fn check_fsm_states() {
        let dt = Duration::from_secs(1);
        let sm = StateMachine {
            initial: "start".into(),
            transitions: vec![
                Transition {
                    condition: BoolExpr::Eval(
                        Source::In("x".into()).cmp_gt(Source::Const(1.0.into())),
                    ),
                    from: "start".into(),
                    to: "step-one".into(),
                    actions: vec![],
                },
                Transition {
                    condition: BoolExpr::Eval(
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
        let mut state = rt.next((&state, &dt)).unwrap();
        assert!(state.state_machines.get("fsm").is_none());
        state.state_machines.insert("fsm".into(), "start".into());
        state.io.inputs.insert("x".into(), 1.5.into());
        let state = rt.next((&state, &dt)).unwrap();
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
                    condition: BoolExpr::Eval(
                        Source::In("x".into()).cmp_eq(Source::Const(true.into())),
                    ),
                    from: "start".into(),
                    to: "step-one".into(),
                    actions: vec!["foo".into()],
                },
                Transition {
                    condition: BoolExpr::Eval(
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
                memory: HashMap::new(),
                timeouts: HashMap::new(),
                controllers: HashMap::new(),
            },
            Action {
                id: "bar".into(),
                outputs: HashMap::new(),
                setpoints: bar_setpoints,
                memory: HashMap::new(),
                timeouts: HashMap::new(),
                controllers: HashMap::new(),
            },
        ];
        rt.state_machines.insert("fsm".into(), sm);
        let mut state = SystemState::default();
        state.io.inputs.insert("x".into(), false.into());
        let mut state = rt.next((&state, &dt)).unwrap();
        state.state_machines.insert("fsm".into(), "start".into());
        assert!(state.io.outputs.get("x").is_none());
        assert!(state.setpoints.get("y").is_none());
        state.io.inputs.insert("x".into(), true.into());
        let mut state = rt.next((&state, &dt)).unwrap();
        assert_eq!(*state.io.outputs.get("x").unwrap(), Value::from(99.9));
        assert!(state.setpoints.get("y").is_none());
        state.io.inputs.insert("y".into(), 123.into());
        let state = rt.next((&state, &dt)).unwrap();
        assert_eq!(*state.setpoints.get("y").unwrap(), Value::from(-100));
    }

    #[test]
    fn collect_runtime_errors() {
        let dt = Duration::from_secs(1);
        let mut rt = SyncRuntime::default();
        let mut state = SystemState::default();
        let mut pid_cfg = PidConfig::default();
        pid_cfg.k_p = 2.0;
        let pid_controller_0 = ControllerConfig::Pid(pid_cfg.clone());
        let pid_controller_1 = ControllerConfig::Pid(pid_cfg);
        let loops = vec![
            Loop {
                id: "pid_0".into(),
                inputs: vec!["sensor_0".into()],
                outputs: vec!["actuator_0".into()],
                controller: pid_controller_0,
            },
            Loop {
                id: "pid_1".into(),
                inputs: vec!["sensor_1".into()],
                outputs: vec!["actuator_1".into()],
                controller: pid_controller_1,
            },
        ];
        state.io.inputs.insert("sensor_1".into(), 5.0.into());
        rt.loops = loops;
        let err = rt.next((&state, &dt)).err().unwrap();
        assert_eq!(err.causes.len(), 1);
        assert!(err.state.io.outputs.get("actuator_0").is_none());
        assert!(err.state.io.outputs.get("actuator_1").is_some());
    }
}
