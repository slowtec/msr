use super::*;
use std::io::{Error, ErrorKind, Result};
use std::str::FromStr;

impl FromStr for Comparison {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        use crate::Comparator::*;
        if s.trim().is_empty() {
            return Err(Error::new(ErrorKind::InvalidInput, "empty str"));
        }
        for cmp in &[GreaterOrEqual, Greater, Equal, LessOrEqual, Less, NotEqual] {
            if let Some(cmp) = parse_comparison(s, *cmp)? {
                return Ok(cmp);
            }
        }
        Err(Error::new(ErrorKind::InvalidInput, "invalid comparison"))
    }
}

fn comparator_as_str(cmp: Comparator) -> &'static str {
    use crate::Comparator::*;
    match cmp {
        Less => "<",
        LessOrEqual => "<=",
        Greater => ">",
        GreaterOrEqual => ">=",
        Equal => "==",
        NotEqual => "!=",
    }
}

fn parse_comparison(s: &str, cmp: Comparator) -> Result<Option<Comparison>> {
    let cmp_str = comparator_as_str(cmp);
    if s.contains(cmp_str) {
        let mut vals = s.split(cmp_str);
        if let Some(lhs) = vals.next() {
            if let Some(rhs) = vals.next() {
                if None == vals.next() {
                    return Ok(Some(Comparison {
                        left: Source::from_str(lhs)?,
                        cmp,
                        right: Source::from_str(rhs)?,
                    }));
                }
            }
        }
        Err(Error::new(
            ErrorKind::InvalidInput,
            format!("invalid number of arguments for comparator {}", cmp_str),
        ))
    } else {
        // Ignore input strings without a comparator
        Ok(None)
    }
}

