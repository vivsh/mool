//! Window-function SQL rendering helpers.

use super::dialect::{self, DialectFeature};
use super::expr::{ExprNode, OrderExpr};
use super::render::{RenderMode, Renderer};
use super::window::{FrameBound, FrameBoundKind, FrameUnit, WindowFrame, WindowSpec};
use crate::QueryError;

pub(super) fn render_over(
    renderer: &mut Renderer,
    expr: &ExprNode,
    window: &WindowSpec,
    mode: RenderMode<'_>,
) -> Result<String, QueryError> {
    if matches!(mode, RenderMode::MutationRoot { .. }) {
        return Err(QueryError::BindError(
            "window functions are not allowed in mutation statements".to_string(),
        ));
    }
    validate_window_base(expr)?;
    dialect::validate_feature(renderer.dialect(), DialectFeature::WindowFunctions)?;
    let inner = renderer.render_expr(expr, mode)?;
    let spec = render_window(renderer, window, mode)?;
    Ok(format!("{inner} OVER ({spec})"))
}

fn validate_window_base(expr: &ExprNode) -> Result<(), QueryError> {
    match expr {
        ExprNode::Function { function, .. } if function.supports_window() => Ok(()),
        _ => Err(QueryError::BindError(
            "OVER requires a window-capable function".to_string(),
        )),
    }
}

fn render_window(
    renderer: &mut Renderer,
    window: &WindowSpec,
    mode: RenderMode<'_>,
) -> Result<String, QueryError> {
    let mut parts = Vec::new();
    push_partitions(renderer, window, mode, &mut parts)?;
    push_orders(renderer, window, mode, &mut parts)?;
    if let Some(frame) = &window.frame {
        parts.push(render_frame(renderer, frame, mode)?);
    }
    Ok(parts.join(" "))
}

fn push_partitions(
    renderer: &mut Renderer,
    window: &WindowSpec,
    mode: RenderMode<'_>,
    parts: &mut Vec<String>,
) -> Result<(), QueryError> {
    if window.partitions.is_empty() {
        return Ok(());
    }
    let rendered = window
        .partitions
        .iter()
        .map(|expr| renderer.render_expr(expr, mode))
        .collect::<Result<Vec<_>, _>>()?;
    parts.push(format!("PARTITION BY {}", rendered.join(", ")));
    Ok(())
}

fn push_orders(
    renderer: &mut Renderer,
    window: &WindowSpec,
    mode: RenderMode<'_>,
    parts: &mut Vec<String>,
) -> Result<(), QueryError> {
    if window.orders.is_empty() {
        return Ok(());
    }
    let mut rendered = Vec::with_capacity(window.orders.len());
    for order in &window.orders {
        rendered.push(render_order(renderer, order, mode)?);
    }
    parts.push(format!("ORDER BY {}", rendered.join(", ")));
    Ok(())
}

fn render_order(
    renderer: &mut Renderer,
    order: &OrderExpr,
    mode: RenderMode<'_>,
) -> Result<String, QueryError> {
    let direction = if order.desc { "DESC" } else { "ASC" };
    Ok(format!(
        "{} {direction}",
        renderer.render_expr(&order.expr, mode)?
    ))
}

fn render_frame(
    renderer: &mut Renderer,
    frame: &WindowFrame,
    mode: RenderMode<'_>,
) -> Result<String, QueryError> {
    let unit = match frame.unit {
        FrameUnit::Rows => "ROWS",
        FrameUnit::Range => "RANGE",
    };
    Ok(format!(
        "{unit} BETWEEN {} AND {}",
        render_bound(renderer, &frame.start, mode)?,
        render_bound(renderer, &frame.end, mode)?
    ))
}

fn render_bound(
    renderer: &mut Renderer,
    bound: &FrameBound,
    mode: RenderMode<'_>,
) -> Result<String, QueryError> {
    match bound.kind {
        FrameBoundKind::UnboundedPreceding => Ok("UNBOUNDED PRECEDING".to_string()),
        FrameBoundKind::CurrentRow => Ok("CURRENT ROW".to_string()),
        FrameBoundKind::UnboundedFollowing => Ok("UNBOUNDED FOLLOWING".to_string()),
        FrameBoundKind::Preceding => render_offset(renderer, bound, mode, "PRECEDING"),
        FrameBoundKind::Following => render_offset(renderer, bound, mode, "FOLLOWING"),
    }
}

fn render_offset(
    renderer: &mut Renderer,
    bound: &FrameBound,
    mode: RenderMode<'_>,
    suffix: &str,
) -> Result<String, QueryError> {
    let Some(expr) = bound.expr.as_ref() else {
        return Err(QueryError::BindError(format!(
            "{suffix} frame bound requires an offset"
        )));
    };
    Ok(format!("{} {suffix}", renderer.render_expr(expr, mode)?))
}
