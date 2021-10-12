use super::*;

#[test]
fn value_types() {
    assert_eq!(Type::Bool, Value::from(true).to_type());
    assert_eq!(Type::Bool, Value::from(false).to_type());
    assert_eq!(Type::I8, Value::from(-123i8).to_type());
    assert_eq!(Type::U8, Value::from(123u8).to_type());
    assert_eq!(Type::I16, Value::from(-123i16).to_type());
    assert_eq!(Type::U16, Value::from(123u16).to_type());
    assert_eq!(Type::I32, Value::from(-123i32).to_type());
    assert_eq!(Type::U32, Value::from(123u32).to_type());
    assert_eq!(Type::F32, Value::from(1.234_f32).to_type());
    assert_eq!(Type::I64, Value::from(-123i64).to_type());
    assert_eq!(Type::U64, Value::from(123u64).to_type());
    assert_eq!(Type::F64, Value::from(1.234).to_type());
}

#[test]
fn try_type_from_str() {
    assert_eq!(Some(Type::Bool), Type::try_from_str(TYPE_STR_BOOL));
    assert_eq!(Some(Type::I8), Type::try_from_str(TYPE_STR_I8));
    assert_eq!(Some(Type::U8), Type::try_from_str(TYPE_STR_U8));
    assert_eq!(Some(Type::I16), Type::try_from_str(TYPE_STR_I16));
    assert_eq!(Some(Type::U16), Type::try_from_str(TYPE_STR_U16));
    assert_eq!(Some(Type::I32), Type::try_from_str(TYPE_STR_I32));
    assert_eq!(Some(Type::U32), Type::try_from_str(TYPE_STR_U32));
    assert_eq!(Some(Type::F32), Type::try_from_str(TYPE_STR_F32));
    assert_eq!(Some(Type::I64), Type::try_from_str(TYPE_STR_I64));
    assert_eq!(Some(Type::U64), Type::try_from_str(TYPE_STR_U64));
    assert_eq!(Some(Type::F64), Type::try_from_str(TYPE_STR_F64));
}
