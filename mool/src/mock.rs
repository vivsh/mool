use std::any::Any;
use std::collections::VecDeque;

use crate::backend::{Database, Row};
use crate::{DbError, DbSession, Statement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbCallKind {
    Execute,
    FetchScalar,
    FetchOne,
    FetchAll,
    FetchOptional,
}

#[derive(Debug)]
pub struct RecordedCall {
    pub kind: DbCallKind,
    pub stmt: Statement,
}

pub struct PlannedCall {
    pub kind: DbCallKind,
    pub matcher: StatementMatcher,
    pub response: PlannedResponse,
}

/// Statement matching policy for a planned mock call.
pub enum StatementMatcher {
    /// Accepts any SQL statement.
    Any,
    /// Requires the complete SQL string to match.
    Exact(String),
    /// Requires the SQL string to contain a diagnostic fragment.
    Contains(String),
    /// Delegates matching to a custom statement predicate.
    Predicate {
        description: String,
        test: Box<dyn Fn(&Statement) -> bool + Send + Sync>,
    },
}

impl StatementMatcher {
    fn matches(&self, statement: &Statement) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(expected) => statement.sql() == expected,
            Self::Contains(expected) => statement.sql().contains(expected),
            Self::Predicate { test, .. } => test(statement),
        }
    }

    fn description(&self) -> String {
        match self {
            Self::Any => "any statement".to_string(),
            Self::Exact(expected) => format!("exact SQL {expected:?}"),
            Self::Contains(expected) => format!("SQL containing {expected:?}"),
            Self::Predicate { description, .. } => description.clone(),
        }
    }
}

pub enum PlannedResponse {
    OkU64(u64),
    OkUnit,                               // if you ever need
    OkAny(Box<dyn Any + Send + Sync>),    // for fetch_one / scalar
    OkAnyVec(Box<dyn Any + Send + Sync>), // for fetch_all returning Vec<T>
    OkAnyOpt(Box<dyn Any + Send + Sync>), // for fetch_optional returning Option<T>
    Err(DbError),
}

pub struct MockDbSession {
    pub recorded: Vec<RecordedCall>,
    planned: VecDeque<PlannedCall>,
    pub strict: bool, // if true: panic on unexpected calls
}

impl MockDbSession {
    pub fn new() -> Self {
        Self {
            recorded: Vec::new(),
            planned: VecDeque::new(),
            strict: true,
        }
    }

    pub fn plan(&mut self, call: PlannedCall) {
        self.planned.push_back(call);
    }

    pub fn plan_execute_ok(&mut self, sql: impl Into<String>, rows: u64) {
        self.plan(PlannedCall {
            kind: DbCallKind::Execute,
            matcher: StatementMatcher::Exact(sql.into()),
            response: PlannedResponse::OkU64(rows),
        });
    }

    /// Plans a successful execute call using an explicit substring matcher.
    pub fn plan_execute_contains(&mut self, sql: impl Into<String>, rows: u64) {
        self.plan(PlannedCall {
            kind: DbCallKind::Execute,
            matcher: StatementMatcher::Contains(sql.into()),
            response: PlannedResponse::OkU64(rows),
        });
    }