impl FromStr for Source {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Err(Error::new(ErrorKind::InvalidInput, "empty str"));
        }
        if s.contains("'") {
            return Ok(Source::Const(Value::Text(s.replace("'", ""))));
        }
        if let Ok(v) = s.parse::<i64>() {
            return Ok(Source::Const(v.into()));
        }
        if let Ok(v) = s.parse::<f64>() {
            return Ok(Source::Const(v.into()));
        }
        let s = s.to_lowercase();
        if s.contains("timeout.") {
            let res = s.split("timeout.").collect::<Vec<&str>>();
            if res.len() < 2 || res[1].is_empty() {
                return Err(Error::new(ErrorKind::InvalidInput, "invalid identifier"));
            }
            return Ok(Source::Timeout(res[1].into()));
        }
        if s.contains("in.") {
            let res = s.split("in.").collect::<Vec<&str>>();
            if res.len() < 2 || res[1].is_empty() {
                return Err(Error::new(ErrorKind::InvalidInput, "invalid identifier"));
            }
            return Ok(Source::In(res[1].into()));
        }
        if s.contains("out.") {
            let res = s.split("out.").collect::<Vec<&str>>();
            if res.len() < 2 || res[1].is_empty() {
                return Err(Error::new(ErrorKind::InvalidInput, "invalid identifier"));
            }
            return Ok(Source::Out(res[1].into()));
        }
        if s.contains("mem.") {
            let res = s.split("mem.").collect::<Vec<&str>>();
            if res.len() < 2 || res[1].is_empty() {
                return Err(Error::new(ErrorKind::InvalidInput, "invalid identifier"));
            }
            return Ok(Source::Mem(res[1].into()));
        }
        if s.contains("setpoint.") {
            let res = s.split("setpoint.").collect::<Vec<&str>>();
            if res.len() < 2 || res[1].is_empty() {
                return Err(Error::new(ErrorKind::InvalidInput, "invalid identifier"));
            }
            return Ok(Source::Setpoint(res[1].into()));
        }
        if s.contains("true") {
            return Ok(Source::Const(true.into()));
        }
        if s.contains("false") {
            return Ok(Source::Const(false.into()));
        }
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_in_src() {
        assert!(Source::from_str("").is_err());
        assert!(Source::from_str(" ").is_err());
        assert!(Source::from_str("in.").is_err());
        assert!(Source::from_str("in. ").is_err());
        assert_eq!(Source::from_str("in.x").unwrap(), Source::In("x".into()));
        assert_eq!(Source::from_str("In.x").unwrap(), Source::In("x".into()));
        assert_eq!(Source::from_str("IN.x").unwrap(), Source::In("x".into()));
        assert_eq!(Source::from_str("in.X").unwrap(), Source::In("x".into()));
    }

    #[test]
    fn parse_out_src() {
        assert_eq!(Source::from_str("out.y").unwrap(), Source::Out("y".into()));
        assert_eq!(Source::from_str("OUT.y").unwrap(), Source::Out("y".into()));
    }

    #[test]
    fn parse_mem_src() {
        assert_eq!(Source::from_str("mem.y").unwrap(), Source::Mem("y".into()));
        assert_eq!(Source::from_str("MEM.y").unwrap(), Source::Mem("y".into()));
    }

    #[test]
    fn parse_setpoint_src() {
        assert_eq!(
            Source::from_str("setpoint.y").unwrap(),
            Source::Setpoint("y".into())
        );
        assert_eq!(
            Source::from_str("SETPOINT.y").unwrap(),
            Source::Setpoint("y".into())
        );
    }

    #[test]
    fn parse_timeout_src() {
        assert_eq!(
            Source::from_str("timeout.y").unwrap(),
            Source::Timeout("y".into())
        );
        assert_eq!(
            Source::from_str("TIMEOUT.y").unwrap(),
            Source::Timeout("y".into())
        );
    }

    #[test]
    fn parse_boolean_src() {
        assert_eq!(
            Source::from_str("true").unwrap(),
            Source::Const(true.into())
        );
        assert_eq!(
            Source::from_str("false").unwrap(),
            Source::Const(false.into())
        );
    }

    #[test]
    fn parse_int_src() {
        assert_eq!(Source::from_str("123").unwrap(), Source::Const(123.into()));
        assert_eq!(Source::from_str("0456").unwrap(), Source::Const(456.into()));
    }

    #[test]
    fn parse_float_src() {
        assert_eq!(
            Source::from_str("123.0").unwrap(),
            Source::Const(123.0.into())
        );
        assert_eq!(
            Source::from_str("0456.0").unwrap(),
            Source::Const(456.0.into())
        );
    }

    #[test]
    fn parse_text_src() {
        assert_eq!(
            Source::from_str("'foo Bar'").unwrap(),
            Source::Const(Value::Text("foo Bar".into()))
        );
        assert_eq!(
            Source::from_str("'in.x'").unwrap(),
            Source::Const(Value::Text("in.x".into()))
        );
    }

    #[test]
    fn parse_cmp() {
        use crate::Comparator::*;
        use crate::Source::*;
        assert!(Comparison::from_str("").is_err());
        assert!(Comparison::from_str(" ").is_err());
        assert!(Comparison::from_str("in.x ?? out.z").is_err());
        let tests = vec![
            (
                "in.x >= in.y",
                In("x".into()),
                GreaterOrEqual,
                In("y".into()),
            ),
            ("in.x > in.y", In("x".into()), Greater, In("y".into())),
            ("in.x == out.y", In("x".into()), Equal, Out("y".into())),
            (
                "out.z <= in.y",
                Out("z".into()),
                LessOrEqual,
                In("y".into()),
            ),
            ("out.z < in.y", Out("z".into()), Less, In("y".into())),
            ("out.z != in.y", Out("z".into()), NotEqual, In("y".into())),
            (
                "timeout.t == true",
                Timeout("t".into()),
                Equal,
                Const(true.into()),
            ),
        ];

        for (s, left, cmp, right) in tests {
            assert_eq!(
                Comparison::from_str(s).unwrap(),
                Comparison { left, cmp, right }
            );
        }
    }
}
