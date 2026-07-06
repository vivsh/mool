//! Typed output projection targets for derived SELECT expressions.

use std::any::TypeId;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use super::expr::{ExprNode, IntoExpr};

/// Provides generated output targets for a record projection.
#[doc(hidden)]
pub trait HasOutputCols: 'static {
    /// Generated output target struct for this record.
    type OutputColumns: Clone;

    /// Builds output targets owned by this record type.
    fn output_columns(source: OutputSource) -> Self::OutputColumns;
}

/// Output target source passed to generated record metadata.
#[derive(Clone)]
pub struct OutputSource {
    record: TypeId,
    record_name: &'static str,
    prefix: Option<Arc<str>>,
}

/// Typed output field target accepted only by read and returning assignments.
pub struct OutputColumn<T> {
    pub(super) data: Arc<OutputColumnData>,
    _marker: PhantomData<fn() -> T>,
}

#[derive(Debug)]
pub(super) struct OutputColumnData {
    pub(super) record: TypeId,
    pub(super) record_name: &'static str,
    pub(super) name: Arc<str>,
}

/// Internal builder for derived output projection assignments.
#[derive(Clone, Default)]
pub struct Selects {
    pub(super) assignments: Vec<SelectAssignment>,
}

#[derive(Clone)]
pub(super) struct SelectAssignment {
    pub(super) target: OutputRef,
    pub(super) expr: ExprNode,
}

#[derive(Clone)]
pub(super) struct OutputRef {
    pub(super) record: TypeId,
    pub(super) record_name: &'static str,
    pub(super) name: Arc<str>,
}

/// Converts owned or borrowed output columns into assignment targets.
#[doc(hidden)]
pub trait IntoOutputTarget<T> {
    /// Returns the output column target.
    fn into_output_target(self) -> OutputColumn<T>;
}

impl OutputSource {
    pub(super) fn new<R>() -> Self
    where
        R: 'static,
    {
        Self {
            record: TypeId::of::<R>(),
            record_name: std::any::type_name::<R>(),
            prefix: None,
        }
    }

    /// Returns a nested output source for reference projection fields.
    #[doc(hidden)]
    pub fn nested(&self, prefix: &'static str) -> Self {
        Self {
            record: self.record,
            record_name: self.record_name,
            prefix: Some(Arc::from(prefix)),
        }
    }

    /// Returns a typed output target for generated output structs.
    #[doc(hidden)]
    pub fn col<T>(&self, name: &'static str) -> OutputColumn<T> {
        let name = match &self.prefix {
            Some(prefix) => Arc::from(format!("{prefix}.{name}")),
            None => Arc::from(name),
        };
        OutputColumn {
            data: Arc::new(OutputColumnData {
                record: self.record,
                record_name: self.record_name,
                name,
            }),
            _marker: PhantomData,
        }
    }
}

impl<T> Clone for OutputColumn<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> IntoOutputTarget<T> for OutputColumn<T> {
    fn into_output_target(self) -> OutputColumn<T> {
        self
    }
}

impl<T> IntoOutputTarget<T> for &OutputColumn<T> {
    fn into_output_target(self) -> OutputColumn<T> {
        self.clone()
    }
}

impl Selects {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn set_in_place<T>(&mut self, target: &OutputColumn<T>, expr: ExprNode) {
        self.assignments.push(SelectAssignment {
            target: OutputRef {
                record: target.data.record,
                record_name: target.data.record_name,
                name: target.data.name.clone(),
            },
            expr,
        });
    }
}

pub(super) fn select_assignment<T>(
    target: impl IntoOutputTarget<T>,
    expr: impl IntoExpr<T>,
) -> SelectAssignment {
    let target = target.into_output_target();
    SelectAssignment {
        target: OutputRef {
            record: target.data.record,
            record_name: target.data.record_name,
            name: target.data.name.clone(),
        },
        expr: expr.into_expr().node,
    }
}

/// Output assignment builder retained for hidden callback compatibility.
#[doc(hidden)]
pub struct ReadUsing<R>
where
    R: HasOutputCols,
{
    columns: Arc<R::OutputColumns>,
    assignments: Rc<RefCell<Selects>>,
}

/// Returning assignment builder retained for hidden callback compatibility.
#[doc(hidden)]
pub struct ReturningUsing<R>
where
    R: HasOutputCols,
{
    columns: Arc<R::OutputColumns>,
    assignments: Rc<RefCell<Selects>>,
}

impl<R> ReadUsing<R>
where
    R: HasOutputCols,
{
    pub(super) fn new() -> Self {
        Self {
            columns: Arc::new(R::output_columns(OutputSource::new::<R>())),
            assignments: Rc::new(RefCell::new(Selects::new())),
        }
    }

    /// Assigns a typed expression to a read output target.
    pub fn set<T>(&self, target: &OutputColumn<T>, expr: impl IntoExpr<T>) -> Self {
        self.assignments
            .borrow_mut()
            .set_in_place(target, expr.into_expr().node);
        Self {
            columns: self.columns.clone(),
            assignments: self.assignments.clone(),
        }
    }

    pub(super) fn into_selects(self) -> Selects {
        into_selects(self.assignments)
    }
}

impl<R> ReturningUsing<R>
where
    R: HasOutputCols,
{
    pub(super) fn new() -> Self {
        Self {
            columns: Arc::new(R::output_columns(OutputSource::new::<R>())),
            assignments: Rc::new(RefCell::new(Selects::new())),
        }
    }

    /// Assigns a typed expression to a returning output target.
    pub fn set<T>(&self, target: &OutputColumn<T>, expr: impl IntoExpr<T>) -> Self {
        self.assignments
            .borrow_mut()
            .set_in_place(target, expr.into_expr().node);
        Self {
            columns: self.columns.clone(),
            assignments: self.assignments.clone(),
        }
    }

    pub(super) fn into_selects(self) -> Selects {
        into_selects(self.assignments)
    }
}

impl<R> Deref for ReadUsing<R>
where
    R: HasOutputCols,
{
    type Target = R::OutputColumns;

    fn deref(&self) -> &Self::Target {
        self.columns.as_ref()
    }
}

impl<R> Deref for ReturningUsing<R>
where
    R: HasOutputCols,
{
    type Target = R::OutputColumns;

    fn deref(&self) -> &Self::Target {
        self.columns.as_ref()
    }
}

fn into_selects(assignments: Rc<RefCell<Selects>>) -> Selects {
    match Rc::try_unwrap(assignments) {
        Ok(assignments) => assignments.into_inner(),
        Err(assignments) => assignments.borrow().clone(),
    }
}