    pub fn plan_fetch_one_ok<T: Send + Sync + 'static>(
        &mut self,
        sql: impl Into<String>,
        value: T,
    ) {
        self.plan(PlannedCall {
            kind: DbCallKind::FetchOne,
            matcher: StatementMatcher::Exact(sql.into()),
            response: PlannedResponse::OkAny(Box::new(value)),
        });
    }

    pub fn plan_fetch_scalar_ok<T: Send + Sync + 'static>(
        &mut self,
        sql: impl Into<String>,
        value: T,
    ) {
        self.plan(PlannedCall {
            kind: DbCallKind::FetchScalar,
            matcher: StatementMatcher::Exact(sql.into()),
            response: PlannedResponse::OkAny(Box::new(value)),
        });
    }

    pub fn plan_err(&mut self, kind: DbCallKind, sql: impl Into<String>, err: DbError) {
        self.plan(PlannedCall {
            kind,
            matcher: StatementMatcher::Exact(sql.into()),
            response: PlannedResponse::Err(err),
        });
    }

    fn take_next(
        &mut self,
        kind: DbCallKind,
        stmt: &Statement,
    ) -> Result<PlannedResponse, DbError> {
        let Some(next) = self.planned.pop_front() else {
            return Err(self.failure(
                "unexpected call",
                format!("{kind:?} with SQL {:?}", stmt.sql()),
            ));
        };

        if next.kind != kind {
            return Err(self.failure(
                "call order",
                format!(
                    "expected call {:?}, got {:?} (SQL: {:?})",
                    next.kind,
                    kind,
                    stmt.sql()
                ),
            ));
        }

        if !next.matcher.matches(stmt) {
            return Err(self.failure(
                "statement match",
                format!(
                    "expected {}, got {:?}",
                    next.matcher.description(),
                    stmt.sql()
                ),
            ));
        }

        Ok(next.response)
    }

    fn failure(&self, operation: &'static str, reason: String) -> DbError {
        assert!(!self.strict, "MockDbSession: {reason}");
        DbError::Mock { operation, reason }
    }

    fn response_failure(&self, operation: &'static str, response: &PlannedResponse) -> DbError {
        self.failure(
            operation,
            format!(
                "planned response has incompatible kind {:?}",
                std::mem::discriminant(response)
            ),
        )
    }
}

impl Default for MockDbSession {
    fn default() -> Self {
        Self::new()
    }
}

impl DbSession for MockDbSession {
    async fn execute(&mut self, qs: Statement) -> Result<u64, DbError> {
        self.recorded.push(RecordedCall {
            kind: DbCallKind::Execute,
            stmt: qs.clone(),
        });
        match self.take_next(DbCallKind::Execute, &qs)? {
            PlannedResponse::OkU64(n) => Ok(n),
            PlannedResponse::Err(e) => Err(e),
            other => Err(self.response_failure("execute", &other)),
        }
    }

    async fn fetch_scalar<T>(&mut self, qs: Statement) -> Result<T, DbError>
    where
        for<'d> T: sqlx::Decode<'d, Database> + sqlx::Type<Database> + Send + Unpin + 'static,
    {
        self.recorded.push(RecordedCall {
            kind: DbCallKind::FetchScalar,
            stmt: qs.clone(),
        });
        match self.take_next(DbCallKind::FetchScalar, &qs)? {
            PlannedResponse::OkAny(v) => v.downcast::<T>().map(|b| *b).map_err(|_| DbError::Mock {
                operation: "fetch_scalar",
                reason: "planned response type mismatch".to_string(),
            }),
            PlannedResponse::Err(e) => Err(e),
            other => Err(self.response_failure("fetch_scalar", &other)),
        }
    }

    async fn fetch_one<M>(&mut self, qs: Statement) -> Result<M, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
    {
        self.recorded.push(RecordedCall {
            kind: DbCallKind::FetchOne,
            stmt: qs.clone(),
        });
        match self.take_next(DbCallKind::FetchOne, &qs)? {
            PlannedResponse::OkAny(v) => v.downcast::<M>().map(|b| *b).map_err(|_| DbError::Mock {
                operation: "fetch_one",
                reason: "planned response type mismatch".to_string(),
            }),
            PlannedResponse::Err(e) => Err(e),
            other => Err(self.response_failure("fetch_one", &other)),
        }
    }

    async fn fetch_all<M>(&mut self, qs: Statement) -> Result<Vec<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
    {
        self.recorded.push(RecordedCall {
            kind: DbCallKind::FetchAll,
            stmt: qs.clone(),
        });
        match self.take_next(DbCallKind::FetchAll, &qs)? {
            PlannedResponse::OkAnyVec(v) => {
                v.downcast::<Vec<M>>()
                    .map(|b| *b)
                    .map_err(|_| DbError::Mock {
                        operation: "fetch_all",
                        reason: "planned response type mismatch".to_string(),
                    })
            }
            PlannedResponse::Err(e) => Err(e),
            other => Err(self.response_failure("fetch_all", &other)),
        }
    }

