use std::any::Any;
use std::sync::Arc;

use super::commons::{Arguments, Database};

type ArgBinder =
    dyn Fn(&mut Arguments<'static>) -> Result<(), sqlx::error::BoxDynError> + Send + Sync;

#[derive(Clone)]
pub struct ArgValue {
    storage: ArgStorage,
}

#[derive(Clone)]
enum ArgStorage {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(String),
    Dyn(Arc<ArgBinder>),
}

impl ArgValue {
    pub fn new<T>(val: T) -> Self
    where
        T: Clone
            + for<'q> sqlx::Encode<'q, Database>
            + sqlx::Type<Database>
            + Send
            + Sync
            + 'static,
    {
        if let Some(storage) = inline_storage(&val) {
            return Self { storage };
        }
        Self {
            storage: ArgStorage::Dyn(Arc::new(move |args| {
                use sqlx::Arguments as _;
                args.add(val.clone())
            })),
        }
    }

    pub fn bind_value(
        &self,
        args: &mut Arguments<'static>,
    ) -> Result<(), sqlx::error::BoxDynError> {
        use sqlx::Arguments as _;

        match &self.storage {
            ArgStorage::Bool(value) => args.add(*value),
            ArgStorage::I8(value) => args.add(*value),
            ArgStorage::I16(value) => args.add(*value),
            ArgStorage::I32(value) => args.add(*value),
            ArgStorage::I64(value) => args.add(*value),
            ArgStorage::F32(value) => args.add(*value),
            ArgStorage::F64(value) => args.add(*value),
            ArgStorage::String(value) => args.add(value.clone()),
            ArgStorage::Dyn(binder) => binder(args),
        }
    }
}

fn inline_storage<T>(val: &T) -> Option<ArgStorage>
where
    T: Any + Clone,
{
    let any = val as &dyn Any;
    if let Some(value) = any.downcast_ref::<bool>() {
        return Some(ArgStorage::Bool(*value));
    }
    if let Some(value) = any.downcast_ref::<i8>() {
        return Some(ArgStorage::I8(*value));
    }
    if let Some(value) = any.downcast_ref::<i16>() {
        return Some(ArgStorage::I16(*value));
    }
    if let Some(value) = any.downcast_ref::<i32>() {
        return Some(ArgStorage::I32(*value));
    }
    if let Some(value) = any.downcast_ref::<i64>() {
        return Some(ArgStorage::I64(*value));
    }
    if let Some(value) = any.downcast_ref::<f32>() {
        return Some(ArgStorage::F32(*value));
    }
    if let Some(value) = any.downcast_ref::<f64>() {
        return Some(ArgStorage::F64(*value));
    }
    any.downcast_ref::<String>()
        .map(|value| ArgStorage::String(value.clone()))
}

impl<T> From<T> for ArgValue
where
    T: Clone + for<'q> sqlx::Encode<'q, Database> + sqlx::Type<Database> + Send + Sync + 'static,
{
    fn from(val: T) -> Self {
        ArgValue::new(val)
    }
}

#[cfg(test)]
mod tests {
    use super::ArgValue;
    use crate::backend::Arguments;

    /// Verifies inline primitive and string bind values increment SQLx arguments.
    #[test]
    fn inline_values_bind_to_arguments() {
        use sqlx::Arguments as _;

        let values = [
            ArgValue::new(true),
            ArgValue::new(1_i8),
            ArgValue::new(2_i16),
            ArgValue::new(3_i32),
            ArgValue::new(4_i64),
            ArgValue::new(5.0_f32),
            ArgValue::new(6.0_f64),
            ArgValue::new("seven".to_string()),
        ];
        let mut args = Arguments::default();
        for value in values {
            value.bind_value(&mut args).unwrap();
        }
        assert_eq!(args.len(), 8);
    }

    /// Verifies fallback dynamic bind values keep the same bind behavior.
    #[test]
    fn fallback_values_bind_to_arguments() {
        use sqlx::Arguments as _;

        let value = ArgValue::new(vec![1_u8, 2, 3]);
        let mut args = Arguments::default();
        value.bind_value(&mut args).unwrap();
        assert_eq!(args.len(), 1);
    }
}
