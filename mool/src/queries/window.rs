//! Typed SQL window-function builders.

use super::expr::{ExprNode, IntoExpr, OrderExpr};

/// SQL window specification used by `expr.over(...)`.
#[derive(Clone, Default)]
pub struct WindowSpec {
    pub(super) partitions: Vec<ExprNode>,
    pub(super) orders: Vec<OrderExpr>,
    pub(super) frame: Option<WindowFrame>,
}

/// SQL window frame definition.
#[derive(Clone)]
pub struct WindowFrame {
    pub(super) unit: FrameUnit,
    pub(super) start: FrameBound,
    pub(super) end: FrameBound,
}

/// SQL window frame bound.
#[derive(Clone)]
pub struct FrameBound {
    pub(super) kind: FrameBoundKind,
    pub(super) expr: Option<Box<ExprNode>>,
}

#[derive(Clone, Copy)]
pub(super) enum FrameUnit {
    Rows,
    Range,
}

#[derive(Clone, Copy)]
pub(super) enum FrameBoundKind {
    UnboundedPreceding,
    Preceding,
    CurrentRow,
    Following,
    UnboundedFollowing,
}

impl WindowSpec {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Adds a typed `PARTITION BY` expression.
    pub fn partition_by<T>(mut self, expr: impl IntoExpr<T>) -> Self {
        self.partitions.push(expr.into_expr().node);
        self
    }

    /// Adds a typed `ORDER BY` expression.
    pub fn order_by(mut self, order: OrderExpr) -> Self {
        self.orders.push(order);
        self
    }

    /// Adds a `ROWS BETWEEN ... AND ...` frame.
    pub fn rows_between(mut self, start: FrameBound, end: FrameBound) -> Self {
        self.frame = Some(rows_between(start, end));
        self
    }

    /// Adds a `RANGE BETWEEN ... AND ...` frame.
    pub fn range_between(mut self, start: FrameBound, end: FrameBound) -> Self {
        self.frame = Some(range_between(start, end));
        self
    }

    /// Applies a prebuilt window frame.
    pub fn frame(mut self, frame: WindowFrame) -> Self {
        self.frame = Some(frame);
        self
    }
}

/// Starts an empty SQL window specification.
pub fn window() -> WindowSpec {
    WindowSpec::new()
}

/// Creates a `ROWS BETWEEN ... AND ...` window frame.
pub fn rows_between(start: FrameBound, end: FrameBound) -> WindowFrame {
    WindowFrame {
        unit: FrameUnit::Rows,
        start,
        end,
    }
}

/// Creates a `RANGE BETWEEN ... AND ...` window frame.
pub fn range_between(start: FrameBound, end: FrameBound) -> WindowFrame {
    WindowFrame {
        unit: FrameUnit::Range,
        start,
        end,
    }
}

/// Window frame bound `UNBOUNDED PRECEDING`.
pub fn unbounded_preceding() -> FrameBound {
    FrameBound::plain(FrameBoundKind::UnboundedPreceding)
}

/// Window frame bound `n PRECEDING`.
pub fn preceding(offset: impl IntoExpr<i64>) -> FrameBound {
    FrameBound::with_expr(FrameBoundKind::Preceding, offset.into_expr().node)
}

/// Window frame bound `CURRENT ROW`.
pub fn current_row() -> FrameBound {
    FrameBound::plain(FrameBoundKind::CurrentRow)
}

/// Window frame bound `n FOLLOWING`.
pub fn following(offset: impl IntoExpr<i64>) -> FrameBound {
    FrameBound::with_expr(FrameBoundKind::Following, offset.into_expr().node)
}

/// Window frame bound `UNBOUNDED FOLLOWING`.
pub fn unbounded_following() -> FrameBound {
    FrameBound::plain(FrameBoundKind::UnboundedFollowing)
}

impl FrameBound {
    fn plain(kind: FrameBoundKind) -> Self {
        Self { kind, expr: None }
    }

    fn with_expr(kind: FrameBoundKind, expr: ExprNode) -> Self {
        Self {
            kind,
            expr: Some(Box::new(expr)),
        }
    }
}
