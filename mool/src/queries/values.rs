//! Write-value builders for insert and update expressions.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::commons::Arguments;
use crate::interfaces::Record;

use super::expr::{ColumnRef, ExprNode, IntoExpr};
use super::handles::{Column, ColumnOwner, Table};
use super::traits::IntoColumnRef;
use super::validate::{table_name, validate_identifier};
use crate::QueryError;

/// Write input accepted by insert and update terminals.
#[doc(hidden)]
pub trait WriteInput {
    /// Builds planned columns, expressions, and pre-bound values.
    fn write_parts(&self, table: &Table) -> Result<WriteParts, QueryError>;
}

/// Internal expression-capable write-value builder.
pub struct WriteValues<'a, R = ()> {
    record: Option<&'a R>,
    sets: Vec<WriteSet>,
}

/// Write assignment builder retained for hidden callback compatibility.
#[derive(Clone)]
pub struct WriteUsing {
    values: Rc<RefCell<WriteValues<'static, NoRecord>>>,
}

/// Marker used by expression-only write terminals when no record payload is attached.
#[doc(hidden)]
pub struct NoRecord;

#[doc(hidden)]
pub struct WriteParts {
    pub(super) columns: Vec<String>,
    pub(super) slots: Vec<WriteSlot>,
    pub(super) args: Arguments<'static>,
    pub(super) prebound_count: usize,
}

pub(super) enum WriteSlot {
    Prebound(usize),
    Expr(ExprNode),
}

struct WriteSet {
    column: ColumnRef,
    expr: ExprNode,
}

impl WriteValues<'static, NoRecord> {
    pub(super) fn empty() -> Self {
        Self {
            record: None,
            sets: Vec::new(),
        }
    }
}

impl<'a, R> WriteValues<'a, R>
where
    R: Record,
{
    pub(super) fn record(record: &'a R) -> Self {
        Self {
            record: Some(record),
            sets: Vec::new(),
        }
    }
}

impl<'a, R> WriteValues<'a, R> {
    fn set_in_place<T>(&mut self, column: &Column<T>, expr: ExprNode) {
        self.sets.push(WriteSet {
            column: column.into_column_ref(),
            expr,
        });
    }

    /// Assigns a typed expression to a table column.
    pub fn set<T>(mut self, column: &Column<T>, expr: impl IntoExpr<T>) -> Self {
        self.set_in_place(column, expr.into_expr().node);
        self
    }

    pub(super) fn extend(mut self, overrides: WriteValues<'static, NoRecord>) -> Self {
        self.sets.extend(overrides.sets);
        self
    }
}

impl WriteUsing {
    pub(super) fn new() -> Self {
        Self {
            values: Rc::new(RefCell::new(WriteValues::empty())),
        }
    }

    /// Assigns a typed expression to a root table column.
    pub fn set<T>(&self, column: &Column<T>, expr: impl IntoExpr<T>) -> Self {
        self.values
            .borrow_mut()
            .set_in_place(column, expr.into_expr().node);
        Self {
            values: self.values.clone(),
        }
    }

    pub(super) fn into_values(self) -> WriteValues<'static, NoRecord> {
        match Rc::try_unwrap(self.values) {
            Ok(values) => values.into_inner(),
            Err(values) => values.borrow().clone(),
        }
    }
}

impl<T> WriteInput for &T
where
    T: Record,
{
    fn write_parts(&self, table: &Table) -> Result<WriteParts, QueryError> {
        let columns = bind_columns::<T>(table)?;
        let names = columns.iter().map(String::as_str).collect::<Vec<_>>();
        let mut args = Arguments::default();
        self.record_bind_selected(&names, &mut args)
            .map_err(|err| QueryError::BindError(err.to_string()))?;
        let slots = (1..=columns.len()).map(WriteSlot::Prebound).collect();
        Ok(WriteParts {
            prebound_count: columns.len(),
            columns,
            slots,
            args,
        })
    }
}

impl<R> WriteInput for WriteValues<'_, R>
where
    R: Record,
{
    fn write_parts(&self, table: &Table) -> Result<WriteParts, QueryError> {
        let mut state = WriteState::new(table);
        if let Some(record) = self.record {
            state.add_record::<R>()?;
            for set in &self.sets {
                state.add_set(set)?;
            }
            return state.finish_with_record(record);
        }
        for set in &self.sets {
            state.add_set(set)?;
        }
        state.finish()
    }
}

impl WriteInput for WriteValues<'_, NoRecord> {
    fn write_parts(&self, table: &Table) -> Result<WriteParts, QueryError> {
        let mut state = WriteState::new(table);
        for set in &self.sets {
            state.add_set(set)?;
        }
        state.finish()
    }
}

struct WriteState<'a> {
    table: &'a Table,
    columns: Vec<String>,
    slots: Vec<WriteSlot>,
}

impl<'a> WriteState<'a> {
    fn new(table: &'a Table) -> Self {
        Self {
            table,
            columns: Vec::new(),
            slots: Vec::new(),
        }
    }