    async fn fetch_optional<M>(&mut self, qs: Statement) -> Result<Option<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
    {
        self.recorded.push(RecordedCall {
            kind: DbCallKind::FetchOptional,
            stmt: qs.clone(),
        });
        match self.take_next(DbCallKind::FetchOptional, &qs)? {
            PlannedResponse::OkAnyOpt(v) => {
                v.downcast::<Option<M>>()
                    .map(|b| *b)
                    .map_err(|_| DbError::Mock {
                        operation: "fetch_optional",
                        reason: "planned response type mismatch".to_string(),
                    })
            }
            PlannedResponse::Err(e) => Err(e),
            other => Err(self.response_failure("fetch_optional", &other)),
        }
    }
}

/// A dummy pool that can be used in tests without a real database connection.
/// Wraps `MockDbSession` and implements `DbSession` trait.
///
/// # Examples
///
/// ```
/// use mool::mock::DummyPool;
///
/// let mut pool = DummyPool::new();
/// pool.plan_execute_ok("INSERT", 1);
/// // Use pool in your tests
/// ```
pub struct DummyPool {
    session: MockDbSession,
}

impl DummyPool {
    /// Create a new dummy pool with strict mode enabled
    pub fn new() -> Self {
        Self {
            session: MockDbSession::new(),
        }
    }

    /// Create a non-strict dummy pool that returns errors for unexpected calls
    pub fn relaxed() -> Self {
        let mut session = MockDbSession::new();
        session.strict = false;
        Self { session }
    }

    /// Plan an execute call with expected SQL pattern and row count
    pub fn plan_execute_ok(&mut self, sql: impl Into<String>, rows: u64) {
        self.session.plan_execute_ok(sql, rows);
    }

    /// Plans an execute call using an explicit SQL substring matcher.
    pub fn plan_execute_contains(&mut self, sql: impl Into<String>, rows: u64) {
        self.session.plan_execute_contains(sql, rows);
    }

    /// Plan a fetch_one call with expected SQL pattern and return value
    pub fn plan_fetch_one_ok<T: Send + Sync + 'static>(
        &mut self,
        sql: impl Into<String>,
        value: T,
    ) {
        self.session.plan_fetch_one_ok(sql, value);
    }

    /// Plan a fetch_scalar call with expected SQL pattern and return value
    pub fn plan_fetch_scalar_ok<T: Send + Sync + 'static>(
        &mut self,
        sql: impl Into<String>,
        value: T,
    ) {
        self.session.plan_fetch_scalar_ok(sql, value);
    }

    /// Plan an error response for any call type
    pub fn plan_err(&mut self, kind: DbCallKind, sql: impl Into<String>, err: DbError) {
        self.session.plan_err(kind, sql, err);
    }

    /// Plan a custom call with full control
    pub fn plan(&mut self, call: PlannedCall) {
        self.session.plan(call);
    }

    /// Get access to recorded calls for assertions
    pub fn recorded(&self) -> &[RecordedCall] {
        &self.session.recorded
    }
}

impl Default for DummyPool {
    fn default() -> Self {
        Self::new()
    }
}

impl DbSession for DummyPool {
    async fn execute(&mut self, qs: Statement) -> Result<u64, DbError> {
        self.session.execute(qs).await
    }

    async fn fetch_scalar<T>(&mut self, qs: Statement) -> Result<T, DbError>
    where
        for<'d> T: sqlx::Decode<'d, Database> + sqlx::Type<Database> + Send + Unpin + 'static,
    {
        self.session.fetch_scalar(qs).await
    }

    async fn fetch_one<M>(&mut self, qs: Statement) -> Result<M, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
    {
        self.session.fetch_one(qs).await
    }

    async fn fetch_all<M>(&mut self, qs: Statement) -> Result<Vec<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
    {
        self.session.fetch_all(qs).await
    }

    async fn fetch_optional<M>(&mut self, qs: Statement) -> Result<Option<M>, DbError>
    where
        M: for<'r> sqlx::FromRow<'r, Row> + Send + Unpin + 'static,
    {
        self.session.fetch_optional(qs).await
    }
}
