use mool as db;
use mool::Model;

#[derive(Debug, Clone, Copy, PartialEq, Eq, db::SqlEnum)]
#[sql_enum(rename_all = "snake_case")]
enum Status {
    Draft,
    Published,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "users")]
struct User {
    #[column(primary_key)]
    id: i64,
    #[column(unique, index)]
    email: String,
}

#[derive(Debug, Clone, db::Model)]
#[table(name = "posts")]
struct Post {
    #[column(primary_key)]
    id: i64,
    #[column(reference = "users.id")]
    author_id: i64,
    title: String,
    #[column(sql_enum)]
    status: Status,
}

#[derive(Debug, Clone, db::Record)]
#[table(name = "posts")]
struct PostSummary {
    id: i64,
    title: String,
}

#[derive(Debug, Clone, db::Record)]
struct PostWithAuthor {
    #[column(flatten)]
    post: Post,
    #[column(reference(on(from = "author_id", to = "id")))]
    author: User,
}

#[derive(Debug, Clone, db::Filterable)]
#[filter(model = Post)]
struct PostFilter {
    #[filter(op = "eq")]
    status: Option<Status>,
    #[filter(op = "ilike", column = "title")]
    q: Option<String>,
    #[filter(op = "in", column = "id")]
    ids: Vec<i64>,
}

struct PostComments;

impl db::Backref for PostComments {
    type From = Post;
    type To = Post;

    const NAME: &'static str = "comments";
    const CARDINALITY: db::RelationCardinality = db::RelationCardinality::Many;

    fn meta() -> db::ReferenceMeta {
        db::ReferenceMeta {
            logical_name: "comment",
            table_name: "posts",
            table_schema: None,
            columns: &[db::JoinColumn {
                from: "posts.id",
                to: "author_id",
            }],
            join_type: db::JoinType::Inner,
        }
    }
}

impl db::ManyBackref for PostComments {}

fn main() {
    let posts = Post::table();
    let filter = PostFilter {
        status: Some(Status::Published),
        q: Some("%mool%".to_string()),
        ids: vec![1, 2],
    };

    let _ = db::from(&posts)
        .filter_with(&filter)
        .filter(db::backref::<PostComments>(&posts).exists())
        .all::<PostSummary>()
        .plan()
        .unwrap();

    let _ = db::from(&posts)
        .all::<PostWithAuthor>()
        .plan()
        .unwrap();
}