    fn add_record<R>(&mut self) -> Result<(), QueryError>
    where
        R: Record,
    {
        let columns = bind_columns::<R>(self.table)?;
        for column in columns {
            self.push_prebound(column);
        }
        Ok(())
    }

    fn add_set(&mut self, set: &WriteSet) -> Result<(), QueryError> {
        let name = self.validate_set(set)?;
        if let Some(index) = self.columns.iter().position(|column| column == &name) {
            if matches!(self.slots.get(index), Some(WriteSlot::Expr(_))) {
                return Err(QueryError::BindError(format!(
                    "duplicate assignment for '{}'",
                    name
                )));
            }
            if let Some(slot) = self.slots.get_mut(index) {
                *slot = WriteSlot::Expr(set.expr.clone());
            }
            return Ok(());
        }
        self.columns.push(name);
        self.slots.push(WriteSlot::Expr(set.expr.clone()));
        Ok(())
    }

    fn finish(self) -> Result<WriteParts, QueryError> {
        if self.columns.is_empty() {
            return Err(QueryError::BindError("no write values".to_string()));
        }
        Ok(WriteParts {
            columns: self.columns,
            slots: self.slots,
            args: Arguments::default(),
            prebound_count: 0,
        })
    }

    fn finish_with_record<R>(mut self, record: &R) -> Result<WriteParts, QueryError>
    where
        R: Record,
    {
        if self.columns.is_empty() {
            return Err(QueryError::BindError("no write values".to_string()));
        }
        let selected = self.renumber_prebound_slots();
        let names = selected.iter().map(String::as_str).collect::<Vec<_>>();
        let mut args = Arguments::default();
        record
            .record_bind_selected(&names, &mut args)
            .map_err(|err| QueryError::BindError(err.to_string()))?;
        Ok(WriteParts {
            columns: self.columns,
            slots: self.slots,
            args,
            prebound_count: selected.len(),
        })
    }

    fn push_prebound(&mut self, column: String) {
        self.columns.push(column);
        self.slots.push(WriteSlot::Prebound(0));
    }

    fn renumber_prebound_slots(&mut self) -> Vec<String> {
        let mut selected = Vec::new();
        for (column, slot) in self.columns.iter().zip(self.slots.iter_mut()) {
            if let WriteSlot::Prebound(position) = slot {
                selected.push(column.clone());
                *position = selected.len();
            }
        }
        selected
    }

    fn validate_set(&self, set: &WriteSet) -> Result<String, QueryError> {
        let ColumnOwner::Root(table) = &set.column.owner else {
            return Err(QueryError::BindError(format!(
                "write assignment target '{}' is not a root table column",
                set.column.name
            )));
        };
        if table != self.table {
            return Err(QueryError::BindError(format!(
                "write assignment target '{}' belongs to another table",
                set.column.name
            )));
        }
        validate_identifier(&set.column.name)?;
        validate_known_column(self.table, &set.column.name)?;
        Ok(set.column.name.to_string())
    }
}

fn bind_columns<T>(table: &Table) -> Result<Vec<String>, QueryError>
where
    T: Record,
{
    let columns = T::record_bind_column_names();
    if columns.is_empty() {
        return Err(QueryError::BindError("no bindable columns".to_string()));
    }
    for column in &columns {
        validate_identifier(column)?;
        validate_known_column(table, column)?;
    }
    Ok(columns)
}

fn validate_known_column(table: &Table, column: &str) -> Result<(), QueryError> {
    let Some(known) = table.data.columns.as_ref() else {
        return Ok(());
    };
    if known.iter().any(|known| known == column) {
        return Ok(());
    }
    Err(QueryError::BindError(format!(
        "column '{}' is not writable for {}",
        column,
        table_name(table.data.schema.as_deref(), table.data.name.as_ref())
    )))
}

impl<R> Clone for WriteValues<'_, R> {
    fn clone(&self) -> Self {
        Self {
            record: self.record,
            sets: self.sets.clone(),
        }
    }
}

impl Clone for WriteSet {
    fn clone(&self) -> Self {
        Self {
            column: self.column.clone(),
            expr: self.expr.clone(),
        }
    }
}

impl Clone for WriteSlot {
    fn clone(&self) -> Self {
        match self {
            Self::Prebound(position) => Self::Prebound(*position),
            Self::Expr(expr) => Self::Expr(expr.clone()),
        }
    }
}

impl<R> std::fmt::Debug for WriteValues<'_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteValues")
            .field("sets", &self.sets.len())
            .finish()
    }
}

impl<R> Default for WriteValues<'_, R> {
    fn default() -> Self {
        Self {
            record: None,
            sets: Vec::new(),
        }
    }
}

impl<R> WriteValues<'_, R> {
    #[doc(hidden)]
    pub fn _marker(&self) -> PhantomData<fn() -> R> {
        PhantomData
    }
}
