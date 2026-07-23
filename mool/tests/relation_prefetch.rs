use mool as db;
use mool::mock::{DbCallKind, MockDbSession, PlannedCall, PlannedResponse, StatementMatcher};
use sqlx::Arguments as _;

#[derive(Debug, Clone, db::Model)]
#[table(
    name = "parents",
    primary_key(name = "parents_pk", columns = ["tenant_id", "id"])
)]
struct Parent {
    tenant_id: i64,
    id: i64,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "children")]
struct Child {
    id: i64,
    tenant_id: i64,
    parent_id: i64,
    body: String,
}

struct ParentChildren;

impl db::Backref for ParentChildren {
    type From = Parent;
    type To = Child;

    const NAME: &'static str = "children";
    const CARDINALITY: db::RelationCardinality = db::RelationCardinality::Many;

    fn meta() -> db::ReferenceMeta {
        db::ReferenceMeta {
            logical_name: "child",
            table_name: "children",
            table_schema: None,
            columns: &[
                db::JoinColumn {
                    from: "tenant_id",
                    to: "tenant_id",
                },
                db::JoinColumn {
                    from: "id",
                    to: "parent_id",
                },
            ],
            join_type: db::JoinType::Inner,
        }
    }
}

impl db::ManyBackref for ParentChildren {}

struct ParentWithChildren {
    parent: Parent,
    children: Vec<Child>,
}

impl db::PrefetchKey<ParentChildren> for ParentWithChildren {
    type Key = (i64, i64);

    const KEY_ARITY: usize = 2;

    fn parent_key(&self) -> Self::Key {
        (self.parent.tenant_id, self.parent.id)
    }

    fn child_key(child: &Child) -> Self::Key {
        (child.tenant_id, child.parent_id)
    }

    fn bind_parent_key(&self, statement: db::Statement) -> db::Statement {
        statement.bind(self.parent.tenant_id).bind(self.parent.id)
    }
}

impl db::ReceivesPrefetch<ParentChildren> for ParentWithChildren {
    fn receive_prefetch(&mut self, rows: Vec<Child>) {
        self.children = rows;
    }
}

fn parent(tenant_id: i64, id: i64) -> ParentWithChildren {
    ParentWithChildren {
        parent: Parent { tenant_id, id },
        children: Vec::new(),
    }
}

fn child(id: i64, tenant_id: i64, parent_id: i64) -> Child {
    Child {
        id,
        tenant_id,
        parent_id,
        body: format!("child-{id}"),
    }
}

#[cfg(feature = "postgres")]
const PREFETCH_SQL: &str = "SELECT id, tenant_id, parent_id, body FROM children WHERE (tenant_id, parent_id) IN (($1, $2), ($3, $4))";
#[cfg(any(feature = "sqlite", feature = "mysql", feature = "mariadb"))]
const PREFETCH_SQL: &str = "SELECT id, tenant_id, parent_id, body FROM children WHERE (tenant_id, parent_id) IN ((?, ?), (?, ?))";

/// Verifies composite-key prefetch uses row-value membership and hydrates each parent explicitly.
#[tokio::test]
async fn composite_prefetch_groups_children_by_all_key_columns() {
    let parents = vec![parent(1, 10), parent(2, 10)];
    let children = vec![child(1, 1, 10), child(2, 1, 10), child(3, 2, 10)];
    let mut session = MockDbSession::new();
    session.plan(PlannedCall {
        kind: DbCallKind::FetchAll,
        matcher: StatementMatcher::Exact(PREFETCH_SQL.to_string()),
        response: PlannedResponse::OkAnyVec(Box::new(children)),
    });

    let hydrated = db::prefetch::<ParentChildren, _>(parents)
        .exec(&mut session)
        .await
        .expect("composite prefetch");

    assert_eq!(hydrated[0].children.len(), 2);
    assert_eq!(hydrated[1].children.len(), 1);
    assert_eq!(session.recorded[0].stmt.arguments().len(), 4);
}
