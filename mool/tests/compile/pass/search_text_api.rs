use mool as db;
use db::Model as _;

#[derive(Debug, Clone, db::Model)]
#[table(full_text(
    name = "content",
    columns = ["title", "body"],
    postgres(config = "english")
))]
struct Document {
    #[column(primary_key)]
    id: i64,
    title: String,
    body: Option<String>,
}

fn main() {
    let documents = Document::table();
    let query = db::var::<String>().named("query");
    let relevance = documents.text_relevance("content", &query);
    let _ = db::from(&documents)
        .filter(documents.text_search("content", &query))
        .sort(relevance.desc())
        .bind(&query, "rust database".to_string())
        .all::<Document>();
}
